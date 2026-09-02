//! HTTP + WebSocket bridge for the browser UI (dev/test; ARCHITECTURE
//! §6.2).
//!
//! One port: `GET /…` serves the embedded `ui/dist` build, `GET /ws`
//! upgrades to the WebSocket that speaks the same contract as the Tauri
//! wrapper: commands in, `view` patch batches out. One `nits-client-host`
//! per server; every connected browser sees the same view. Messages are
//! JSON text frames:
//!
//! - in: `{"cmd":"dispatch","action":…}` | `{"cmd":"key","chord":…}` |
//!   `{"cmd":"attach"}`
//! - out: an array of `ViewPatch`
//!
//! `Attach` re-emits every section, so each frame is broadcast: a client
//! that attaches late catches up from its own attach.

use std::net::SocketAddr;

use futures_util::{SinkExt as _, StreamExt as _};
use include_dir::{Dir, include_dir};
use nits_client_core::{Action, KeyChord};
use nits_client_host::{Handle, HostConfig, HostError};
use tokio::io::{AsyncReadExt as _, AsyncWriteExt as _};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::broadcast;
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::tungstenite::handshake::derive_accept_key;
use tokio_tungstenite::tungstenite::protocol::Role;
use tokio_util::sync::CancellationToken;

/// The built browser UI, embedded at compile time (build `ui/dist` first).
static UI: Dir<'_> = include_dir!("$CARGO_MANIFEST_DIR/dist");

/// The one request head we parse: method, path, and the two headers the
/// websocket upgrade needs. Anything else is a plain asset request.
#[derive(Debug)]
struct RequestHead {
    get: bool,
    path: String,
    upgrade_websocket: bool,
    ws_key: Option<String>,
}

/// Read a request head (≤ 16 KB) off a fresh connection.
async fn read_head(stream: &mut TcpStream) -> std::io::Result<(RequestHead, Vec<u8>)> {
    let mut buf = Vec::with_capacity(1024);
    let mut byte = [0u8; 1024];
    while !buf.windows(4).any(|w| w == b"\r\n\r\n") {
        if buf.len() > 16 * 1024 {
            return Err(std::io::Error::other("request head too large"));
        }
        let n = stream.read(&mut byte).await?;
        if n == 0 {
            return Err(std::io::ErrorKind::UnexpectedEof.into());
        }
        buf.extend_from_slice(&byte[..n]);
    }
    let text = String::from_utf8_lossy(&buf);
    let mut lines = text.lines();
    let request = lines.next().unwrap_or_default();
    let mut parts = request.split_whitespace();
    let method = parts.next().unwrap_or_default();
    let path = parts.next().unwrap_or("/").to_owned();
    let mut upgrade_websocket = false;
    let mut ws_key = None;
    for line in lines {
        if line.is_empty() {
            break;
        }
        if let Some((name, value)) = line.split_once(':') {
            let value = value.trim();
            match name.to_ascii_lowercase().as_str() {
                "upgrade" if value.eq_ignore_ascii_case("websocket") => upgrade_websocket = true,
                "sec-websocket-key" => ws_key = Some(value.to_owned()),
                _ => {}
            }
        }
    }
    Ok((
        RequestHead {
            get: method == "GET",
            path,
            upgrade_websocket,
            ws_key,
        },
        buf,
    ))
}

fn mime(path: &str) -> &'static str {
    match path.rsplit_once('.').map(|(_, ext)| ext) {
        Some("html") => "text/html; charset=utf-8",
        Some("js" | "mjs") => "text/javascript",
        Some("css") => "text/css",
        Some("json" | "map") => "application/json",
        Some("svg") => "image/svg+xml",
        Some("png") => "image/png",
        Some("ico") => "image/x-icon",
        Some("woff2") => "font/woff2",
        _ => "application/octet-stream",
    }
}

/// Serve one asset from the embedded build; unknown paths fall back to
/// `index.html` (query strings like `?review=` land on the app shell).
async fn respond_asset(stream: &mut TcpStream, head: &RequestHead) -> std::io::Result<()> {
    let path = head.path.split(['?', '#']).next().unwrap_or("/");
    let rel = path.trim_start_matches('/');
    let (body, mime): (&[u8], &str) = match UI.get_file(rel) {
        Some(f) if head.get && !rel.is_empty() => (f.contents(), mime(rel)),
        _ if head.get => (
            UI.get_file("index.html")
                .map_or(b"missing ui/dist" as &[u8], |f| f.contents()),
            "text/html; charset=utf-8",
        ),
        _ => {
            stream
                .write_all(b"HTTP/1.1 405 Method Not Allowed\r\ncontent-length: 0\r\n\r\n")
                .await?;
            return Ok(());
        }
    };
    let header = format!(
        "HTTP/1.1 200 OK\r\ncontent-type: {mime}\r\ncontent-length: {}\r\nconnection: close\r\n\r\n",
        body.len()
    );
    stream.write_all(header.as_bytes()).await?;
    stream.write_all(body).await
}

/// One command from the browser; the tag is parsed once, here.
#[derive(Debug, serde::Deserialize)]
#[serde(tag = "cmd", rename_all = "snake_case", deny_unknown_fields)]
enum Command {
    Dispatch { action: Action },
    Key { chord: KeyChord },
    Attach,
}

