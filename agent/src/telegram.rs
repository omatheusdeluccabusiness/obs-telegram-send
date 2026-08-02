use std::{
    net::{Ipv4Addr, SocketAddr, TcpStream},
    path::PathBuf,
    process::{Child, Command},
    time::Duration,
};

use async_trait::async_trait;
use reqwest::{
    multipart::{Form, Part},
    Client,
};
use secrecy::ExposeSecret;
use serde_json::Value;
use tokio::io::AsyncReadExt;
use tokio_util::io::ReaderStream;

use crate::{
    config::{TelegramConfig, TelegramCredentials},
    jobs::MAX_FILE_SIZE_BYTES,
    keychain::SecretStore,
};

pub const DEFAULT_BOT_API_ENDPOINT: &str = "http://127.0.0.1:8081";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TelegramError {
    ConfigurationUnavailable,
    ChatNotFound,
    FileTooLarge,
    RequestFailed,
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
    async fn upload_file(&self, path: PathBuf, display_name: String) -> Result<(), String>;
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
        self.client
            .post(self.url(config.bot_token.expose_secret(), "sendMessage")?)
            .form(&[
                ("chat_id", config.chat_id.to_string()),
                ("text", message.to_owned()),
            ])
            .send()
            .await
            .and_then(reqwest::Response::error_for_status)
            .map(|_| ())
            .map_err(|_| TelegramError::RequestFailed)
    }

    pub async fn upload_file(
        &self,
        path: PathBuf,
        display_name: String,
    ) -> Result<(), TelegramError> {
        let config = self.configuration()?;
        let is_mp4 = path
            .extension()
            .and_then(|extension| extension.to_str())
            .is_some_and(|extension| extension.eq_ignore_ascii_case("mp4"));
        let field = if is_mp4 { "video" } else { "document" };
        let method = if is_mp4 { "sendVideo" } else { "sendDocument" };
        let file = tokio::fs::File::open(&path)
            .await
            .map_err(|_| TelegramError::RequestFailed)?;
        let size = file
            .metadata()
            .await
            .map_err(|_| TelegramError::RequestFailed)?
            .len();
        if size > MAX_FILE_SIZE_BYTES {
            return Err(TelegramError::FileTooLarge);
        }
        let part = Part::stream_with_length(
            reqwest::Body::wrap_stream(ReaderStream::new(file.take(size))),
            size,
        )
        .file_name(display_name);
        let form = Form::new()
            .text("chat_id", config.chat_id.to_string())
            .part(field, part);

        self.client
            .post(self.url(config.bot_token.expose_secret(), method)?)
            .multipart(form)
            .send()
            .await
            .and_then(reqwest::Response::error_for_status)
            .map(|_| ())
            .map_err(|_| TelegramError::RequestFailed)
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
    async fn upload_file(&self, path: PathBuf, display_name: String) -> Result<(), String> {
        self.upload_file(path, display_name)
            .await
            .map_err(|error| error.to_string())
    }
}

pub struct LocalBotApiServer {
    child: Child,
    port: u16,
}

impl LocalBotApiServer {
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
        let child = Command::new("telegram-bot-api")
            .args(Self::command_arguments(port))
            .env("TELEGRAM_API_ID", api_id.to_string())
            .env("TELEGRAM_API_HASH", api_hash)
            .spawn()
            .map_err(|_| TelegramError::RequestFailed)?;
        let mut server = Self { child, port };
        server.wait_until_ready()?;
        Ok(server)
    }

    pub fn command_arguments(port: u16) -> Vec<String> {
        vec![
            "--local".to_owned(),
            "--http-ip-address".to_owned(),
            "127.0.0.1".to_owned(),
            "--http-port".to_owned(),
            port.to_string(),
        ]
    }

    pub fn start_if_configured(port: u16) -> Result<Option<Self>, TelegramError> {
        let Some(config) = SecretStore::new()
            .and_then(|store| store.load())
            .map_err(|_| TelegramError::ConfigurationUnavailable)?
        else {
            return Ok(None);
        };
        Self::start(&config, port).map(Some)
    }

    pub fn endpoint(&self) -> String {
        format!("http://127.0.0.1:{}", self.port)
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
