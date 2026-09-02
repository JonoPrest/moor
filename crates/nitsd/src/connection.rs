//! One client connection: handshake, request multiplexing, cancellation,
//! subscriptions and streamed responses.
//!
//! Every request runs in its own task so a long render never delays a
//! mutation from the same client. Outgoing frames funnel through one
//! unbounded channel to a writer task; ordering within a request is
//! preserved because each request task sends sequentially.

use std::collections::HashMap;
use std::sync::Arc;

use nits_protocol::{
    ChunkIndex, ClientMsg, Envelope, Event, ProtocolVersion, RenderChunk, Request, RequestId,
    Response, ResponseShape, RpcError, Seq, ServerMsg, Since, StreamItem, SubscribeScope,
};
use tokio::io::{AsyncRead, AsyncWrite};
use tokio::sync::{Mutex, mpsc};
use tokio::task::AbortHandle;

use crate::codec::CodecError;
use crate::daemon::{Daemon, DaemonError};
use crate::dispatch;
use crate::handshake::{AwaitingHello, Negotiated};
use crate::transport::{self, FrameRead, FrameWrite};

#[derive(Debug, thiserror::Error)]
pub enum ConnectionError {
    #[error(transparent)]
    Codec(#[from] CodecError),
    #[error("handshake rejected")]
    Rejected,
}

/// Outgoing side of a connection, cheap to clone into request tasks.
#[derive(Debug, Clone)]
pub struct Outbox {
    tx: mpsc::UnboundedSender<ServerMsg>,
}

impl Outbox {
    /// Queue a message. A closed connection drops it; the request task will
    /// notice when it is aborted.
    pub fn send(&self, msg: ServerMsg) {
        let _ = self.tx.send(msg);
    }
}

/// Per-connection subscription state, shared with the event tail task.
#[derive(Debug, Default)]
struct Subscriptions {
    scopes: Vec<SubscribeScope>,
    /// Highest `Seq` already delivered to this client, so replay and the
    /// live tail never send the same event twice.
    delivered: Option<Seq>,
}

/// Serve one already-accepted byte stream (length-prefixed frames) until
/// the peer disconnects.
pub async fn serve<S>(daemon: Arc<Daemon>, stream: S) -> Result<(), ConnectionError>
where
    S: AsyncRead + AsyncWrite + Send + 'static,
{
    let (rd, wr) = transport::byte_stream(stream);
    serve_framed(daemon, rd, wr).await
}

/// Serve one connection over any framed transport.
pub async fn serve_framed<R, W>(
    daemon: Arc<Daemon>,
    mut rd: R,
    mut wr: W,
) -> Result<(), ConnectionError>
where
    R: FrameRead + 'static,
    W: FrameWrite + 'static,
{
    // Handshake happens inline: nothing else may be in flight yet.
    let Some(first) = transport::recv_msg::<_, ClientMsg>(&mut rd).await? else {
        return Ok(());
    };
    let hello = AwaitingHello {
        daemon: daemon.build.clone(),
        schema: daemon.schema(),
    };
    let negotiated = match hello.negotiate(first) {
        Ok((n, welcome)) => {
            transport::send_msg(&mut wr, &welcome).await?;
            n
        }
        Err(rejected) => {
            transport::send_msg(&mut wr, &rejected.reply).await?;
            wr.close().await.ok();
            return Err(ConnectionError::Rejected);
        }
    };
    tracing::info!(client = %negotiated.client_id, name = %negotiated.client.name, "connected");

    let (tx, rx) = mpsc::unbounded_channel();
    let outbox = Outbox { tx };
    let writer = tokio::spawn(write_loop(wr, rx, negotiated.protocol));

    let _tracked = daemon.track_connection();
    let conn = Arc::new(Connection {
        daemon,
        negotiated,
        outbox: outbox.clone(),
        subs: Mutex::new(Subscriptions::default()),
    });
    let tail = tokio::spawn(event_tail(Arc::clone(&conn)));
    let delta_tail = tokio::spawn(delta_tail(Arc::clone(&conn)));
    let mut in_flight: HashMap<RequestId, AbortHandle> = HashMap::new();

    let result = read_loop(&conn, &mut rd, &mut in_flight).await;

    for (_, h) in in_flight.drain() {
        h.abort();
    }
    tail.abort();
    delta_tail.abort();
    drop(conn);
    drop(outbox);
    // Let queued frames drain before closing.
    let _ = writer.await;
    result
}

struct Connection {
    daemon: Arc<Daemon>,
    negotiated: Negotiated,
    outbox: Outbox,
    subs: Mutex<Subscriptions>,
}

async fn read_loop<R: FrameRead>(
    conn: &Arc<Connection>,
    rd: &mut R,
    in_flight: &mut HashMap<RequestId, AbortHandle>,
) -> Result<(), ConnectionError> {
    while let Some(env) = transport::recv_msg::<_, ClientMsg>(rd).await? {
        in_flight.retain(|_, h| !h.is_finished());
        if let Err(error) = conn.negotiated.check(env.v) {
            // No request id to attach it to for a bad Hello re-send; use 0.
            let id = match &env.msg {
                ClientMsg::Request { id, .. } | ClientMsg::Cancel { id } => *id,
                ClientMsg::Hello { .. } => RequestId::new(0),
            };
            conn.outbox.send(ServerMsg::Error { id, error });
            continue;
        }
        match env.msg {
            ClientMsg::Hello { .. } => {
                conn.outbox.send(ServerMsg::Error {
                    id: RequestId::new(0),
                    error: RpcError::Invalid {
                        reason: "Hello after handshake".into(),
                    },
                });
            }
            ClientMsg::Request { id, request } => {
                let c = Arc::clone(conn);
                let handle = tokio::spawn(async move { c.handle(id, request).await });
                in_flight.insert(id, handle.abort_handle());
            }
            ClientMsg::Cancel { id } => {
                if let Some(h) = in_flight.remove(&id) {
                    h.abort();
                    conn.outbox.send(ServerMsg::Error {
                        id,
                        error: RpcError::Cancelled,
                    });
                    conn.outbox.send(ServerMsg::StreamEnd { id });
                }
            }
        }
    }
    Ok(())
}

async fn write_loop<W: FrameWrite>(
    mut wr: W,
    mut rx: mpsc::UnboundedReceiver<ServerMsg>,
    v: ProtocolVersion,
) {
    while let Some(msg) = rx.recv().await {
        let env = Envelope { v, msg };
        if let Err(e) = transport::send_msg(&mut wr, &env).await {
            tracing::debug!(error = %e, "write failed; closing");
            return;
        }
    }
    let _ = wr.close().await;
}

/// Forward broadcast events matching this connection's scopes.
async fn event_tail(conn: Arc<Connection>) {
    let mut rx = conn.daemon.subscribe();
    loop {
        match rx.recv().await {
            Ok(event) => conn.deliver_live(&event).await,
            Err(tokio::sync::broadcast::error::RecvError::Lagged(n)) => {
                // The client fell behind the backlog. Tell it where we are
                // so it can resubscribe from its last seen seq.
                tracing::warn!(skipped = n, "subscriber lagged");
                let oldest = conn
                    .daemon
                    .read(nits_review_core::Core::last_seq)
                    .await
                    .ok()
                    .flatten()
                    .unwrap_or(Seq::FIRST);
                conn.outbox.send(ServerMsg::Error {
                    id: RequestId::new(0),
                    error: RpcError::SeqTooOld { oldest },
                });
            }
            Err(tokio::sync::broadcast::error::RecvError::Closed) => return,
        }
    }
}

/// Forward working-tree deltas matching this connection's scopes.
async fn delta_tail(conn: Arc<Connection>) {
    let mut rx = conn.daemon.subscribe_deltas();
    loop {
        match rx.recv().await {
            Ok(delta) => {
                let subs = conn.subs.lock().await;
                if subs
                    .scopes
                    .iter()
                    .any(|s| conn.daemon.delta_matches(s, &delta))
                {
                    conn.outbox.send(ServerMsg::TreeDelta {
                        delta: (*delta).clone(),
                    });
                }
            }
            Err(tokio::sync::broadcast::error::RecvError::Lagged(n)) => {
                tracing::warn!(skipped = n, "delta subscriber lagged");
            }
            Err(tokio::sync::broadcast::error::RecvError::Closed) => return,
        }
    }
}

impl Connection {
    async fn deliver_live(&self, event: &Arc<Event>) {
        let mut subs = self.subs.lock().await;
        if subs.delivered.is_some_and(|d| event.seq <= d) {
            return;
        }
        if subs.scopes.iter().any(|s| self.daemon.matches(s, event)) {
            subs.delivered = Some(event.seq);
            self.outbox.send(ServerMsg::Event {
                event: (**event).clone(),
            });
        }
    }

