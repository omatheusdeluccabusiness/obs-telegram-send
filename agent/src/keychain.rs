use secrecy::ExposeSecret;
use serde_json::{json, Value};

use crate::config::{validate_config, ConfigInput, TelegramConfig};

// This is the first public storage namespace. Earlier development builds used
// an unversioned Keychain item whose ad-hoc code-signing ACL could keep a new
// build blocked behind an obsolete authorization prompt.
const SERVICE_NAME: &str = "com.obs-telegram-send.agent.v1";
const ACCOUNT_NAME: &str = "telegram-configuration";

pub struct SecretStore {
    entry: keyring::Entry,
}

#[derive(Debug, PartialEq, Eq)]
pub enum SecretStoreError {
    Unavailable,
    InvalidStoredConfiguration,
}

impl SecretStore {
    pub fn new() -> Result<Self, SecretStoreError> {
        keyring::Entry::new(SERVICE_NAME, ACCOUNT_NAME)
            .map(|entry| Self { entry })
            .map_err(|_| SecretStoreError::Unavailable)
    }

    pub fn save(&self, config: &TelegramConfig) -> Result<(), SecretStoreError> {
        let serialized = json!({
            "bot_token": config.bot_token.expose_secret(),
            "api_id": config.api_id,
            "api_hash": config.api_hash.expose_secret(),
            "chat_id": config.chat_id,
        })
        .to_string();

        self.entry
            .set_password(&serialized)
            .map_err(|_| SecretStoreError::Unavailable)
    }

    pub fn load(&self) -> Result<Option<TelegramConfig>, SecretStoreError> {
        let serialized = match self.entry.get_password() {
            Ok(value) => value,
            Err(keyring::Error::NoEntry) => return Ok(None),
            Err(_) => return Err(SecretStoreError::Unavailable),
        };

        stored_config(&serialized)
            .map(Some)
            .ok_or(SecretStoreError::InvalidStoredConfiguration)
    }

    pub fn clear(&self) -> Result<(), SecretStoreError> {
        match self.entry.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(_) => Err(SecretStoreError::Unavailable),
        }
    }
}

fn stored_config(serialized: &str) -> Option<TelegramConfig> {
    let value: Value = serde_json::from_str(serialized).ok()?;
    let input = ConfigInput {
        bot_token: value.get("bot_token")?.as_str()?.to_owned(),
        api_id: value.get("api_id")?.as_u64()?.try_into().ok()?,
        api_hash: value.get("api_hash")?.as_str()?.to_owned(),
        chat_id: value.get("chat_id")?.as_i64()?,
    };

    validate_config(input).ok()
}
