use axum::{routing::get, Json, Router};
use serde_json::json;

pub mod api;
pub mod config;
pub mod install_bearer;
pub mod jobs;
pub mod keychain;
pub mod telegram;

pub fn app_with_secret(install_secret: &str) -> Router {
    app_with_local_bot_api(install_secret, None)
}

pub fn app_with_local_bot_api(
    install_secret: &str,
    local_bot_api: Option<telegram::LocalBotApiServer>,
) -> Router {
    Router::new()
        .route(
            "/health",
            get(|| async { Json(json!({"version": "0.1.0", "status": "ok"})) }),
        )
        .merge(api::router_with_local_bot_api(
            install_secret.to_owned(),
            local_bot_api,
        ))
}