    async fn handle(self: Arc<Self>, id: RequestId, request: Request) {
        let shape = request.shape();
        let result = match request {
            Request::Subscribe { scope, since } => self.subscribe(scope, since).await,
            Request::Unsubscribe { scope } => {
                self.subs.lock().await.scopes.retain(|s| *s != scope);
                Ok(Response::Unsubscribed)
            }
            Request::Shutdown => {
                tracing::info!(client = %self.negotiated.client_id, "shutdown requested");
                let token = self.daemon.shutdown().clone();
                // Let the reply reach the client before the accept loop
                // drops every connection.
                tokio::spawn(async move {
                    tokio::time::sleep(std::time::Duration::from_millis(50)).await;
                    token.cancel();
                });
                Ok(Response::ShuttingDown)
            }
            Request::OpenReview { review_id, opts } => {
                let r =
                    dispatch::open_review(&self.daemon, id, &self.outbox, review_id, opts).await;
                self.finish_stream(id, r);
                return;
            }
            Request::FileRender {
                review_id,
                repo_id,
                path,
                opts,
                first_chunk,
                scope,
            } => {
                let r = self
                    .daemon
                    .read(move |core| core.file_render(review_id, repo_id, &path, opts, &scope))
                    .await
                    .map(|(header, rendered)| {
                        stream_render(&self.outbox, id, header, &rendered, first_chunk);
                    });
                self.finish_stream(id, r);
                return;
            }
            Request::ChangeRender {
                repo_id,
                path,
                change,
                opts,
                first_chunk,
            } => {
                let r = self
                    .daemon
                    .read(move |core| core.render_change(repo_id, &path, change, opts))
                    .await
                    .map(|(header, rendered)| {
                        stream_render(&self.outbox, id, header, &rendered, first_chunk);
                    });
                self.finish_stream(id, r);
                return;
            }
            Request::BlobRender {
                repo_id,
                path,
                blob_oid,
                first_chunk,
            } => {
                let r = self
                    .daemon
                    .read(move |core| core.blob_render(repo_id, &path, blob_oid))
                    .await
                    .map(|(header, rendered)| {
                        stream_render(&self.outbox, id, header, &rendered, first_chunk);
                    });
                self.finish_stream(id, r);
                return;
            }
            other => dispatch::single(&self.daemon, &self.negotiated, other).await,
        };
        debug_assert_eq!(shape, ResponseShape::Single);
        match result {
            Ok(response) => self.outbox.send(ServerMsg::Response { id, response }),
            Err(error) => self.outbox.send(ServerMsg::Error {
                id,
                error: error.into(),
            }),
        }
    }

