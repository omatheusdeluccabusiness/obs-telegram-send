use std::net::Ipv4Addr;

use obs_telegram_agent::install_bearer::InstallBearerStore;
use obs_telegram_agent::telegram::LocalBotApiServer;
use secrecy::ExposeSecret;
use tokio::net::TcpListener;

const DEFAULT_PORT: u16 = 43127;

fn listener_port() -> u16 {
    std::env::var("OBS_TELEGRAM_AGENT_PORT")
        .ok()
        .and_then(|port| port.parse().ok())
        .filter(|port: &u16| *port != 0)
        .unwrap_or(DEFAULT_PORT)
}

#[tokio::main]
async fn main() {
    let install_bearer = InstallBearerStore::in_app_support()
        .and_then(|store| store.load_or_create())
        .expect("agent must access its protected loopback bearer store");
    let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, listener_port()))
        .await
        .expect("agent must bind to the local loopback interface");
    let local_bot_api = LocalBotApiServer::start_if_configured(0)
        .expect("agent must load its protected Telegram configuration");
    axum::serve(
        listener,
        obs_telegram_agent::app_with_local_bot_api(install_bearer.expose_secret(), local_bot_api),
    )
    .await
    .expect("agent server stopped unexpectedly");
}
