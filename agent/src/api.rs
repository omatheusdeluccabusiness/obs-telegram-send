use axum::{
    extract::{Path, Request, State},
    http::{header::AUTHORIZATION, HeaderValue, StatusCode},
    middleware::{self, Next},
    response::{IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use rand::RngCore;
use serde::{Deserialize, Serialize};
use std::future::Future;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use crate::{
    config::{validate_config, validate_credentials, ConfigInput, TelegramCredentials},
    jobs::{JobError, JobService},
    keychain::SecretStore,
    telegram::{LocalBotApiServer, TelegramError, TelegramGateway},
};

type LocalJobs = JobService<TelegramGateway>;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum LocalBotApiStartState {
    Stopped,
    Starting([u8; 32]),
    Ready([u8; 32]),
}

#[derive(Clone)]
struct ApiState {
    gateway: TelegramGateway,
    jobs: LocalJobs,
    local_bot_api: Arc<Mutex<Option<LocalBotApiServer>>>,
    local_bot_api_start: Arc<tokio::sync::Mutex<LocalBotApiStartState>>,
}

impl Default for ApiState {
    fn default() -> Self {
        let local_bot_api = Arc::new(Mutex::new(None));
        Self::with_shared_server(local_bot_api)
    }
}

impl ApiState {
    fn with_shared_server(local_bot_api: Arc<Mutex<Option<LocalBotApiServer>>>) -> Self {
        let gateway = TelegramGateway::from_keychain_with_server(Arc::clone(&local_bot_api));
        let jobs = JobService::in_app_support(gateway.clone())
            .expect("agent must access its protected local upload-job store");
        let state = Self {
            gateway,
            jobs,
            local_bot_api,
            local_bot_api_start: Arc::new(tokio::sync::Mutex::new(LocalBotApiStartState::Stopped)),
        };
        debug_assert!(state.gateway.uses_server_handle(&state.local_bot_api));
        state
    }
    async fn start_local_bot_api(
        &self,
        configuration: &crate::config::TelegramConfig,
    ) -> Result<(), TelegramError> {
        let fingerprint = LocalBotApiServer::app_credentials_fingerprint(
            configuration.api_id,
            &configuration.api_hash,
        );
        ensure_server_for(
            &self.local_bot_api_start,
            &self.local_bot_api,
            fingerprint,
            || LocalBotApiServer::start_after_cloud_logout(configuration, 0),
            ready_endpoint,
            |endpoint| async move {
                LocalBotApiServer::validate_config_after_cloud_logout(configuration, &endpoint)
                    .await
            },
        )
        .await
        .map(|_| ())
    }

    async fn start_onboarding_bot_api(
        &self,
        credentials: &TelegramCredentials,
    ) -> Result<String, TelegramError> {
        let fingerprint = LocalBotApiServer::app_credentials_fingerprint(
            credentials.api_id,
            &credentials.api_hash,
        );
        ensure_server_for(
            &self.local_bot_api_start,
            &self.local_bot_api,
            fingerprint,
            || LocalBotApiServer::start_onboarding_after_cloud_logout(credentials, 0),
            ready_endpoint,
            |endpoint| async move {
                LocalBotApiServer::validate_onboarding_after_cloud_logout(credentials, &endpoint)
                    .await
            },
        )
        .await
    }
}

fn ready_endpoint(server: &mut LocalBotApiServer) -> Result<String, TelegramError> {
    server.ensure_ready()?;
    Ok(server.endpoint())
}

async fn ensure_server_for<T, Start, StartFuture, Endpoint, Validate, ValidateFuture>(
    start_state: &tokio::sync::Mutex<LocalBotApiStartState>,
    server: &Mutex<Option<T>>,
    fingerprint: [u8; 32],
    start: Start,
    endpoint: Endpoint,
    validate_existing: Validate,
) -> Result<String, TelegramError>
where
    Start: FnOnce() -> StartFuture,
    StartFuture: Future<Output = Result<T, TelegramError>>,
    Endpoint: Fn(&mut T) -> Result<String, TelegramError>,
    Validate: FnOnce(String) -> ValidateFuture,
    ValidateFuture: Future<Output = Result<(), TelegramError>>,
{
    let mut state = start_state.lock().await;
    if *state == LocalBotApiStartState::Ready(fingerprint) {
        let endpoint_result = match server.lock() {
            Ok(mut server) => server
                .as_mut()
                .ok_or(TelegramError::RequestFailed)
                .and_then(&endpoint),
            Err(_) => Err(TelegramError::RequestFailed),
        };
        let existing_endpoint = match endpoint_result {
            Ok(endpoint) => endpoint,
            Err(error) => {
                *state = LocalBotApiStartState::Stopped;
                let failed = server
                    .lock()
                    .map_err(|_| TelegramError::RequestFailed)?
                    .take();
                drop(failed);
                return Err(error);
            }
        };
        validate_existing(existing_endpoint.clone()).await?;
        return Ok(existing_endpoint);
    }
    *state = LocalBotApiStartState::Starting(fingerprint);
    let previous = server
        .lock()
        .map_err(|_| TelegramError::RequestFailed)?
        .take();
    drop(previous);
    match start().await {
        Ok(mut candidate) => {
            let candidate_endpoint = match endpoint(&mut candidate) {
                Ok(endpoint) => endpoint,
                Err(error) => {
                    *state = LocalBotApiStartState::Stopped;
                    return Err(error);
                }
            };
            let mut server = match server.lock() {
                Ok(server) => server,
                Err(_) => {
                    *state = LocalBotApiStartState::Stopped;
                    return Err(TelegramError::RequestFailed);
                }
            };
            *server = Some(candidate);
            *state = LocalBotApiStartState::Ready(fingerprint);
            Ok(candidate_endpoint)
        }
        Err(error) => {
            *state = LocalBotApiStartState::Stopped;
            Err(error)
        }
    }
}

#[derive(Serialize)]
pub struct AgentError {
    pub code: String,
    pub message: String,
}

impl AgentError {
    fn unauthorized() -> Self {
        Self {
            code: "unauthorized".to_owned(),
            message: "A valid install bearer is required.".to_owned(),
        }
    }

    fn invalid_configuration() -> Self {
        Self {
            code: "invalid_configuration".to_owned(),
            message: "The Telegram configuration is invalid.".to_owned(),
        }
    }

    fn storage_unavailable() -> Self {
        Self {
            code: "storage_unavailable".to_owned(),
            message: "Secure configuration storage is unavailable.".to_owned(),
        }
    }

    fn local_server_unavailable() -> Self {
        Self {
            code: "local_server_unavailable".to_owned(),
            message: "The local Telegram service could not start.".to_owned(),
        }
    }

    fn invalid_job() -> Self {
        Self {
            code: "invalid_job".to_owned(),
            message: "The recording cannot be queued for upload.".to_owned(),
        }
    }

    fn job_not_found() -> Self {
        Self {
            code: "job_not_found".to_owned(),
            message: "The upload job was not found.".to_owned(),
        }
    }

    fn job_not_retryable() -> Self {
        Self {
            code: "job_not_retryable".to_owned(),
            message: "This upload job cannot be retried.".to_owned(),
        }
    }

    fn telegram(error: TelegramError) -> Self {
        let (code, message) = match error {
            TelegramError::ConfigurationUnavailable => (
                "telegram_not_configured",
                "Telegram configuration is not available.",
            ),
            TelegramError::ChatNotFound => (
                "chat_not_found",
                "No Telegram chat was found. Send /start to the bot first.",
            ),
            TelegramError::FileTooLarge => {
                ("file_too_large", "The recording is larger than 2 GiB.")
            }
            TelegramError::RequestFailed => (
                "telegram_request_failed",
                "Telegram could not complete the request.",
            ),
        };
        Self {
            code: code.to_owned(),
            message: message.to_owned(),
        }
    }
}

pub fn router(install_secret: String) -> Router {
    router_with_state(install_secret, ApiState::default())
}

pub fn router_with_local_bot_api(
    install_secret: String,
    local_bot_api: Option<LocalBotApiServer>,
) -> Router {
    let state = ApiState::with_shared_server(Arc::new(Mutex::new(local_bot_api)));
    router_with_state(install_secret, state)
}

fn router_with_state(install_secret: String, state: ApiState) -> Router {
    Router::new()
        .route("/v1/config", post(save_configuration))
        .route("/v1/jobs", post(create_job))
        .route("/v1/jobs/{job_id}", get(get_job))
        .route("/v1/jobs/{job_id}/retry", post(retry_job))
        .route("/v1/chat/detect", post(detect_chat))
        .route("/v1/test-send", post(test_send))
        .with_state(state)
        .route_layer(middleware::from_fn_with_state(
            install_secret,
            require_bearer,
        ))
}

async fn require_bearer(State(secret): State<String>, request: Request, next: Next) -> Response {
    let expected = HeaderValue::from_str(&format!("Bearer {secret}"));
    if expected.ok().as_ref() == request.headers().get(AUTHORIZATION) {
        return next.run(request).await;
    }

    (StatusCode::UNAUTHORIZED, Json(AgentError::unauthorized())).into_response()
}

async fn save_configuration(
    State(state): State<ApiState>,
    Json(input): Json<ConfigInput>,
) -> Response {
    let configuration = match validate_config(input) {
        Ok(configuration) => configuration,
        Err(_) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(AgentError::invalid_configuration()),
            )
                .into_response()
        }
    };

    let result = SecretStore::new().and_then(|store| store.save(&configuration));
    match result {
        Ok(()) => match state.start_local_bot_api(&configuration).await {
            Ok(()) => StatusCode::NO_CONTENT.into_response(),
            Err(_) => (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(AgentError::local_server_unavailable()),
            )
                .into_response(),
        },
        Err(_) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(AgentError::storage_unavailable()),
        )
            .into_response(),
    }
}