#[derive(Debug, thiserror::Error)]
pub enum ServeError {
    #[error("bind {addr}: {source}")]
    Bind {
        addr: SocketAddr,
        source: std::io::Error,
    },
    #[error("host: {0}")]
    Host(#[from] HostError),
    #[error("host task exited during setup")]
    HostGone,
}

/// A running bridge: the bound address and a way to stop it.
#[derive(Debug)]
pub struct Server {
    addr: SocketAddr,
    shutdown: CancellationToken,
}

impl Server {
    #[must_use]
    pub fn addr(&self) -> SocketAddr {
        self.addr
    }

    pub fn stop(&self) {
        self.shutdown.cancel();
    }
}

impl Drop for Server {
    fn drop(&mut self) {
        self.shutdown.cancel();
    }
}

/// Start the host and accept browsers on `addr` (port 0 picks a free one).
pub async fn serve(addr: SocketAddr, config: HostConfig) -> Result<Server, ServeError> {
    let listener = TcpListener::bind(addr)
        .await
        .map_err(|source| ServeError::Bind { addr, source })?;
    let addr = listener
        .local_addr()
        .map_err(|source| ServeError::Bind { addr, source })?;
    let shutdown = CancellationToken::new();
    let (handle, mut patches) = nits_client_host::spawn(config, shutdown.clone())?;
    // Serialize once, fan out to every socket.
    let (patches_tx, _) = broadcast::channel::<String>(256);
    let fan_out = patches_tx.clone();
    let stop = shutdown.clone();
    tokio::spawn(async move {
        while let Some(batch) = patches.recv().await {
            match serde_json::to_string(&batch) {
                Ok(text) => {
                    // No receivers is fine: nobody is attached yet.
                    let _ = fan_out.send(text);
                }
                Err(e) => tracing::error!(error = %e, "serialize patches"),
            }
        }
        stop.cancel();
    });
    // The core only dials when asked; a bridge always wants a connection.
    if !handle.dispatch(Action::Connect) {
        return Err(ServeError::HostGone);
    }
    let accept_shutdown = shutdown.clone();
    tokio::spawn(async move {
        let mut tasks = tokio::task::JoinSet::<()>::new();
        loop {
            tokio::select! {
                () = accept_shutdown.cancelled() => break,
                accepted = listener.accept() => match accepted {
                    Ok((stream, peer)) => {
                        let handle = handle.clone();
                        let rx = patches_tx.subscribe();
                        let stop = accept_shutdown.clone();
                        tasks.spawn(async move {
                            if let Err(e) = route(stream, handle, rx, stop).await {
                                tracing::debug!(%peer, error = %e, "connection failed");
                            }
                        });
                    }
                    Err(e) => {
                        tracing::warn!(error = %e, "accept failed");
                        break;
                    }
                },
            }
        }
        tasks.abort_all();
    });
    Ok(Server { addr, shutdown })
}

/// One TCP connection: `/ws` + upgrade becomes a bridge client, anything
/// else is a single asset response.
async fn route(
    mut stream: TcpStream,
    handle: Handle,
    patches: broadcast::Receiver<String>,
    shutdown: CancellationToken,
) -> std::io::Result<()> {
    let (head, _raw) = read_head(&mut stream).await?;
    if head.get && head.upgrade_websocket && head.path.starts_with("/ws") {
        let Some(key) = &head.ws_key else {
            stream
                .write_all(b"HTTP/1.1 400 Bad Request\r\ncontent-length: 0\r\n\r\n")
                .await?;
            return Ok(());
        };
        let accept = derive_accept_key(key.as_bytes());
        let reply = format!(
            "HTTP/1.1 101 Switching Protocols\r\nupgrade: websocket\r\nconnection: Upgrade\r\nsec-websocket-accept: {accept}\r\n\r\n"
        );
        stream.write_all(reply.as_bytes()).await?;
        let ws =
            tokio_tungstenite::WebSocketStream::from_raw_socket(stream, Role::Server, None).await;
        client(ws, handle, patches, shutdown).await;
        return Ok(());
    }
    respond_asset(&mut stream, &head).await
}

async fn client<S>(
    ws: tokio_tungstenite::WebSocketStream<S>,
    handle: Handle,
    mut patches: broadcast::Receiver<String>,
    shutdown: CancellationToken,
) where
    S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin,
{
    let (mut tx, mut rx) = ws.split();
    loop {
        tokio::select! {
            () = shutdown.cancelled() => break,
            batch = patches.recv() => match batch {
                Ok(text) => {
                    if tx.send(Message::Text(text.into())).await.is_err() {
                        break;
                    }
                }
                Err(broadcast::error::RecvError::Lagged(n)) => {
                    // Dropped patches would corrupt the client's model copy;
                    // an attach re-emits every section.
                    tracing::warn!(dropped = n, "slow websocket client; re-attaching");
                    if !handle.attach() {
                        break;
                    }
                }
                Err(broadcast::error::RecvError::Closed) => break,
            },
            msg = rx.next() => match msg {
                Some(Ok(Message::Text(text))) => {
                    let alive = match serde_json::from_str::<Command>(&text) {
                        Ok(Command::Dispatch { action }) => handle.dispatch(action),
                        Ok(Command::Key { chord }) => handle.key(chord),
                        Ok(Command::Attach) => handle.attach(),
                        Err(e) => {
                            tracing::warn!(error = %e, "bad command from browser");
                            true
                        }
                    };
                    if !alive {
                        break;
                    }
                }
                Some(Ok(Message::Ping(_) | Message::Pong(_) | Message::Binary(_) | Message::Frame(_))) => {}
                Some(Ok(Message::Close(_)) | Err(_)) | None => break,
            },
        }
    }
}
