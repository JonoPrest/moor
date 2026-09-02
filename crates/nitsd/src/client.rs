//! Async client over any `AsyncRead + AsyncWrite` stream. Used by the
//! daemon's own tests, the `nits` CLI and the MCP shim; UIs use
//! `nits-client-core` instead.

use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use nits_protocol::{
    Author, BuildInfo, ClientId, ClientMsg, Envelope, Event, ProtocolVersion, Request, RequestId,
    Response, RpcError, SchemaVersion, ServerMsg, StreamItem, UpgradeNotice,
};
use tokio::io::{AsyncRead, AsyncWrite};
use tokio::sync::{Mutex, mpsc, oneshot};

use crate::codec::CodecError;
use crate::transport::{self, FrameRead, FrameWrite};

#[derive(Debug, thiserror::Error)]
pub enum ClientError {
    #[error(transparent)]
    Codec(#[from] CodecError),
    #[error("handshake rejected: {0:?}")]
    Rejected(RpcError),
    #[error("expected Welcome or Rejected, got {0:?}")]
    BadHandshake(Box<ServerMsg>),
    #[error("connection closed")]
    Closed,
    #[error("daemon error: {0:?}")]
    Rpc(RpcError),
}

/// What the daemon said in `Welcome`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Welcome {
    pub protocol: ProtocolVersion,
    pub daemon: BuildInfo,
    pub schema: SchemaVersion,
    pub upgrade: Option<UpgradeNotice>,
}

/// Unsolicited messages: events for subscribed scopes, tree deltas, and
/// errors the daemon could not attach to a request (`id` 0).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Unsolicited {
    Event(Event),
    TreeDelta(nits_protocol::TreeDelta),
    Error(RpcError),
}

enum Pending {
    Single(oneshot::Sender<Result<Response, RpcError>>),
    Stream(mpsc::UnboundedSender<Result<StreamItem, RpcError>>),
}

#[derive(Debug)]
pub struct Client {
    out: mpsc::UnboundedSender<ClientMsg>,
    next_id: AtomicU64,
    pending: Arc<Mutex<HashMap<RequestId, Pending>>>,
    unsolicited: Mutex<mpsc::UnboundedReceiver<Unsolicited>>,
    /// The demux task. Aborted on drop so the read half is released; with
    /// a split stream (stdio) the peer only sees EOF once both halves go.
    reader: tokio::task::AbortHandle,
    pub welcome: Welcome,
}

impl Drop for Client {
    fn drop(&mut self) {
        self.reader.abort();
    }
}

impl std::fmt::Debug for Pending {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Pending::Single(_) => f.write_str("Single"),
            Pending::Stream(_) => f.write_str("Stream"),
        }
    }
}

/// Identity presented in `Hello`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Identity {
    pub client_id: ClientId,
    pub client: BuildInfo,
    pub author: Author,
}

impl Client {
    /// Connect to a daemon's unix socket and complete the handshake.
    pub async fn connect_unix(path: &Path, identity: Identity) -> Result<Self, ClientError> {
        let stream = tokio::net::UnixStream::connect(path)
            .await
            .map_err(CodecError::Io)?;
        Self::handshake(stream, identity, ProtocolVersion::CURRENT).await
    }

    /// Connect to a daemon's WebSocket endpoint (`ws://host:port`) and
    /// complete the handshake.
    pub async fn connect_ws(url: &str, identity: Identity) -> Result<Self, ClientError> {
        let (ws, _) = tokio_tungstenite::connect_async(url)
            .await
            .map_err(|e| CodecError::Io(std::io::Error::other(e)))?;
        let (rd, wr) = transport::web_socket(ws);
        Self::handshake_framed(rd, wr, identity, ProtocolVersion::CURRENT).await
    }

    /// Handshake over a byte stream (length-prefixed frames), requesting
    /// `protocol`.
    pub async fn handshake<S>(
        stream: S,
        identity: Identity,
        protocol: ProtocolVersion,
    ) -> Result<Self, ClientError>
    where
        S: AsyncRead + AsyncWrite + Send + 'static,
    {
        let (rd, wr) = transport::byte_stream(stream);
        Self::handshake_framed(rd, wr, identity, protocol).await
    }