#[derive(Deserialize)]
struct CreateJobInput {
    recording_path: PathBuf,
    display_name: String,
}

#[derive(Serialize)]
struct CreatedJob {
    job_id: String,
    state: &'static str,
}

async fn create_job(State(state): State<ApiState>, Json(input): Json<CreateJobInput>) -> Response {
    let job_id = match state
        .jobs
        .enqueue(input.recording_path, input.display_name)
        .await
    {
        Ok(job_id) => job_id,
        Err(error) => return job_error_response(error),
    };

    // This task was explicitly authorized by the plugin POST. No recording is
    // ever discovered or queued by a timer, file watcher, or recording event.
    let jobs = state.jobs.clone();
    let upload_job_id = job_id.clone();
    tokio::spawn(async move {
        let _ = jobs.start_upload(&upload_job_id).await;
    });
    (
        StatusCode::CREATED,
        Json(CreatedJob {
            job_id,
            state: "queued",
        }),
    )
        .into_response()
}

async fn get_job(State(state): State<ApiState>, Path(job_id): Path<String>) -> Response {
    match state.jobs.status(&job_id).await {
        Ok(status) => Json(status).into_response(),
        Err(error) => job_error_response(error),
    }
}

async fn retry_job(State(state): State<ApiState>, Path(job_id): Path<String>) -> Response {
    match state.jobs.retry(&job_id).await {
        Ok(status) => {
            let jobs = state.jobs.clone();
            let upload_job_id = job_id;
            tokio::spawn(async move {
                let _ = jobs.start_upload(&upload_job_id).await;
            });
            Json(status).into_response()
        }
        Err(error) => job_error_response(error),
    }
}

