use axum::{
    extract::State,
    http::StatusCode,
    routing::{get, post},
    Json, Router,
};
use obs_telegram_agent::{
    config::{validate_config, ConfigInput},
    telegram::{migrate_bot_to_local, LocalBotApiServer, TelegramGateway},
};
use serde_json::{json, Value};
use std::os::unix::fs::PermissionsExt;
use tokio::net::TcpListener;

async fn fake_updates() -> Json<Value> {
    Json(json!({"ok": true, "result": [{"message": {"chat": {"id": -10012345}}}]}))
}

async fn capture_upload(
    State(methods): State<std::sync::Arc<std::sync::Mutex<Vec<String>>>>,
    request: axum::extract::Request,
) -> StatusCode {
    methods
        .lock()
        .unwrap()
        .push(request.uri().path().to_owned());
    StatusCode::OK
}

#[tokio::test]
async fn fake_local_endpoint_receives_video_for_mp4_and_document_for_other_files() {
    let methods = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
    let app = Router::new()
        .route("/bot123:token/getUpdates", get(fake_updates))
        .route("/bot123:token/sendVideo", post(capture_upload))
        .route("/bot123:token/sendDocument", post(capture_upload))
        .with_state(methods.clone());
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });

    let config = validate_config(ConfigInput {
        bot_token: "123:token".to_owned(),
        api_id: 123,
        api_hash: "0123456789abcdef0123456789abcdef".to_owned(),
        chat_id: -10012345,
    })
    .unwrap();
    let gateway = TelegramGateway::with_endpoint(config, format!("http://{address}"));
    let mp4 = tempfile::NamedTempFile::with_suffix(".mp4").unwrap();
    let mkv = tempfile::NamedTempFile::with_suffix(".mkv").unwrap();

    gateway
        .upload_file(mp4.path().into(), "video.mp4".to_owned())
        .await
        .unwrap();
    gateway
        .upload_file(mkv.path().into(), "video.mkv".to_owned())
        .await
        .unwrap();

    assert_eq!(
        *methods.lock().unwrap(),
        vec!["/bot123:token/sendVideo", "/bot123:token/sendDocument"]
    );
}

#[test]
fn local_bot_api_arguments_force_an_isolated_loopback_listener() {
    let application_support = tempfile::tempdir().unwrap();
    let directories =
        LocalBotApiServer::prepare_directories_at(application_support.path()).unwrap();
    let arguments = LocalBotApiServer::command_arguments(41723, &directories);

    assert!(arguments
        .windows(2)
        .any(|pair| pair == ["--http-ip-address", "127.0.0.1"]));
    assert!(arguments
        .windows(2)
        .any(|pair| pair == ["--http-port", "41723"]));
    assert!(arguments
        .windows(2)
        .any(|pair| { pair[0] == "--dir" && pair[1] == directories.data_dir().to_string_lossy() }));
    assert!(arguments.windows(2).any(|pair| {
        pair[0] == "--temp-dir" && pair[1] == directories.temp_dir().to_string_lossy()
    }));
}

#[test]
fn local_bot_api_directories_are_private_and_user_writable() {
    let temporary_root = tempfile::tempdir().unwrap();
    let application_support = temporary_root.path().join("OBS-Telegram-Send");

    let directories = LocalBotApiServer::prepare_directories_at(&application_support).unwrap();

    assert_eq!(
        directories.data_dir(),
        application_support.join("telegram-bot-api")
    );
    assert_eq!(
        directories.temp_dir(),
        application_support.join("telegram-bot-api/temp")
    );
    for directory in [
        application_support.as_path(),
        directories.data_dir(),
        directories.temp_dir(),
    ] {
        assert!(directory.is_dir());
        assert_eq!(
            std::fs::metadata(directory).unwrap().permissions().mode() & 0o777,
            0o700
        );
    }
}

#[test]
fn packaged_server_path_does_not_depend_on_the_shell_path() {
    assert_eq!(
        LocalBotApiServer::packaged_executable_path(),
        std::path::Path::new("/Library/Application Support/OBS-Telegram-Send/telegram-bot-api")
    );
}

#[tokio::test]
async fn chat_detection_ignores_an_unrelated_newer_update_without_its_start_nonce() {
    let config = validate_config(ConfigInput {
        bot_token: "123:token".to_owned(),
        api_id: 123,
        api_hash: "0123456789abcdef0123456789abcdef".to_owned(),
        chat_id: -10012345,
    })
    .unwrap();
    let app = Router::new().route(
        "/bot123:token/getUpdates",
        get(|| async {
            Json(json!({"ok": true, "result": [
                {"message": {"text": "/start accepted", "chat": {"id": 42}}},
                {"message": {"text": "/start unrelated", "chat": {"id": 99}}}
            ]}))
        }),
    );
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    let gateway = TelegramGateway::with_endpoint(config, format!("http://{address}"));

    assert_eq!(gateway.detect_chat_for_nonce("accepted").await.unwrap(), 42);
    assert!(gateway.detect_chat_for_nonce("missing").await.is_err());
}

#[tokio::test]
async fn logs_out_from_the_cloud_before_starting_the_local_server() {
    let calls = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
    let app = Router::new().route(
        "/bot123:token/logOut",
        post({
            let calls = calls.clone();
            move || {
                let calls = calls.clone();
                async move {
                    calls.lock().unwrap().push("logOut".to_owned());
                    Json(json!({"ok": true, "result": true}))
                }
            }
        }),
    );
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });

    let config = validate_config(ConfigInput {
        bot_token: "123:token".to_owned(),
        api_id: 123,
        api_hash: "0123456789abcdef0123456789abcdef".to_owned(),
        chat_id: -10012345,
    })
    .unwrap();
    let started = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let started_by_server = started.clone();

    migrate_bot_to_local(&config, &format!("http://{address}"), move || {
        started_by_server.store(true, std::sync::atomic::Ordering::SeqCst);
        Ok(())
    })
    .await
    .unwrap();

    assert_eq!(*calls.lock().unwrap(), vec!["logOut"]);
    assert!(started.load(std::sync::atomic::Ordering::SeqCst));
}

#[test]
fn managed_gateway_keeps_the_exact_server_handle_it_was_given() {
    let server = std::sync::Arc::new(std::sync::Mutex::new(None));
    let gateway = TelegramGateway::from_keychain_with_server(server.clone());

    assert!(gateway.uses_server_handle(&server));
}

#[test]
fn only_the_child_process_id_is_accepted_as_the_loopback_listener_owner() {
    assert!(LocalBotApiServer::listener_is_owned_by(4242, "p4242\n"));
    assert!(!LocalBotApiServer::listener_is_owned_by(4242, "p9999\n"));
    assert!(!LocalBotApiServer::listener_is_owned_by(
        4242,
        "p4242\np9999\n"
    ));
}
