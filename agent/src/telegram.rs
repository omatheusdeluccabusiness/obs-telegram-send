use std::{
    path::PathBuf,
    process::{Child, Command},
};

use async_trait::async_trait;
use reqwest::{
    multipart::{Form, Part},
    Client,
};
use secrecy::ExposeSecret;
use serde_json::Value;
use tokio_util::io::ReaderStream;

use crate::{config::TelegramConfig, keychain::SecretStore};

pub const DEFAULT_BOT_API_ENDPOINT: &str = "http://127.0.0.1:8081";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TelegramError {
    ConfigurationUnavailable,
    ChatNotFound,
    RequestFailed,
}

impl std::fmt::Display for TelegramError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let message = match self {
            Self::ConfigurationUnavailable => "Telegram is not configured.",
            Self::ChatNotFound => "No Telegram chat was found. Send /start to the bot first.",
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
}

#[derive(Clone)]
pub struct TelegramGateway {
    client: Client,
    endpoint: String,
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
            endpoint: endpoint.into().trim_end_matches('/').to_owned(),
            credentials: CredentialSource::Keychain,
        }
    }

    pub fn with_endpoint(config: TelegramConfig, endpoint: impl Into<String>) -> Self {
        Self {
            client: Client::new(),
            endpoint: endpoint.into().trim_end_matches('/').to_owned(),
            credentials: CredentialSource::Supplied(std::sync::Arc::new(config)),
        }
    }

    pub async fn detect_chat(&self) -> Result<i64, TelegramError> {
        let config = self.configuration()?;
        let response = self
            .client
            .get(self.url(config.bot_token.expose_secret(), "getUpdates"))
            .send()
            .await
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
            .rev()
            .find_map(|update| {
                update
                    .pointer("/message/chat/id")
                    .or_else(|| update.pointer("/channel_post/chat/id"))
                    .and_then(Value::as_i64)
            })
            .ok_or(TelegramError::ChatNotFound)
    }

    pub async fn test_send(&self, message: &str) -> Result<(), TelegramError> {
        let config = self.configuration()?;
        self.client
            .post(self.url(config.bot_token.expose_secret(), "sendMessage"))
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
        let part =
            Part::stream_with_length(reqwest::Body::wrap_stream(ReaderStream::new(file)), size)
                .file_name(display_name);
        let form = Form::new()
            .text("chat_id", config.chat_id.to_string())
            .part(field, part);

        self.client
            .post(self.url(config.bot_token.expose_secret(), method))
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
            CredentialSource::Keychain => SecretStore::new()
                .and_then(|store| store.load())
                .ok()
                .flatten()
                .map(std::sync::Arc::new)
                .ok_or(TelegramError::ConfigurationUnavailable),
        }
    }

    fn url(&self, bot_token: &str, method: &str) -> String {
        format!("{}/bot{bot_token}/{method}", self.endpoint)
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
}

impl LocalBotApiServer {
    pub fn start(config: &TelegramConfig, port: u16) -> Result<Self, TelegramError> {
        let child = Command::new("telegram-bot-api")
            .arg("--local")
            .arg("--http-port")
            .arg(port.to_string())
            .env("TELEGRAM_API_ID", config.api_id.to_string())
            .env("TELEGRAM_API_HASH", config.api_hash.expose_secret())
            .spawn()
            .map_err(|_| TelegramError::RequestFailed)?;
        Ok(Self { child })
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
}

impl Drop for LocalBotApiServer {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}
