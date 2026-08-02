use std::{
    io,
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

fn available_non_loopback_address() -> SocketAddr {
    let socket = UdpSocket::bind((Ipv4Addr::UNSPECIFIED, 0)).unwrap();
    socket.connect("192.0.2.1:9").unwrap();

    let IpAddr::V4(address) = socket.local_addr().unwrap().ip() else {
        panic!("test environment did not provide an IPv4 address");
    };

    assert!(!address.is_loopback());
    assert!(!address.is_unspecified());
    SocketAddr::from((address, 43127))
}

fn wait_for_loopback_listener() {
    for _ in 0..20 {
        if TcpStream::connect_timeout(
            &SocketAddr::from((Ipv4Addr::LOCALHOST, 43127)),
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

#[test]
fn binary_accepts_loopback_and_rejects_a_non_loopback_address() {
    let _agent = AgentProcess(
        Command::new(env!("CARGO_BIN_EXE_obs-telegram-agent"))
            .spawn()
            .unwrap(),
    );
    wait_for_loopback_listener();

    let error = TcpStream::connect_timeout(
        &available_non_loopback_address(),
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
