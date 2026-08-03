use std::{
    future::Future,
    io::Write,
    net::{Ipv4Addr, SocketAddr, TcpStream},
    path::{Path, PathBuf},
    process::{Child, Command},
    time::Duration,
};

use async_trait::async_trait;
use directories_next::BaseDirs;
use reqwest::{
    multipart::{Form, Part},
    Client,
};
use secrecy::ExposeSecret;
use serde_json::Value;
use sha2::{Digest, Sha256};
use tokio::io::AsyncReadExt;
use tokio_util::io::ReaderStream;

use crate::{
    config::{TelegramConfig, TelegramCredentials},
    install_bearer::APP_SUPPORT_DIRECTORY_NAME,
    jobs::MAX_FILE_SIZE_BYTES,
    keychain::SecretStore,
    private_fs::{ensure_private_directory, private_create_truncate},
};

pub const DEFAULT_BOT_API_ENDPOINT: &str = "http://127.0.0.1:8081";
pub const TELEGRAM_CLOUD_BOT_API_ENDPOINT: &str = "https://api.telegram.org";
#[cfg(target_os = "macos")]
pub const PACKAGED_BOT_API_PATH: &str =
    "/Library/Application Support/OBS-Telegram-Send/telegram-bot-api";
const BOT_API_DATA_DIRECTORY_NAME: &str = "telegram-bot-api-data";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TelegramError {
    ConfigurationUnavailable,
    ChatNotFound,
    FileTooLarge,
    RequestFailed,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UploadError {
    PreUploadRejected,
    TransportUncertain,
}

impl std::fmt::Display for TelegramError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let message = match self {
            Self::ConfigurationUnavailable => "Telegram is not configured.",
            Self::ChatNotFound => "No Telegram chat was found. Send /start to the bot first.",
            Self::FileTooLarge => "The recording is larger than 2 GiB.",
            Self::RequestFailed => "Telegram could not complete the request.",
        };
        formatter.write_str(message)
    }
}

#[async_trait]
pub trait TelegramClient: Clone + Send + Sync + 'static {
    async fn upload_file(&self, path: PathBuf, display_name: String) -> Result<(), UploadError>;
}

#[derive(Clone)]
enum CredentialSource {
    Keychain,
    Supplied(std::sync::Arc<TelegramConfig>),
    Onboarding(std::sync::Arc<TelegramCredentials>),
}

#[derive(Clone)]
enum EndpointSource {
    Fixed(String),
    Managed(std::sync::Arc<std::sync::Mutex<Option<LocalBotApiServer>>>),
}

#[derive(Clone)]
pub struct TelegramGateway {
    client: Client,
    endpoint: EndpointSource,
    credentials: CredentialSource,
}

impl Default for TelegramGateway {
    fn default() -> Self {
        Self::from_keychain(DEFAULT_BOT_API_ENDPOINT)
    }
}

impl TelegramGateway {
    pub fn from_keychain(endpoint: impl Into<String>) -> Self {
        Self {
            client: Client::new(),
            endpoint: EndpointSource::Fixed(endpoint.into().trim_end_matches('/').to_owned()),
            credentials: CredentialSource::Keychain,
        }
    }

    pub fn with_endpoint(config: TelegramConfig, endpoint: impl Into<String>) -> Self {
        Self {
            client: Client::new(),
            endpoint: EndpointSource::Fixed(endpoint.into().trim_end_matches('/').to_owned()),
            credentials: CredentialSource::Supplied(std::sync::Arc::new(config)),
        }
    }

    pub fn onboarding_with_endpoint(
        credentials: TelegramCredentials,
        endpoint: impl Into<String>,
    ) -> Self {
        Self {
            client: Client::new(),
            endpoint: EndpointSource::Fixed(endpoint.into().trim_end_matches('/').to_owned()),
            credentials: CredentialSource::Onboarding(std::sync::Arc::new(credentials)),
        }
    }

