//! Integration tests for helper bridge token enforcement.

use std::time::Duration;

use codex_assistant_core::helper_auth::ensure_helper_token;
use codex_assistant_core::launcher::test_support::{
    HelperHandle, shutdown_helper_listener, spawn_helper_listener,
};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

async fn send_raw(port: u16, raw: &str) -> String {
    let mut stream = TcpStream::connect(("127.0.0.1", port))
        .await
        .expect("connect");
    stream.write_all(raw.as_bytes()).await.expect("write");
    let mut buf = Vec::new();
    tokio::time::timeout(Duration::from_secs(2), stream.read_to_end(&mut buf))
        .await
        .expect("read timeout")
        .expect("read");
    String::from_utf8_lossy(&buf).into_owned()
}

#[tokio::test]
async fn missing_token_returns_401() {
    let HelperHandle { port, shutdown } = spawn_helper_listener().await;
    let response = send_raw(
        port,
        "POST /backend/status HTTP/1.1\r\nHost: 127.0.0.1\r\nContent-Length: 0\r\n\r\n",
    )
    .await;
    assert!(response.starts_with("HTTP/1.1 401"), "got: {response}");
    shutdown_helper_listener(shutdown).await;
}

#[tokio::test]
async fn wrong_token_returns_401() {
    let HelperHandle { port, shutdown } = spawn_helper_listener().await;
    let raw = "POST /backend/status HTTP/1.1\r\nHost: 127.0.0.1\r\nX-Codex-Helper-Token: not-a-real-token\r\nContent-Length: 0\r\n\r\n";
    let response = send_raw(port, raw).await;
    assert!(response.starts_with("HTTP/1.1 401"), "got: {response}");
    shutdown_helper_listener(shutdown).await;
}

#[tokio::test]
async fn correct_token_returns_200() {
    let HelperHandle { port, shutdown } = spawn_helper_listener().await;
    let token = ensure_helper_token();
    let raw = format!(
        "POST /backend/status HTTP/1.1\r\nHost: 127.0.0.1\r\nX-Codex-Helper-Token: {token}\r\nContent-Length: 0\r\n\r\n"
    );
    let response = send_raw(port, &raw).await;
    assert!(response.starts_with("HTTP/1.1 200"), "got: {response}");
    shutdown_helper_listener(shutdown).await;
}

#[tokio::test]
async fn options_preflight_no_token_returns_204() {
    let HelperHandle { port, shutdown } = spawn_helper_listener().await;
    let raw = "OPTIONS /backend/status HTTP/1.1\r\nHost: 127.0.0.1\r\nOrigin: https://chatgpt.com\r\nAccess-Control-Request-Method: POST\r\n\r\n";
    let response = send_raw(port, raw).await;
    assert!(response.starts_with("HTTP/1.1 204"), "got: {response}");
    assert!(
        response.contains(
            "Access-Control-Allow-Headers: Content-Type, Authorization, X-Codex-Helper-Token"
        ),
        "missing token in Allow-Headers: {response}"
    );
    shutdown_helper_listener(shutdown).await;
}
