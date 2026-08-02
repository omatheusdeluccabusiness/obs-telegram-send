use axum::{
    extract::State,
    http::StatusCode,
    routing::{get, post},
    Json, Router,
};
use obs_telegram_agent::{
    config::{validate_config, ConfigInput},
    telegram::TelegramGateway,
};
use serde_json::{json, Value};
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