    pub fn from_keychain_with_server(
        server: std::sync::Arc<std::sync::Mutex<Option<LocalBotApiServer>>>,
    ) -> Self {
        Self {
            client: Client::new(),
            endpoint: EndpointSource::Managed(server),
            credentials: CredentialSource::Keychain,
        }
    }

    pub fn uses_server_handle(
        &self,
        server: &std::sync::Arc<std::sync::Mutex<Option<LocalBotApiServer>>>,
    ) -> bool {
        matches!(&self.endpoint, EndpointSource::Managed(owned) if std::sync::Arc::ptr_eq(owned, server))
    }

    pub async fn detect_chat_for_nonce(&self, nonce: &str) -> Result<i64, TelegramError> {
        let response = self
            .client
            .get(self.url(self.bot_token()?.expose_secret(), "getUpdates")?)
            .send()
            .await
            .map_err(|_| TelegramError::RequestFailed)?
            .error_for_status()
            .map_err(|_| TelegramError::RequestFailed)?;
        let value: Value = response
            .json()
            .await
            .map_err(|_| TelegramError::RequestFailed)?;
        let Some(updates) = value.get("result").and_then(Value::as_array) else {
            return Err(TelegramError::ChatNotFound);
        };

        updates
            .iter()
            .find_map(|update| matching_start_chat(update, nonce))
            .ok_or(TelegramError::ChatNotFound)
    }

    pub async fn detect_chat(&self, nonce: &str) -> Result<i64, TelegramError> {
        self.detect_chat_for_nonce(nonce).await
    }

    pub async fn test_send(&self, message: &str) -> Result<(), TelegramError> {
        let config = self.configuration()?;
        let response = self
            .client
            .post(self.url(config.bot_token.expose_secret(), "sendMessage")?)
            .form(&[
                ("chat_id", config.chat_id.to_string()),
                ("text", message.to_owned()),
            ])
            .send()
            .await
            .and_then(reqwest::Response::error_for_status)
            .map(|_| ())
            .map_err(|_| TelegramError::RequestFailed);
        response
    }

    pub async fn upload_file(
        &self,
        path: PathBuf,
        display_name: String,
    ) -> Result<(), UploadError> {
        let config = self
            .configuration()
            .map_err(|_| UploadError::PreUploadRejected)?;
        let is_mp4 = path
            .extension()
            .and_then(|extension| extension.to_str())
            .is_some_and(|extension| extension.eq_ignore_ascii_case("mp4"));
        let field = if is_mp4 { "video" } else { "document" };
        let method = if is_mp4 { "sendVideo" } else { "sendDocument" };
        let file = tokio::fs::File::open(&path)
            .await
            .map_err(|_| UploadError::PreUploadRejected)?;
        let size = file
            .metadata()
            .await
            .map_err(|_| UploadError::PreUploadRejected)?
            .len();
        if size > MAX_FILE_SIZE_BYTES {
            return Err(UploadError::PreUploadRejected);
        }
        let part = Part::stream_with_length(
            reqwest::Body::wrap_stream(ReaderStream::new(file.take(size))),
            size,
        )
        .file_name(display_name);
        let form = Form::new()
            .text("chat_id", config.chat_id.to_string())
            .part(field, part);

        let response = self
            .client
            .post(
                self.url(config.bot_token.expose_secret(), method)
                    .map_err(|_| UploadError::PreUploadRejected)?,
            )
            .multipart(form)
            .send()
            .await
            .map_err(|_| UploadError::TransportUncertain)?;
        if !response.status().is_success() {
            return Err(UploadError::TransportUncertain);
        }
        Ok(())
    }

    fn configuration(&self) -> Result<std::sync::Arc<TelegramConfig>, TelegramError> {
        match &self.credentials {
            CredentialSource::Supplied(config) => Ok(config.clone()),
            CredentialSource::Onboarding(_) => Err(TelegramError::ConfigurationUnavailable),
            CredentialSource::Keychain => SecretStore::new()
                .and_then(|store| store.load())
                .ok()
                .flatten()
                .map(std::sync::Arc::new)
                .ok_or(TelegramError::ConfigurationUnavailable),
        }
    }

