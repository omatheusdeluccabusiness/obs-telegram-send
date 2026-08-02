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
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use crate::{
    config::{validate_config, validate_credentials, ConfigInput, TelegramCredentials},
    jobs::{JobError, JobService},
    keychain::SecretStore,
    telegram::{LocalBotApiServer, TelegramError, TelegramGateway},
};

type LocalJobs = JobService<TelegramGateway>;

#[derive(Clone)]
struct ApiState {
    gateway: TelegramGateway,
    jobs: LocalJobs,
    local_bot_api: Arc<Mutex<Option<LocalBotApiServer>>>,
}

impl Default for ApiState {
    fn default() -> Self {
        let local_bot_api = Arc::new(Mutex::new(None));
        let gateway = TelegramGateway::from_keychain_with_server(local_bot_api.clone());
        let jobs = JobService::in_app_support(gateway.clone())
            .expect("agent must access its protected local upload-job store");
        Self {
            gateway,
            jobs,
            local_bot_api,
        }
    }
}

impl ApiState {
    fn start_local_bot_api(
        &self,
        configuration: &crate::config::TelegramConfig,
    ) -> Result<(), TelegramError> {
        let mut server = self.local_bot_api.lock().unwrap();
        if server.is_none() {
            *server = Some(LocalBotApiServer::start(configuration, 0)?);
        }
        Ok(())
    }

    fn start_onboarding_bot_api(
        &self,
        credentials: &TelegramCredentials,
    ) -> Result<String, TelegramError> {
        let mut server = self
            .local_bot_api
            .lock()
            .map_err(|_| TelegramError::RequestFailed)?;
        if server.is_none() {
            *server = Some(LocalBotApiServer::start_with_onboarding_credentials(
                credentials,
                0,
            )?);
        }
        let server = server.as_mut().ok_or(TelegramError::RequestFailed)?;
        server.ensure_ready()?;
        Ok(server.endpoint())
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
    let state = ApiState {
        local_bot_api: Arc::new(Mutex::new(local_bot_api)),
        ..ApiState::default()
    };
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
        Ok(()) => match state.start_local_bot_api(&configuration) {
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
    let endpoint = match state.start_onboarding_bot_api(&credentials) {
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
