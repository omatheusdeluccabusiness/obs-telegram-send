use secrecy::SecretString;
use serde::Deserialize;

#[derive(Debug, PartialEq, Eq)]
pub enum ConfigError {
    InvalidBotToken,
    InvalidTelegramApp,
}

#[derive(Deserialize)]
pub struct ConfigInput {
    pub bot_token: String,
    pub api_id: u32,
    pub api_hash: String,
    pub chat_id: i64,
}

pub struct TelegramConfig {
    pub bot_token: SecretString,
    pub api_id: u32,
    pub api_hash: SecretString,
    pub chat_id: i64,
}

impl From<ConfigInput> for TelegramConfig {
    fn from(input: ConfigInput) -> Self {
        Self {
            bot_token: SecretString::from(input.bot_token),
            api_id: input.api_id,
            api_hash: SecretString::from(input.api_hash),
            chat_id: input.chat_id,
        }
    }
}

pub fn validate_config(input: ConfigInput) -> Result<TelegramConfig, ConfigError> {
    let valid_bot_token = input
        .bot_token
        .split_once(':')
        .is_some_and(|(identifier, suffix)| {
            !identifier.is_empty()
                && identifier.bytes().all(|byte| byte.is_ascii_digit())
                && !suffix.is_empty()
        });
    if !valid_bot_token {
        return Err(ConfigError::InvalidBotToken);
    }

    let valid_api_hash =
        input.api_hash.len() == 32 && input.api_hash.bytes().all(|byte| byte.is_ascii_hexdigit());
    if input.api_id == 0 || input.chat_id == 0 || !valid_api_hash {
        return Err(ConfigError::InvalidTelegramApp);
    }

    Ok(input.into())
}