    fn bot_token(&self) -> Result<std::sync::Arc<secrecy::SecretString>, TelegramError> {
        match &self.credentials {
            CredentialSource::Supplied(config) => Ok(std::sync::Arc::new(config.bot_token.clone())),
            CredentialSource::Onboarding(credentials) => {
                Ok(std::sync::Arc::new(credentials.bot_token.clone()))
            }
            CredentialSource::Keychain => self
                .configuration()
                .map(|config| std::sync::Arc::new(config.bot_token.clone())),
        }
    }

    fn url(&self, bot_token: &str, method: &str) -> Result<String, TelegramError> {
        Ok(format!("{}/bot{bot_token}/{method}", self.endpoint()?))
    }

    fn endpoint(&self) -> Result<String, TelegramError> {
        match &self.endpoint {
            EndpointSource::Fixed(endpoint) => Ok(endpoint.clone()),
            EndpointSource::Managed(server) => {
                let mut server = server.lock().map_err(|_| TelegramError::RequestFailed)?;
                let server = server
                    .as_mut()
                    .ok_or(TelegramError::ConfigurationUnavailable)?;
                server.ensure_ready()?;
                Ok(server.endpoint())
            }
        }
    }
}

#[async_trait]
impl TelegramClient for TelegramGateway {
    async fn upload_file(&self, path: PathBuf, display_name: String) -> Result<(), UploadError> {
        TelegramGateway::upload_file(self, path, display_name).await
    }
}

pub struct LocalBotApiServer {
    child: Child,
    port: u16,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BotApiDirectories {
    data: PathBuf,
    temporary: PathBuf,
}

impl BotApiDirectories {
    pub fn data_dir(&self) -> &Path {
        &self.data
    }

    pub fn temp_dir(&self) -> &Path {
        &self.temporary
    }
}

impl LocalBotApiServer {
    pub fn app_credentials_fingerprint(api_id: u32, api_hash: &secrecy::SecretString) -> [u8; 32] {
        let mut digest = Sha256::new();
        digest.update(api_id.to_be_bytes());
        digest.update(api_hash.expose_secret().as_bytes());
        digest.finalize().into()
    }

    pub fn packaged_executable_path() -> PathBuf {
        #[cfg(target_os = "macos")]
        {
            PathBuf::from(PACKAGED_BOT_API_PATH)
        }
        #[cfg(target_os = "windows")]
        {
            std::env::current_exe()
                .ok()
                .and_then(|path| path.parent().map(Path::to_path_buf))
                .unwrap_or_else(|| PathBuf::from("."))
                .join("telegram-bot-api.exe")
        }
        #[cfg(not(any(target_os = "macos", target_os = "windows")))]
        {
            PathBuf::from("telegram-bot-api")
        }
    }

    pub fn prepare_directories_at(
        application_support: &Path,
    ) -> Result<BotApiDirectories, TelegramError> {
        // The packaged executable is named `telegram-bot-api` in the same
        // application-support directory. Keep runtime data at a distinct path
        // so a regular executable file never collides with this directory.
        let data = application_support.join(BOT_API_DATA_DIRECTORY_NAME);
        let temporary = data.join("temp");
        for directory in [application_support, data.as_path(), temporary.as_path()] {
            ensure_private_directory(directory).map_err(|_| TelegramError::RequestFailed)?;
        }
        Ok(BotApiDirectories { data, temporary })
    }

    fn prepare_user_directories() -> Result<BotApiDirectories, TelegramError> {
        let base_directories = BaseDirs::new().ok_or(TelegramError::RequestFailed)?;
        Self::prepare_directories_at(
            &base_directories
                .data_local_dir()
                .join(APP_SUPPORT_DIRECTORY_NAME),
        )
    }

