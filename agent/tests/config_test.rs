use obs_telegram_agent::config::{validate_config, ConfigError, ConfigInput};

fn input(bot_token: &str, chat_id: i64) -> ConfigInput {
    ConfigInput {
        bot_token: bot_token.to_owned(),
        api_id: 123_456,
        api_hash: "0123456789abcdef0123456789abcdef".to_owned(),
        chat_id,
    }
}

#[test]
fn rejects_a_token_without_the_botfather_separator() {
    let secret_token = "bot-token-that-must-not-appear-in-errors";

    let error = match validate_config(input(secret_token, 12345)) {
        Err(error) => error,
        Ok(_) => panic!("a token without a separator must be rejected"),
    };

    assert_eq!(error, ConfigError::InvalidBotToken);
    assert!(!format!("{error:?}").contains(secret_token));
}

#[test]
fn rejects_a_bot_token_without_a_numeric_identifier() {
    assert!(matches!(
        validate_config(input(":abc", 12345)),
        Err(ConfigError::InvalidBotToken)
    ));
}

#[test]
fn rejects_a_bot_token_without_a_suffix() {
    assert!(matches!(
        validate_config(input("123:", 12345)),
        Err(ConfigError::InvalidBotToken)
    ));
}

#[test]
fn accepts_a_private_chat_identifier() {
    assert_eq!(
        validate_config(input("123:abc", 12345)).unwrap().chat_id,
        12345
    );
}

#[test]
fn accepts_a_negative_group_chat_identifier() {
    assert_eq!(
        validate_config(input("123:abc", -10012345))
            .unwrap()
            .chat_id,
        -10012345
    );
}

#[test]
fn rejects_a_zero_chat_identifier() {
    assert!(matches!(
        validate_config(input("123:abc", 0)),
        Err(ConfigError::InvalidTelegramApp)
    ));
}

#[test]
fn rejects_a_zero_telegram_api_identifier() {
    let mut configuration = input("123:abc", 12345);
    configuration.api_id = 0;

    assert!(matches!(
        validate_config(configuration),
        Err(ConfigError::InvalidTelegramApp)
    ));
}

#[test]
fn rejects_an_empty_telegram_api_hash() {
    let mut configuration = input("123:abc", 12345);
    configuration.api_hash = " \t ".to_owned();

    assert!(matches!(
        validate_config(configuration),
        Err(ConfigError::InvalidTelegramApp)
    ));
}

#[test]
fn rejects_a_telegram_api_hash_with_non_hexadecimal_characters() {
    let mut configuration = input("123:abc", 12345);
    configuration.api_hash = "0123456789abcdef0123456789abcdeg".to_owned();

    assert!(matches!(
        validate_config(configuration),
        Err(ConfigError::InvalidTelegramApp)
    ));
}

#[test]
fn rejects_a_telegram_api_hash_with_the_wrong_length() {
    let mut configuration = input("123:abc", 12345);
    configuration.api_hash = "0123456789abcdef0123456789abcde".to_owned();

    assert!(matches!(
        validate_config(configuration),
        Err(ConfigError::InvalidTelegramApp)
    ));
}
