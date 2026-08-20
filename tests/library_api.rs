use std::net::SocketAddr;
use std::time::Duration;

use tftp_rs::server::{ServerConfig, ServerEvent, run};
use tokio::net::UdpSocket;
use tokio::sync::{mpsc, watch};

#[tokio::test]
async fn rejects_a_wildcard_listener_address() {
    let dir = tempfile::tempdir().expect("temporary directory");
    let (events, _event_rx) = mpsc::unbounded_channel();
    let (_shutdown_tx, shutdown_rx) = watch::channel(false);

    let error = run(
        SocketAddr::from(([0, 0, 0, 0], 0)),
        dir.path().to_path_buf(),
        events,
        shutdown_rx,
        ServerConfig::default(),
    )
    .await
    .expect_err("wildcard binds must be rejected");

    assert!(error.to_string().contains("wildcard bind address"));
}

#[tokio::test]
async fn serves_from_the_explicit_listener_address() {
    let dir = tempfile::tempdir().expect("temporary directory");
    tokio::fs::write(dir.path().join("hello.txt"), b"AROS")
        .await
        .expect("test file");

    let reservation = UdpSocket::bind("127.0.0.1:0")
        .await
        .expect("reserve test port");
    let listener_address = reservation.local_addr().expect("listener address");
    drop(reservation);

    let (events, mut event_rx) = mpsc::unbounded_channel();
    let (shutdown_tx, shutdown_rx) = watch::channel(false);
    let server_dir = dir.path().to_path_buf();
    let server = tokio::spawn(async move {
        run(
            listener_address,
            server_dir,
            events,
            shutdown_rx,
            ServerConfig::default(),
        )
        .await
    });

    wait_until_listening(&mut event_rx).await;

    let client = UdpSocket::bind("127.0.0.1:0").await.expect("client socket");
    client
        .send_to(&rrq("hello.txt"), listener_address)
        .await
        .expect("RRQ");

    let mut buffer = [0_u8; 516];
    let (length, transfer_address) =
        tokio::time::timeout(Duration::from_secs(2), client.recv_from(&mut buffer))
            .await
            .expect("TFTP response timeout")
            .expect("TFTP response");

    assert_eq!(transfer_address.ip(), listener_address.ip());
    assert_eq!(&buffer[..4], &[0, 3, 0, 1]);
    assert_eq!(&buffer[4..length], b"AROS");

    client
        .send_to(&[0, 4, 0, 1], transfer_address)
        .await
        .expect("ACK");

    shutdown_tx.send(true).expect("shutdown signal");
    server.await.expect("server task").expect("server result");
}

async fn wait_until_listening(events: &mut mpsc::UnboundedReceiver<ServerEvent>) {
    loop {
        let event = tokio::time::timeout(Duration::from_secs(2), events.recv())
            .await
            .expect("server startup timeout")
            .expect("server event channel");
        if matches!(event, ServerEvent::Log(message) if message.starts_with("Listening on ")) {
            return;
        }
    }
}

fn rrq(filename: &str) -> Vec<u8> {
    let mut request = Vec::new();
    request.extend_from_slice(&1_u16.to_be_bytes());
    request.extend_from_slice(filename.as_bytes());
    request.push(0);
    request.extend_from_slice(b"octet");
    request.push(0);
    request
}