    pub fn start(config: &TelegramConfig, port: u16) -> Result<Self, TelegramError> {
        Self::start_with_credentials(config.api_id, config.api_hash.expose_secret(), port)
    }

    pub fn start_with_onboarding_credentials(
        credentials: &TelegramCredentials,
        port: u16,
    ) -> Result<Self, TelegramError> {
        Self::start_with_credentials(
            credentials.api_id,
            credentials.api_hash.expose_secret(),
            port,
        )
    }

    /// Moves the bot out of Telegram's hosted Bot API before a local server
    /// claims updates. Telegram documents `logOut` as the required transition;
    /// a private token-fingerprint marker keeps restarts recoverable after a
    /// successful transition.
    pub async fn start_after_cloud_logout(
        config: &TelegramConfig,
        port: u16,
    ) -> Result<Self, TelegramError> {
        migrate_bot_to_local(config, TELEGRAM_CLOUD_BOT_API_ENDPOINT, || async {
            let server = Self::start(config, port)?;
            server.validate_bot(&config.bot_token).await?;
            Ok(server)
        })
        .await
    }

    pub async fn start_onboarding_after_cloud_logout(
        credentials: &TelegramCredentials,
        port: u16,
    ) -> Result<Self, TelegramError> {
        migrate_bot_token_to_local(
            &credentials.bot_token,
            TELEGRAM_CLOUD_BOT_API_ENDPOINT,
            || async {
                let server = Self::start_with_onboarding_credentials(credentials, port)?;
                server.validate_bot(&credentials.bot_token).await?;
                Ok(server)
            },
        )
        .await
    }

    pub async fn validate_config_after_cloud_logout(
        config: &TelegramConfig,
        endpoint: &str,
    ) -> Result<(), TelegramError> {
        migrate_bot_to_local(config, TELEGRAM_CLOUD_BOT_API_ENDPOINT, || async {
            Self::validate_bot_at(&config.bot_token, endpoint).await
        })
        .await
    }

    pub async fn validate_onboarding_after_cloud_logout(
        credentials: &TelegramCredentials,
        endpoint: &str,
    ) -> Result<(), TelegramError> {
        migrate_bot_token_to_local(
            &credentials.bot_token,
            TELEGRAM_CLOUD_BOT_API_ENDPOINT,
            || async { Self::validate_bot_at(&credentials.bot_token, endpoint).await },
        )
        .await
    }

    fn start_with_credentials(
        api_id: u32,
        api_hash: &str,
        port: u16,
    ) -> Result<Self, TelegramError> {
        let port = if port == 0 {
            isolated_loopback_port()?
        } else {
            port
        };
        let executable = std::env::var_os("OBS_TELEGRAM_BOT_API_PATH")
            .map(PathBuf::from)
            .unwrap_or_else(Self::packaged_executable_path);
        let directories = Self::prepare_user_directories()?;
        let child = Command::new(executable)
            .args(Self::command_arguments(port, &directories))
            .env("TELEGRAM_API_ID", api_id.to_string())
            .env("TELEGRAM_API_HASH", api_hash)
            .current_dir(directories.data_dir())
            .spawn()
            .map_err(|_| TelegramError::RequestFailed)?;
        let mut server = Self { child, port };
        server.wait_until_ready()?;
        Ok(server)
    }

    pub fn command_arguments(port: u16, directories: &BotApiDirectories) -> Vec<String> {
        vec![
            "--local".to_owned(),
            "--http-ip-address".to_owned(),
            "127.0.0.1".to_owned(),
            "--http-port".to_owned(),
            port.to_string(),
            "--dir".to_owned(),
            directories.data_dir().to_string_lossy().into_owned(),
            "--temp-dir".to_owned(),
            directories.temp_dir().to_string_lossy().into_owned(),
        ]
    }

    pub async fn start_if_configured(port: u16) -> Result<Option<Self>, TelegramError> {
        let Some(config) = SecretStore::new()
            .and_then(|store| store.load())
            .map_err(|_| TelegramError::ConfigurationUnavailable)?
        else {
            return Ok(None);
        };
        Self::start_after_cloud_logout(&config, port)
            .await
            .map(Some)
    }

