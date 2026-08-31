//! WebSocket bridge for the browser UI (dev/test; ARCHITECTURE §6.2).
//!
//! Speaks the same contract as the Tauri wrapper: commands in, `view`
//! patch batches out. One `moor-client-host` per server; every connected
//! browser sees the same view. Messages are JSON text frames:
//!
//! - in: `{"cmd":"dispatch","action":…}` | `{"cmd":"key","chord":…}` |
//!   `{"cmd":"attach"}`
//! - out: an array of `ViewPatch`
//!
//! `Attach` re-emits every section, so each frame is broadcast: a client
//! that attaches late catches up from its own attach.

use std::net::SocketAddr;

use futures_util::{SinkExt as _, StreamExt as _};
use moor_client_core::{Action, KeyChord};
use moor_client_host::{Handle, HostConfig, HostError};
use tokio::net::TcpListener;
use tokio::sync::broadcast;
use tokio_tungstenite::tungstenite::Message;
use tokio_util::sync::CancellationToken;

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
    let (handle, mut patches) = moor_client_host::spawn(config, shutdown.clone())?;
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
                            match tokio_tungstenite::accept_async(stream).await {
                                Ok(ws) => client(ws, handle, rx, stop).await,
                                Err(e) => {
                                    tracing::debug!(%peer, error = %e, "websocket upgrade failed");
                                }
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
