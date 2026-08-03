use std::{
    io::{self, Read, Write},
    net::{IpAddr, Ipv4Addr, SocketAddr, TcpStream, UdpSocket},
    process::{Child, Command},
    thread,
    time::Duration,
};

struct AgentProcess(Child);

impl Drop for AgentProcess {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

fn isolated_loopback_port() -> u16 {
    let listener = std::net::TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
    listener.local_addr().unwrap().port()
}

fn available_non_loopback_address(port: u16) -> SocketAddr {
    let socket = UdpSocket::bind((Ipv4Addr::UNSPECIFIED, 0)).unwrap();
    socket.connect("192.0.2.1:9").unwrap();

    let IpAddr::V4(address) = socket.local_addr().unwrap().ip() else {
        panic!("test environment did not provide an IPv4 address");
    };

    assert!(!address.is_loopback());
    assert!(!address.is_unspecified());
    SocketAddr::from((address, port))
}

fn wait_for_loopback_listener(port: u16) {
    for _ in 0..20 {
        if TcpStream::connect_timeout(
            &SocketAddr::from((Ipv4Addr::LOCALHOST, port)),
            Duration::from_millis(100),
        )
        .is_ok()
        {
            return;
        }
        thread::sleep(Duration::from_millis(100));
    }

    panic!("agent did not start a loopback listener");
}

fn health_response(port: u16) -> String {
    let mut stream = TcpStream::connect_timeout(
        &SocketAddr::from((Ipv4Addr::LOCALHOST, port)),
        Duration::from_millis(500),
    )
    .unwrap();
    stream
        .write_all(b"GET /health HTTP/1.1\r\nHost: 127.0.0.1\r\nConnection: close\r\n\r\n")
        .unwrap();

    let mut response = String::new();
    stream.read_to_string(&mut response).unwrap();
    response
}

#[test]
fn binary_accepts_loopback_and_rejects_a_non_loopback_address() {
    let port = isolated_loopback_port();
    let mut agent = AgentProcess(
        Command::new(env!("CARGO_BIN_EXE_obs-telegram-agent"))
            .env("OBS_TELEGRAM_AGENT_PORT", port.to_string())
            .env("OBS_TELEGRAM_AGENT_SKIP_AUTOSTART", "1")
            .spawn()
            .unwrap(),
    );
    wait_for_loopback_listener(port);
    assert!(agent.0.try_wait().unwrap().is_none());

    let response = health_response(port);
    assert!(response.starts_with("HTTP/1.1 200"));
    assert!(response.contains("\"status\":\"ok\""));
    assert!(response.contains("\"version\":\"0.1.0\""));

    let error = TcpStream::connect_timeout(
        &available_non_loopback_address(port),
        Duration::from_millis(500),
    )
    .unwrap_err();

    assert!(
        matches!(
            error.kind(),
            io::ErrorKind::ConnectionRefused
                | io::ErrorKind::TimedOut
                | io::ErrorKind::AddrNotAvailable
        ),
        "non-loopback connection must be rejected: {error}"
    );
}