    pub fn endpoint(&self) -> String {
        format!("http://127.0.0.1:{}", self.port)
    }

    async fn validate_bot(&self, bot_token: &secrecy::SecretString) -> Result<(), TelegramError> {
        Self::validate_bot_at(bot_token, &self.endpoint()).await
    }

    pub async fn validate_bot_at(
        bot_token: &secrecy::SecretString,
        endpoint: &str,
    ) -> Result<(), TelegramError> {
        let url = format!(
            "{}/bot{}/getMe",
            endpoint.trim_end_matches('/'),
            bot_token.expose_secret()
        );
        for attempt in 0..20 {
            let valid = match Client::new().get(&url).send().await {
                Ok(response) if response.status().is_success() => {
                    response
                        .json::<Value>()
                        .await
                        .ok()
                        .and_then(|body| body.get("ok").and_then(Value::as_bool))
                        == Some(true)
                }
                _ => false,
            };
            if valid {
                return Ok(());
            }
            if attempt < 19 {
                tokio::time::sleep(Duration::from_millis(100)).await;
            }
        }
        Err(TelegramError::RequestFailed)
    }

    pub fn ensure_ready(&mut self) -> Result<(), TelegramError> {
        if self
            .child
            .try_wait()
            .map_err(|_| TelegramError::RequestFailed)?
            .is_some()
        {
            return Err(TelegramError::RequestFailed);
        }
        TcpStream::connect_timeout(
            &SocketAddr::from((Ipv4Addr::LOCALHOST, self.port)),
            Duration::from_millis(100),
        )
        .map_err(|_| TelegramError::RequestFailed)?;
        #[cfg(unix)]
        {
            let output = Command::new("lsof")
                .arg("-nP")
                .arg(format!("-iTCP:{}", self.port))
                .arg("-sTCP:LISTEN")
                .arg("-Fp")
                .output()
                .map_err(|_| TelegramError::RequestFailed)?;
            if !output.status.success()
                || !Self::listener_is_owned_by(
                    self.child.id(),
                    &String::from_utf8_lossy(&output.stdout),
                )
            {
                return Err(TelegramError::RequestFailed);
            }
        }
        Ok(())
    }

    pub fn listener_is_owned_by(child_pid: u32, lsof_output: &str) -> bool {
        let pids: Vec<u32> = lsof_output
            .lines()
            .filter_map(|line| line.strip_prefix('p'))
            .filter_map(|pid| pid.parse::<u32>().ok())
            .collect();
        !pids.is_empty() && pids.into_iter().all(|pid| pid == child_pid)
    }