#[derive(Deserialize)]
struct DetectChatInput {
    bot_token: String,
    api_id: u32,
    api_hash: String,
    challenge: Option<String>,
    confirmed_chat_id: Option<i64>,
}

async fn detect_chat(
    State(state): State<ApiState>,
    Json(input): Json<DetectChatInput>,
) -> Response {
    let credentials = match validate_credentials(input.bot_token, input.api_id, input.api_hash) {
        Ok(credentials) => credentials,
        Err(_) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(AgentError::invalid_configuration()),
            )
                .into_response()
        }
    };
    if let Some(chat_id) = input.confirmed_chat_id.filter(|chat_id| *chat_id != 0) {
        return Json(serde_json::json!({ "chat_id": chat_id, "confirmed": true })).into_response();
    }
    let endpoint = match state.start_onboarding_bot_api(&credentials).await {
        Ok(endpoint) => endpoint,
        Err(_) => {
            return (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(AgentError::local_server_unavailable()),
            )
                .into_response()
        }
    };
    let Some(challenge) = input.challenge else {
        return Json(serde_json::json!({ "challenge": new_challenge() })).into_response();
    };
    let gateway = TelegramGateway::onboarding_with_endpoint(credentials, endpoint);
    match gateway.detect_chat(&challenge).await {
        Ok(chat_id) => {
            Json(serde_json::json!({ "chat_id": chat_id, "challenge": challenge })).into_response()
        }
        Err(error) => (StatusCode::BAD_REQUEST, Json(AgentError::telegram(error))).into_response(),
    }
}

#[derive(Deserialize)]
struct TestSendInput {
    message: Option<String>,
}

async fn test_send(State(state): State<ApiState>, Json(input): Json<TestSendInput>) -> Response {
    let message = input
        .message
        .unwrap_or_else(|| "OBS Telegram Send is connected.".to_owned());
    match state.gateway.test_send(&message).await {
        Ok(()) => StatusCode::NO_CONTENT.into_response(),
        Err(error) => (StatusCode::BAD_REQUEST, Json(AgentError::telegram(error))).into_response(),
    }
}

