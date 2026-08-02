use tokio::net::TcpListener;

#[tokio::main]
async fn main() {
    let listener = TcpListener::bind("127.0.0.1:43127")
        .await
        .expect("agent must bind to the local loopback interface");

    axum::serve(listener, obs_telegram_agent::app())
        .await
        .expect("agent server stopped unexpectedly");
}
