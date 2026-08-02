use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use http_body_util::BodyExt;
use obs_telegram_agent::app_with_secret;
use tower::ServiceExt;

fn valid_input() -> Body {
    Body::from(
        serde_json::json!({
            "bot_token": "123:bot-token",
            "api_id": 12345,
            "api_hash": "0123456789abcdef0123456789abcdef",
            "chat_id": -10012345
        })
        .to_string(),
    )
}

fn post_json(path: &str, body: Body) -> Request<Body> {
    Request::post(path)
        .header("content-type", "application/json")
        .body(body)
        .unwrap()
}

#[tokio::test]
async fn configuration_is_rejected_without_the_install_bearer() {
    let response = app_with_secret("install-secret")
        .oneshot(post_json("/v1/config", valid_input()))
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn configuration_errors_do_not_render_credential_values() {
    let bot_token = "bot-token-that-must-not-appear-in-errors";
    let request = Request::post("/v1/config")
        .header("authorization", "Bearer install-secret")
        .header("content-type", "application/json")
        .body(Body::from(
            serde_json::json!({
                "bot_token": bot_token,
                "api_id": 12345,
                "api_hash": "0123456789abcdef0123456789abcdef",
                "chat_id": -10012345
            })
            .to_string(),
        ))
        .unwrap();

    let response = app_with_secret("install-secret")
        .oneshot(request)
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);

    let body = response.into_body().collect().await.unwrap().to_bytes();
    let rendered = String::from_utf8(body.to_vec()).unwrap();
    assert!(!rendered.contains(bot_token));
    assert!(rendered.contains("invalid_configuration"));
}