fn job_error_response(error: JobError) -> Response {
    let (status, body) = match error {
        JobError::FileTooLarge => (StatusCode::PAYLOAD_TOO_LARGE, AgentError::invalid_job()),
        JobError::FileUnavailable | JobError::InvalidDisplayName => {
            (StatusCode::BAD_REQUEST, AgentError::invalid_job())
        }
        JobError::StorageUnavailable => (
            StatusCode::INTERNAL_SERVER_ERROR,
            AgentError::storage_unavailable(),
        ),
        JobError::NotFound => (StatusCode::NOT_FOUND, AgentError::job_not_found()),
        JobError::NotRetryable => (StatusCode::CONFLICT, AgentError::job_not_retryable()),
    };
    (status, Json(body)).into_response()
}

fn new_challenge() -> String {
    let mut bytes = [0_u8; 16];
    rand::rngs::OsRng.fill_bytes(&mut bytes);
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

#[cfg(test)]
mod concurrent_start_tests {
    use super::{ensure_server_for, LocalBotApiStartState, TelegramError};
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::{Arc, Mutex};

    struct FakeServer {
        endpoint: String,
        drops: Option<Arc<AtomicUsize>>,
    }

    impl Drop for FakeServer {
        fn drop(&mut self) {
            if let Some(drops) = &self.drops {
                drops.fetch_add(1, Ordering::SeqCst);
            }
        }
    }

    fn endpoint(server: &mut FakeServer) -> Result<String, TelegramError> {
        Ok(server.endpoint.clone())
    }

    #[tokio::test]
    async fn a_new_token_reuses_the_child_but_runs_its_own_migration_and_validation() {
        let fingerprint = [7; 32];
        let state = tokio::sync::Mutex::new(LocalBotApiStartState::Ready(fingerprint));
        let server = Mutex::new(Some(FakeServer {
            endpoint: "http://127.0.0.1:41723".to_owned(),
            drops: None,
        }));
        let starts = AtomicUsize::new(0);
        let token_migrations = AtomicUsize::new(0);

        let reused_endpoint = ensure_server_for(
            &state,
            &server,
            fingerprint,
            || async {
                starts.fetch_add(1, Ordering::SeqCst);
                Err::<FakeServer, _>(TelegramError::RequestFailed)
            },
            endpoint,
            |_| async {
                token_migrations.fetch_add(1, Ordering::SeqCst);
                Ok(())
            },
        )
        .await
        .unwrap();

        assert_eq!(reused_endpoint, "http://127.0.0.1:41723");
        assert_eq!(starts.load(Ordering::SeqCst), 0);
        assert_eq!(token_migrations.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn changed_app_credentials_drop_the_exact_child_and_start_once() {
        let drops = Arc::new(AtomicUsize::new(0));
        let state = tokio::sync::Mutex::new(LocalBotApiStartState::Ready([1; 32]));
        let server = Mutex::new(Some(FakeServer {
            endpoint: "http://127.0.0.1:41001".to_owned(),
            drops: Some(Arc::clone(&drops)),
        }));
        let starts = AtomicUsize::new(0);

        let endpoint = ensure_server_for(
            &state,
            &server,
            [2; 32],
            || async {
                starts.fetch_add(1, Ordering::SeqCst);
                Ok(FakeServer {
                    endpoint: "http://127.0.0.1:41002".to_owned(),
                    drops: None,
                })
            },
            endpoint,
            |_| async { Ok(()) },
        )
        .await
        .unwrap();

        assert_eq!(endpoint, "http://127.0.0.1:41002");
        assert_eq!(drops.load(Ordering::SeqCst), 1);
        assert_eq!(starts.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn concurrent_requests_share_one_completed_start() {
        let gate = Arc::new(tokio::sync::Mutex::new(LocalBotApiStartState::Stopped));
        let server = Arc::new(Mutex::new(None));
        let starts = Arc::new(AtomicUsize::new(0));
        let mut tasks = Vec::new();

        for _ in 0..2 {
            let gate = Arc::clone(&gate);
            let server = Arc::clone(&server);
            let starts = Arc::clone(&starts);
            tasks.push(tokio::spawn(async move {
                ensure_server_for(
                    &gate,
                    &server,
                    [9; 32],
                    || async move {
                        starts.fetch_add(1, Ordering::SeqCst);
                        tokio::time::sleep(std::time::Duration::from_millis(25)).await;
                        Ok::<_, TelegramError>(FakeServer {
                            endpoint: "http://127.0.0.1:41723".to_owned(),
                            drops: None,
                        })
                    },
                    endpoint,
                    |_| async { Ok(()) },
                )
                .await
            }));
        }

        for task in tasks {
            task.await.unwrap().unwrap();
        }
        assert_eq!(starts.load(Ordering::SeqCst), 1);
        assert_eq!(*gate.lock().await, LocalBotApiStartState::Ready([9; 32]));
        assert_eq!(
            server
                .lock()
                .unwrap()
                .as_ref()
                .map(|server| server.endpoint.as_str()),
            Some("http://127.0.0.1:41723")
        );
    }
}
