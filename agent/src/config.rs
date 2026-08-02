use secrecy::SecretString;

#[derive(Debug, PartialEq, Eq)]
pub enum ConfigError {
    InvalidBotToken,
    InvalidTelegramApp,
}

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
    if !input.bot_token.contains(':') {
        return Err(ConfigError::InvalidBotToken);
    }

    if input.api_id == 0 || input.api_hash.trim().is_empty() {
        return Err(ConfigError::InvalidTelegramApp);
    }

    Ok(input.into())
}
