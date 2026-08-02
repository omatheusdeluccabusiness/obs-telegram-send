use axum::{routing::get, Json, Router};
use rand::RngCore;
use serde_json::json;

pub mod api;
pub mod config;
pub mod keychain;

pub fn app() -> Router {
    app_with_secret(&generated_install_secret())
}

pub fn app_with_secret(install_secret: &str) -> Router {
    Router::new()
        .route(
            "/health",
            get(|| async { Json(json!({"version": "0.1.0", "status": "ok"})) }),
        )
        .merge(api::router(install_secret.to_owned()))
}

fn generated_install_secret() -> String {
    let mut bytes = [0_u8; 32];
    rand::rngs::OsRng.fill_bytes(&mut bytes);
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}