    fn wait_until_ready(&mut self) -> Result<(), TelegramError> {
        for _ in 0..40 {
            if self.ensure_ready().is_ok() {
                return Ok(());
            }
            if self.child.try_wait().ok().flatten().is_some() {
                return Err(TelegramError::RequestFailed);
            }
            std::thread::sleep(Duration::from_millis(50));
        }
        Err(TelegramError::RequestFailed)
    }
}

/// Call Telegram's hosted `logOut` endpoint before creating the local server.
/// The endpoint is an argument so tests can exercise the transition entirely
/// against a loopback fake; production always supplies Telegram's cloud URL.
pub async fn migrate_bot_to_local<T, Start, StartFuture>(
    config: &TelegramConfig,
    cloud_endpoint: &str,
    start_local_server: Start,
) -> Result<T, TelegramError>
where
    Start: FnOnce() -> StartFuture,
    StartFuture: Future<Output = Result<T, TelegramError>>,
{
    let marker_directory = migration_marker_directory()?;
    migrate_bot_token_to_local_at(
        &config.bot_token,
        cloud_endpoint,
        &marker_directory,
        start_local_server,
    )
    .await
}

pub async fn migrate_bot_to_local_at<T, Start, StartFuture>(
    config: &TelegramConfig,
    cloud_endpoint: &str,
    marker_directory: &Path,
    start_local_server: Start,
) -> Result<T, TelegramError>
where
    Start: FnOnce() -> StartFuture,
    StartFuture: Future<Output = Result<T, TelegramError>>,
{
    migrate_bot_token_to_local_at(
        &config.bot_token,
        cloud_endpoint,
        marker_directory,
        start_local_server,
    )
    .await
}

async fn migrate_bot_token_to_local<T, Start, StartFuture>(
    bot_token: &secrecy::SecretString,
    cloud_endpoint: &str,
    start_local_server: Start,
) -> Result<T, TelegramError>
where
    Start: FnOnce() -> StartFuture,
    StartFuture: Future<Output = Result<T, TelegramError>>,
{
    let marker_directory = migration_marker_directory()?;
    migrate_bot_token_to_local_at(
        bot_token,
        cloud_endpoint,
        &marker_directory,
        start_local_server,
    )
    .await
}

async fn migrate_bot_token_to_local_at<T, Start, StartFuture>(
    bot_token: &secrecy::SecretString,
    cloud_endpoint: &str,
    marker_directory: &Path,
    start_local_server: Start,
) -> Result<T, TelegramError>
where
    Start: FnOnce() -> StartFuture,
    StartFuture: Future<Output = Result<T, TelegramError>>,
{
    ensure_private_directory(marker_directory).map_err(|_| TelegramError::RequestFailed)?;
    let marker = marker_directory.join(token_fingerprint(bot_token));
    if marker.is_file() {
        return start_local_server().await;
    }

    let endpoint = cloud_endpoint.trim_end_matches('/');
    let cloud_logout_succeeded = Client::new()
        .post(format!(
            "{endpoint}/bot{}/logOut",
            bot_token.expose_secret()
        ))
        .send()
        .await
        .ok()
        .filter(|response| response.status().is_success());
    let cloud_logout_succeeded = match cloud_logout_succeeded {
        Some(response) => {
            response
                .json::<Value>()
                .await
                .ok()
                .and_then(|body| body.get("ok").and_then(Value::as_bool))
                == Some(true)
        }
        None => false,
    };
    if cloud_logout_succeeded {
        write_migration_marker(&marker)?;
        return start_local_server().await;
    }

    let result = start_local_server().await;
    result
}

fn migration_marker_directory() -> Result<PathBuf, TelegramError> {
    let base_directories = BaseDirs::new().ok_or(TelegramError::RequestFailed)?;
    Ok(base_directories
        .data_local_dir()
        .join(APP_SUPPORT_DIRECTORY_NAME)
        .join(BOT_API_DATA_DIRECTORY_NAME)
        .join("cloud-migrations"))
}

fn token_fingerprint(bot_token: &secrecy::SecretString) -> String {
    Sha256::digest(bot_token.expose_secret().as_bytes())
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn write_migration_marker(path: &Path) -> Result<(), TelegramError> {
    let mut marker = private_create_truncate(path).map_err(|_| TelegramError::RequestFailed)?;
    marker
        .write_all(b"cloud-logout-completed\n")
        .map_err(|_| TelegramError::RequestFailed)?;
    Ok(())
}

impl Drop for LocalBotApiServer {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn isolated_loopback_port() -> Result<u16, TelegramError> {
    let listener = std::net::TcpListener::bind((Ipv4Addr::LOCALHOST, 0))
        .map_err(|_| TelegramError::RequestFailed)?;
    listener
        .local_addr()
        .map(|address| address.port())
        .map_err(|_| TelegramError::RequestFailed)
}

fn matching_start_chat(update: &Value, nonce: &str) -> Option<i64> {
    let message = update.get("message")?;
    let text = message.get("text")?.as_str()?;
    if text == format!("/start {nonce}") {
        message.pointer("/chat/id")?.as_i64()
    } else {
        None
    }
}