    fn finish_stream(&self, id: RequestId, r: Result<(), DaemonError>) {
        if let Err(error) = r {
            self.outbox.send(ServerMsg::Error {
                id,
                error: error.into(),
            });
        }
        self.outbox.send(ServerMsg::StreamEnd { id });
    }

    /// Register a scope, replay the gap for `Since::After`, and report the
    /// position live events continue from.
    async fn subscribe(
        &self,
        scope: SubscribeScope,
        since: Since,
    ) -> Result<Response, DaemonError> {
        // Hold the lock across the replay so the live tail cannot interleave
        // an event the replay is about to send.
        let mut subs = self.subs.lock().await;
        let after = match since {
            Since::Now => None,
            Since::After { seq } => Some(seq),
        };
        let events = self
            .daemon
            .read(move |core| core.events_after(after))
            .await?;
        let mut last = subs.delivered;
        if matches!(since, Since::After { .. }) {
            for e in events
                .iter()
                .filter(|e| self.daemon.matches(&scope, e))
                .filter(|e| subs.delivered.is_none_or(|d| e.seq > d))
            {
                last = Some(e.seq);
                self.outbox.send(ServerMsg::Event { event: e.clone() });
            }
        }
        let head = events.last().map(|e| e.seq).or(after);
        let head = match head {
            Some(h) => h,
            None => self
                .daemon
                .read(nits_review_core::Core::last_seq)
                .await?
                .unwrap_or(Seq::new(0)),
        };
        // Everything up to `head` is either replayed above or, for
        // `Since::Now`, deliberately skipped.
        subs.delivered = Some(last.map_or(head, |l| l.max(head)));
        if !subs.scopes.contains(&scope) {
            subs.scopes.push(scope);
        }
        Ok(Response::Subscribed { seq: head })
    }
}

/// Header, then chunks starting at `first`, wrapping round to the start.
pub fn stream_render(
    out: &Outbox,
    id: RequestId,
    header: nits_protocol::FileRenderHeader,
    rendered: &nits_review_core::render::Rendered,
    first: ChunkIndex,
) {
    let repo_id = header.repo_id;
    let path = header.path.clone();
    out.send(ServerMsg::StreamItem {
        id,
        item: StreamItem::Header { header },
    });
    let n = rendered.chunk_count();
    let start = first.get().min(n.saturating_sub(1));
    for i in (start..n).chain(0..start) {
        if let Some(chunk) = rendered.chunk(ChunkIndex::new(i)) {
            send_chunk(out, id, repo_id, path.clone(), chunk);
        }
    }
}

pub fn send_chunk(
    out: &Outbox,
    id: RequestId,
    repo_id: nits_protocol::RepoId,
    path: nits_protocol::RepoPath,
    chunk: RenderChunk,
) {
    out.send(ServerMsg::StreamItem {
        id,
        item: StreamItem::Chunk {
            repo_id,
            path,
            chunk,
        },
    });
}
