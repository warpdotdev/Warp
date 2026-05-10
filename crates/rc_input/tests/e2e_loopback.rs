//! End-to-end loopback test: bind a listener, connect a client over the same
//! socket / pipe, send messages, assert they arrive on the consumer channel.
//!
//! Unix-only for v1. The Windows named-pipe variant has the same semantics
//! but extra path-construction quirks; we'll add a Windows-specific test
//! once the listener is wired into Warp's shared_session.

#![cfg(unix)]

use async_compat::{Compat, CompatExt};
use interprocess::local_socket::tokio::LocalSocketStream;
use rc_input::{InputMsg, MessageKind, Server};
use std::time::Duration;
use tokio::io::AsyncWriteExt;
use tokio::sync::mpsc;

async fn write_line(stream: &mut Compat<impl futures::AsyncWrite + Unpin>, line: &str) {
    stream.write_all(line.as_bytes()).await.unwrap();
    stream.write_all(b"\n").await.unwrap();
    stream.flush().await.unwrap();
}

#[tokio::test]
async fn delivers_text_message() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("rc_input.sock");
    let addr = path.to_string_lossy().into_owned();

    let (tx, mut rx) = mpsc::channel::<InputMsg>(8);
    let server = Server::bind(&addr, tx).unwrap();
    tokio::spawn(server.run());

    // Yield once so the listener is in accept().
    tokio::task::yield_now().await;

    let stream = LocalSocketStream::connect(addr.as_str())
        .compat()
        .await
        .unwrap();
    let (_read, write) = stream.into_split();
    let mut write = Compat::new(write);
    write_line(
        &mut write,
        r#"{"kind":"text","value":"hello\n","client_id":"c1","ts":"t"}"#,
    )
    .await;

    let msg = tokio::time::timeout(Duration::from_secs(2), rx.recv())
        .await
        .expect("timeout")
        .expect("channel closed");

    assert_eq!(msg.kind, MessageKind::Text);
    assert_eq!(msg.value, "hello\n");
    assert_eq!(msg.client_id, "c1");
}

#[tokio::test]
async fn delivers_slash_command() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("rc_input.sock");
    let addr = path.to_string_lossy().into_owned();

    let (tx, mut rx) = mpsc::channel::<InputMsg>(8);
    let server = Server::bind(&addr, tx).unwrap();
    tokio::spawn(server.run());
    tokio::task::yield_now().await;

    let stream = LocalSocketStream::connect(addr.as_str())
        .compat()
        .await
        .unwrap();
    let (_read, write) = stream.into_split();
    let mut write = Compat::new(write);
    write_line(&mut write, r#"{"kind":"slash","value":"/mcp"}"#).await;

    let msg = tokio::time::timeout(Duration::from_secs(2), rx.recv())
        .await
        .expect("timeout")
        .expect("channel closed");

    assert_eq!(msg.kind, MessageKind::Slash);
    assert_eq!(msg.value, "/mcp");
}

#[tokio::test]
async fn skips_invalid_json_but_keeps_connection_open() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("rc_input.sock");
    let addr = path.to_string_lossy().into_owned();

    let (tx, mut rx) = mpsc::channel::<InputMsg>(8);
    let server = Server::bind(&addr, tx).unwrap();
    tokio::spawn(server.run());
    tokio::task::yield_now().await;

    let stream = LocalSocketStream::connect(addr.as_str())
        .compat()
        .await
        .unwrap();
    let (_read, write) = stream.into_split();
    let mut write = Compat::new(write);
    write_line(&mut write, "this is not json").await;
    write_line(&mut write, r#"{"kind":"text","value":"after-bad"}"#).await;

    let msg = tokio::time::timeout(Duration::from_secs(2), rx.recv())
        .await
        .expect("timeout")
        .expect("channel closed");

    assert_eq!(msg.value, "after-bad");
}
