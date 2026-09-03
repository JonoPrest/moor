//! The bridge against a real daemon: a websocket client attaches and sees
//! the view reach `Subscribed` with the workspace listed; commands round-
//! trip as JSON text frames.

use std::net::{Ipv4Addr, SocketAddr};
use std::sync::Arc;
use std::time::Duration;

use futures_util::{SinkExt as _, StreamExt as _};
use nits_client_core::{IdSeed, ViewPatch};
use nits_client_host::{Identity, KvConfig, host_config};
use nits_protocol::{Author, BuildInfo, ClientId, ViewSection};
use nits_review_core::DataDir;
use nitsd::Daemon;
use nitsd::contexts::{DaemonEndpoint, StartPolicy};
use nitsd::server::UnixServer;
use tokio_tungstenite::tungstenite::Message;
use tokio_util::sync::CancellationToken;

fn identity() -> Identity {
    Identity {
        client_id: ClientId::from_parts(1, 7),
        client: BuildInfo {
            name: "bridge-test".into(),
            version: "0".into(),
        },
        author: Author::Human {
            name: "ada".into(),
            machine: "box".into(),
        },
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn browser_attach_sees_subscribed_view() {
    let dir = tempfile::tempdir().unwrap();
    let socket = dir.path().join("nitsd.sock");
    let daemon = Daemon::open(
        &DataDir::new(dir.path().join("data")),
        BuildInfo {
            name: "nitsd".into(),
            version: "test".into(),
        },
    )
    .unwrap();
    let shutdown = CancellationToken::new();
    let server = UnixServer::bind(&socket).unwrap();
    tokio::spawn(server.run(Arc::clone(&daemon), shutdown.clone()));

    let endpoint = DaemonEndpoint::resolve(
        &nits_config::Context::Local {
            data_dir: None,
            socket: Some(socket),
        },
        StartPolicy::RequireRunning,
    )
    .unwrap();
    let config = host_config(endpoint, identity(), IdSeed(42), KvConfig::Memory);
    let bridge = nits_client_web::serve(SocketAddr::from((Ipv4Addr::LOCALHOST, 0)), config)
        .await
        .unwrap();

    let (ws, _) = tokio_tungstenite::connect_async(format!("ws://{}/ws", bridge.addr()))
        .await
        .unwrap();
    let (mut tx, mut rx) = ws.split();
    tx.send(Message::Text(r#"{"cmd":"attach"}"#.into()))
        .await
        .unwrap();

    // Every frame must parse as a patch batch; wait for Subscribed.
    let deadline = tokio::time::Instant::now() + Duration::from_secs(10);
    let mut subscribed = false;
    while !subscribed {
        let msg = tokio::time::timeout_at(deadline, rx.next())
            .await
            .expect("timed out waiting for Subscribed")
            .expect("bridge closed the socket")
            .unwrap();
        let Message::Text(text) = msg else {
            panic!("expected text frame, got {msg:?}")
        };
        let patches: Vec<ViewPatch> = serde_json::from_str(&text).unwrap();
        subscribed = patches
            .iter()
            .any(|p| p.section() == ViewSection::Connection && text.contains("\"Subscribed\""));
    }

    // A bad command is logged, not fatal: the socket stays open.
    tx.send(Message::Text(r#"{"cmd":"nonsense"}"#.into()))
        .await
        .unwrap();
    tx.send(Message::Text(r#"{"cmd":"attach"}"#.into()))
        .await
        .unwrap();
    let msg = tokio::time::timeout(Duration::from_secs(5), rx.next())
        .await
        .expect("no re-attach response")
        .expect("bridge closed after bad command")
        .unwrap();
    assert!(matches!(msg, Message::Text(_)));

    // The same port serves the embedded UI over plain HTTP.
    let mut http = tokio::net::TcpStream::connect(bridge.addr()).await.unwrap();
    tokio::io::AsyncWriteExt::write_all(&mut http, b"GET /?review=abc HTTP/1.1\r\nhost: x\r\n\r\n")
        .await
        .unwrap();
    let mut body = Vec::new();
    tokio::io::AsyncReadExt::read_to_end(&mut http, &mut body)
        .await
        .unwrap();
    let text = String::from_utf8_lossy(&body);
    assert!(
        text.starts_with("HTTP/1.1 200"),
        "{}",
        &text[..40.min(text.len())]
    );
    assert!(
        text.contains("<div id=\"root\">"),
        "should serve index.html"
    );

    bridge.stop();
    shutdown.cancel();
}