    /// Handshake over any framed transport, requesting `protocol`.
    pub async fn handshake_framed<R, W>(
        mut rd: R,
        mut wr: W,
        identity: Identity,
        protocol: ProtocolVersion,
    ) -> Result<Self, ClientError>
    where
        R: FrameRead + 'static,
        W: FrameWrite + 'static,
    {
        transport::send_msg(
            &mut wr,
            &Envelope {
                v: protocol,
                msg: ClientMsg::Hello {
                    client_id: identity.client_id,
                    protocol,
                    client: identity.client,
                    author: identity.author,
                },
            },
        )
        .await?;
        let reply = transport::recv_msg::<_, ServerMsg>(&mut rd)
            .await?
            .ok_or(ClientError::Closed)?;
        let welcome = match reply.msg {
            ServerMsg::Welcome {
                protocol,
                daemon,
                schema,
                upgrade,
            } => Welcome {
                protocol,
                daemon,
                schema,
                upgrade,
            },
            ServerMsg::Rejected { error } => return Err(ClientError::Rejected(error)),
            other => return Err(ClientError::BadHandshake(Box::new(other))),
        };

        let (out, mut out_rx) = mpsc::unbounded_channel::<ClientMsg>();
        let v = welcome.protocol;
        tokio::spawn(async move {
            while let Some(msg) = out_rx.recv().await {
                if transport::send_msg(&mut wr, &Envelope { v, msg })
                    .await
                    .is_err()
                {
                    return;
                }
            }
        });

        let pending: Arc<Mutex<HashMap<RequestId, Pending>>> = Arc::default();
        let (un_tx, un_rx) = mpsc::unbounded_channel();
        let demux = Arc::clone(&pending);
        let reader = tokio::spawn(async move {
            loop {
                let Ok(Some(env)) = transport::recv_msg::<_, ServerMsg>(&mut rd).await else {
                    break;
                };
                dispatch(&demux, &un_tx, env.msg).await;
            }
            demux.lock().await.clear();
        })
        .abort_handle();

        Ok(Self {
            out,
            next_id: AtomicU64::new(1),
            pending,
            unsolicited: Mutex::new(un_rx),
            reader,
            welcome,
        })
    }

    fn next_id(&self) -> RequestId {
        RequestId::new(self.next_id.fetch_add(1, Ordering::Relaxed))
    }

    /// Send a single-response request.
    pub async fn request(&self, request: Request) -> Result<Response, ClientError> {
        let id = self.next_id();
        let (tx, rx) = oneshot::channel();
        self.pending.lock().await.insert(id, Pending::Single(tx));
        self.out
            .send(ClientMsg::Request { id, request })
            .map_err(|_| ClientError::Closed)?;
        rx.await
            .map_err(|_| ClientError::Closed)?
            .map_err(ClientError::Rpc)
    }

    /// Send a streaming request. The receiver closes after `StreamEnd`.
    pub async fn stream(
        &self,
        request: Request,
    ) -> Result<
        (
            RequestId,
            mpsc::UnboundedReceiver<Result<StreamItem, RpcError>>,
        ),
        ClientError,
    > {
        let id = self.next_id();
        let (tx, rx) = mpsc::unbounded_channel();
        self.pending.lock().await.insert(id, Pending::Stream(tx));
        self.out
            .send(ClientMsg::Request { id, request })
            .map_err(|_| ClientError::Closed)?;
        Ok((id, rx))
    }

    /// Ask the daemon to stop a stream early.
    pub fn cancel(&self, id: RequestId) {
        let _ = self.out.send(ClientMsg::Cancel { id });
    }

    /// Next unsolicited message; `None` once the connection is gone.
    pub async fn next_unsolicited(&self) -> Option<Unsolicited> {
        self.unsolicited.lock().await.recv().await
    }

    /// Next subscribed event, skipping other unsolicited messages.
    pub async fn next_event(&self) -> Option<Event> {
        loop {
            match self.next_unsolicited().await? {
                Unsolicited::Event(e) => return Some(e),
                Unsolicited::TreeDelta(_) | Unsolicited::Error(_) => {}
            }
        }
    }

    /// Send a raw client message; for protocol tests.
    pub fn send_raw(&self, msg: ClientMsg) {
        let _ = self.out.send(msg);
    }
}

async fn dispatch(
    pending: &Mutex<HashMap<RequestId, Pending>>,
    unsolicited: &mpsc::UnboundedSender<Unsolicited>,
    msg: ServerMsg,
) {
    match msg {
        ServerMsg::Response { id, response } => {
            if let Some(Pending::Single(tx)) = pending.lock().await.remove(&id) {
                let _ = tx.send(Ok(response));
            }
        }
        ServerMsg::StreamItem { id, item } => {
            if let Some(Pending::Stream(tx)) = pending.lock().await.get(&id) {
                let _ = tx.send(Ok(item));
            }
        }
        ServerMsg::StreamEnd { id } => {
            pending.lock().await.remove(&id);
        }
        ServerMsg::Error { id, error } => {
            if id == RequestId::new(0) {
                let _ = unsolicited.send(Unsolicited::Error(error));
                return;
            }
            let mut p = pending.lock().await;
            match p.get(&id) {
                Some(Pending::Single(_)) => {
                    if let Some(Pending::Single(tx)) = p.remove(&id) {
                        let _ = tx.send(Err(error));
                    }
                }
                Some(Pending::Stream(tx)) => {
                    // The stream stays registered until StreamEnd.
                    let _ = tx.send(Err(error));
                }
                None => {}
            }
        }
        ServerMsg::Event { event } => {
            let _ = unsolicited.send(Unsolicited::Event(event));
        }
        ServerMsg::TreeDelta { delta } => {
            let _ = unsolicited.send(Unsolicited::TreeDelta(delta));
        }
        ServerMsg::Welcome { .. } | ServerMsg::Rejected { .. } => {}
    }
}
