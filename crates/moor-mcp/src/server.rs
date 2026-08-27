//! The MCP server: `initialize`, `tools/list`, `tools/call`, proxied to the
//! daemon. One request at a time, in order — MCP clients pipeline rarely and
//! the daemon connection is shared, so serialising keeps event long-polls
//! from interleaving with other calls.

use std::path::PathBuf;
use std::time::Duration;

use moor_protocol::{
    AgentVia, Anchor, Author, BlobOid, BuildInfo, ChunkIndex, ClientId, ClientSeq, CommentId,
    CommentKind, ContextHash, Event, Human, LineNo, LineRange, Mutation, NonEmpty, RefSpec,
    RenderOpts, RepoId, RepoPath, Request, Response, ReviewId, ReviewSnapshot, ReviewTarget,
    RpcError, Seq, Side, Since, StreamItem, SubscribeScope, ThreadId, TreeEntryKind,
};
use moord::client::{Client, ClientError, Identity, Unsolicited};
use serde_json::{Value, json};

use crate::jsonrpc::{self, Incoming, Outgoing};
use crate::{text, tools};

/// Tools that append to the log; the rest only read.
const MUTATING: &[&str] = &[
    "create_review",
    "update_review",
    "add_comment",
    "suggest",
    "reply",
    "resolve",
    "request_review",
];

/// MCP protocol revision this server implements.
pub const MCP_VERSION: &str = "2025-06-18";

/// How to reach the daemon.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Endpoint {
    Unix(PathBuf),
    Ws(String),
}

/// Identity of the agent on the other end of stdio, from the environment
/// and the MCP `initialize` call.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentIdentity {
    /// Model name, e.g. from `MOOR_AGENT_MODEL`.
    pub model: String,
    /// Stable id for this agent session, e.g. from `MOOR_SESSION_ID`.
    pub session_id: String,
    /// The human whose shell/editor launched the agent, if known.
    pub invoked_by: Option<Human>,
}

impl AgentIdentity {
    /// From `MOOR_AGENT_MODEL`, `MOOR_SESSION_ID` and `USER`.
    #[must_use]
    pub fn from_env() -> Self {
        let machine = gethostname::gethostname().to_string_lossy().into_owned();
        let (ts, r) = moord::ids::fresh_parts();
        Self {
            model: std::env::var("MOOR_AGENT_MODEL").unwrap_or_else(|_| "unknown".into()),
            session_id: std::env::var("MOOR_SESSION_ID")
                .unwrap_or_else(|_| ClientId::from_parts(ts, r).to_string()),
            invoked_by: std::env::var("USER")
                .ok()
                .map(|name| Human { name, machine }),
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ToolError {
    #[error("{0}")]
    Invalid(String),
    #[error("not connected: call initialize first")]
    NotInitialized,
    #[error("daemon: {0}")]
    Daemon(Box<ClientError>),
    #[error("daemon: {0:?}")]
    Rpc(RpcError),
    #[error("unexpected response from daemon")]
    Shape,
}

impl From<ClientError> for ToolError {
    fn from(e: ClientError) -> Self {
        ToolError::Daemon(Box::new(e))
    }
}

impl From<RpcError> for ToolError {
    fn from(e: RpcError) -> Self {
        ToolError::Rpc(e)
    }
}

impl From<serde_json::Error> for ToolError {
    fn from(e: serde_json::Error) -> Self {
        ToolError::Invalid(format!("invalid params: {e}"))
    }
}

impl From<moor_protocol::InvariantError> for ToolError {
    fn from(e: moor_protocol::InvariantError) -> Self {
        ToolError::Invalid(e.to_string())
    }
}

#[derive(Debug)]
pub struct Server {
    endpoint: Endpoint,
    agent: AgentIdentity,
    build: BuildInfo,
    client: Option<Client>,
    seq: u64,
}

impl Server {
    #[must_use]
    pub fn new(endpoint: Endpoint, agent: AgentIdentity, build: BuildInfo) -> Self {
        Self {
            endpoint,
            agent,
            build,
            client: None,
            seq: 0,
        }
    }

    /// The daemon connection, once `initialize` has run.
    #[must_use]
    pub fn client(&self) -> Option<&Client> {
        self.client.as_ref()
    }

    /// Handle one line of stdin. Notifications produce no reply.
    pub async fn handle_line(&mut self, line: &str) -> Option<Outgoing> {
        let msg: Incoming = match serde_json::from_str(line) {
            Ok(m) => m,
            Err(e) => {
                return Some(Outgoing::error(
                    Value::Null,
                    jsonrpc::PARSE_ERROR,
                    format!("parse error: {e}"),
                ));
            }
        };
        self.handle(msg).await
    }

    /// Handle one decoded message.
    pub async fn handle(&mut self, msg: Incoming) -> Option<Outgoing> {
        if msg.jsonrpc != "2.0" {
            return Some(Outgoing::error(
                msg.id.unwrap_or(Value::Null),
                jsonrpc::INVALID_REQUEST,
                "jsonrpc must be \"2.0\"",
            ));
        }
        let id = msg.id?;
        Some(match msg.method.as_str() {
            "initialize" => match self.initialize(&msg.params).await {
                Ok(v) => Outgoing::result(id, v),
                Err(e) => Outgoing::error(id, jsonrpc::INTERNAL_ERROR, e.to_string()),
            },
            "ping" => Outgoing::result(id, json!({})),
            "tools/list" => Outgoing::result(
                id,
                json!({
                    "tools": tools::all().into_iter().map(|t| json!({
                        "name": t.name,
                        "description": t.description,
                        "inputSchema": t.input_schema,
                    })).collect::<Vec<_>>()
                }),
            ),
            "tools/call" => {
                let name = msg.params.get("name").and_then(Value::as_str);
                let args = msg
                    .params
                    .get("arguments")
                    .cloned()
                    .unwrap_or_else(|| json!({}));
                match name {
                    None => Outgoing::error(id, jsonrpc::INVALID_PARAMS, "missing tool name"),
                    Some(name) => match self.call(name, args).await {
                        Ok(v) => Outgoing::result(id, tool_ok(&v)),
                        Err(ToolError::Invalid(m))
                            if !tools::all().iter().any(|t| t.name == name) =>
                        {
                            Outgoing::error(id, jsonrpc::INVALID_PARAMS, m)
                        }
                        Err(e) => Outgoing::result(id, tool_err(&e)),
                    },
                }
            }
            other => Outgoing::error(
                id,
                jsonrpc::METHOD_NOT_FOUND,
                format!("method not found: {other}"),
            ),
        })
    }

    async fn initialize(&mut self, params: &Value) -> Result<Value, ToolError> {
        let info = params.get("clientInfo");
        let name = info
            .and_then(|i| i.get("name"))
            .and_then(Value::as_str)
            .unwrap_or("mcp-client")
            .to_string();
        let author = Author::Agent {
            name,
            model: self.agent.model.clone(),
            session_id: self.agent.session_id.clone(),
            invoked_by: self.agent.invoked_by.clone(),
            via: AgentVia::Mcp,
        };
        let (ts, r) = moord::ids::fresh_parts();
        let identity = Identity {
            client_id: ClientId::from_parts(ts, r),
            client: self.build.clone(),
            author,
        };
        let client = match &self.endpoint {
            Endpoint::Unix(p) => Client::connect_unix(p, identity).await?,
            Endpoint::Ws(url) => Client::connect_ws(url, identity).await?,
        };
        let daemon = client.welcome.daemon.clone();
        self.client = Some(client);
        Ok(json!({
            "protocolVersion": MCP_VERSION,
            "capabilities": { "tools": {} },
            "serverInfo": { "name": self.build.name, "version": self.build.version },
            "instructions": format!(
                "Moor code review. Connected to {} {}. Start with list_workspaces, then get_review; \
                 anchor comments with add_comment/suggest; wait for work with subscribe_events.",
                daemon.name, daemon.version
            ),
        }))
    }

    fn conn(&self) -> Result<&Client, ToolError> {
        self.client.as_ref().ok_or(ToolError::NotInitialized)
    }

    /// Run a tool by name. Public so tests can bypass JSON-RPC.
    pub async fn call(&mut self, name: &str, args: Value) -> Result<Value, ToolError> {
        if MUTATING.contains(&name) {
            self.call_mutating(name, args).await
        } else {
            self.call_query(name, args).await
        }
    }

    async fn call_query(&self, name: &str, args: Value) -> Result<Value, ToolError> {
        match name {
            "list_workspaces" => match self.conn()?.request(Request::ListWorkspaces).await? {
                Response::Workspaces { workspaces } => Ok(json!({ "workspaces": workspaces })),
                _ => Err(ToolError::Shape),
            },
            "list_reviews" => {
                let p: tools::ListReviews = serde_json::from_value(args)?;
                match self
                    .conn()?
                    .request(Request::ListReviews {
                        workspace_id: p.workspace_id,
                    })
                    .await?
                {
                    Response::Reviews { reviews } => Ok(json!({ "reviews": reviews })),
                    _ => Err(ToolError::Shape),
                }
            }
            "get_review" => {
                let p: tools::ByReview = serde_json::from_value(args)?;
                let snap = self.snapshot(p.review_id).await?;
                let files = self.files(p.review_id).await?;
                Ok(json!({
                    "review": snap.review,
                    "resolved": snap.resolved,
                    "files": files,
                    "threads": snap.threads,
                    "comments": snap.comments,
                    "seq": snap.seq,
                }))
            }
            "get_diff" => {
                let p: tools::GetDiff = serde_json::from_value(args)?;
                let file = self.file(p.review_id, p.repo_id, &p.path).await?;
                let opts = RenderOpts {
                    ignore_whitespace: p.ignore_whitespace,
                    context_lines: p
                        .context_lines
                        .unwrap_or(RenderOpts::default().context_lines),
                };
                let (header, chunks) = self
                    .collect_render(Request::FileRender {
                        review_id: p.review_id,
                        repo_id: file.repo_id,
                        path: file.path.clone(),
                        opts,
                        first_chunk: ChunkIndex::FIRST,
                    })
                    .await?;
                Ok(json!({
                    "repo_id": file.repo_id,
                    "path": file.path,
                    "change": file.kind,
                    "lang": header.lang,
                    "content": header.content,
                    "text": text::render(&header, &chunks),
                }))
            }
            "get_file" => {
                let p: tools::GetFile = serde_json::from_value(args)?;
                let path = RepoPath::new(p.path)?;
                let (repo_id, blob_oid) = self.blob(p.review_id, p.repo_id, &path, p.side).await?;
                let (header, chunks) = self
                    .collect_render(Request::BlobRender {
                        repo_id,
                        path: path.clone(),
                        blob_oid,
                        first_chunk: ChunkIndex::FIRST,
                    })
                    .await?;
                Ok(json!({
                    "repo_id": repo_id,
                    "path": path,
                    "side": p.side,
                    "blob_oid": blob_oid,
                    "lang": header.lang,
                    "content": header.content,
                    "text": text::render_blob(&header, &chunks),
                }))
            }
            "list_comments" => {
                let p: tools::ByReview = serde_json::from_value(args)?;
                let snap = self.snapshot(p.review_id).await?;
                Ok(json!({ "threads": snap.threads, "comments": snap.comments, "seq": snap.seq }))
            }
            "subscribe_events" => {
                let p: tools::SubscribeEvents = serde_json::from_value(args)?;
                self.subscribe_events(p).await
            }
            other => Err(ToolError::Invalid(format!("unknown tool: {other}"))),
        }
    }

    async fn call_mutating(&mut self, name: &str, args: Value) -> Result<Value, ToolError> {
        match name {
            "create_review" => {
                let p: tools::CreateReview = serde_json::from_value(args)?;
                let targets = NonEmpty::new(
                    p.targets
                        .into_iter()
                        .map(|t| ReviewTarget {
                            repo_id: t.repo_id,
                            base: t.base,
                            head: t.head,
                        })
                        .collect(),
                )?;
                let (ts, r) = moord::ids::fresh_parts();
                let review_id = ReviewId::from_parts(ts, r);
                let event = self
                    .mutate(Mutation::CreateReview {
                        review_id,
                        workspace_id: p.workspace_id,
                        title: p.title,
                        targets,
                    })
                    .await?;
                let snap = self.snapshot(review_id).await?;
                Ok(json!({ "review": snap.review, "resolved": snap.resolved, "event": event }))
            }
            "update_review" => {
                let p: tools::UpdateReview = serde_json::from_value(args)?;
                let event = self
                    .mutate(Mutation::UpdateReview {
                        review_id: p.review_id,
                        title: p.title,
                        status: p.status,
                    })
                    .await?;
                Ok(json!({ "event": event }))
            }
            "add_comment" => {
                let p: tools::AddComment = serde_json::from_value(args)?;
                self.add_comment(p).await
            }
            "suggest" => {
                let p: tools::Suggest = serde_json::from_value(args)?;
                self.suggest(p).await
            }
            "reply" => {
                let p: tools::Reply = serde_json::from_value(args)?;
                let (ts, r) = moord::ids::fresh_parts();
                let comment_id = CommentId::from_parts(ts, r);
                let event = self
                    .mutate(Mutation::Reply {
                        review_id: p.review_id,
                        thread_id: p.thread_id,
                        comment_id,
                        kind: CommentKind::Note,
                        body: p.body,
                    })
                    .await?;
                Ok(json!({ "comment_id": comment_id, "event": event }))
            }
            "resolve" => {
                let p: tools::Resolve = serde_json::from_value(args)?;
                let m = if p.resolved {
                    Mutation::ResolveThread {
                        review_id: p.review_id,
                        thread_id: p.thread_id,
                    }
                } else {
                    Mutation::UnresolveThread {
                        review_id: p.review_id,
                        thread_id: p.thread_id,
                    }
                };
                let event = self.mutate(m).await?;
                Ok(json!({ "event": event }))
            }
            "request_review" => {
                let p: tools::RequestReview = serde_json::from_value(args)?;
                let event = self
                    .mutate(Mutation::RequestReview {
                        review_id: p.review_id,
                        agent: p.agent,
                        note: p.note,
                    })
                    .await?;
                Ok(json!({ "event": event }))
            }
            other => Err(ToolError::Invalid(format!("unknown tool: {other}"))),
        }
    }

    async fn add_comment(&mut self, p: tools::AddComment) -> Result<Value, ToolError> {
        let anchor = match (p.path, p.start_line) {
            (None, Some(_)) => {
                return Err(ToolError::Invalid("start_line needs a path".into()));
            }
            (None, None) => Anchor::Review,
            (Some(path), start) => {
                let path = RepoPath::new(path)?;
                let (repo_id, blob_oid) = self.blob(p.review_id, p.repo_id, &path, p.side).await?;
                match start {
                    None => Anchor::File {
                        repo_id,
                        path,
                        blob_oid,
                    },
                    Some(start) => Anchor::Lines {
                        repo_id,
                        path,
                        side: p.side,
                        blob_oid,
                        lines: line_range(start, p.end_line)?,
                        context_hash: ContextHash::new(0),
                    },
                }
            }
        };
        self.new_thread(p.review_id, CommentKind::Note, anchor, p.body)
            .await
    }

    async fn suggest(&mut self, p: tools::Suggest) -> Result<Value, ToolError> {
        let path = RepoPath::new(p.path)?;
        let (repo_id, blob_oid) = self.blob(p.review_id, p.repo_id, &path, p.side).await?;
        let anchor = Anchor::Lines {
            repo_id,
            path,
            side: p.side,
            blob_oid,
            lines: line_range(p.start_line, p.end_line)?,
            context_hash: ContextHash::new(0),
        };
        self.new_thread(
            p.review_id,
            CommentKind::Suggestion { patch: p.patch },
            anchor,
            p.body,
        )
        .await
    }

    async fn mutate(&mut self, mutation: Mutation) -> Result<Event, ToolError> {
        self.seq += 1;
        let client_seq = ClientSeq::new(self.seq);
        match self
            .conn()?
            .request(Request::Mutate {
                client_seq,
                mutation,
            })
            .await?
        {
            Response::Committed { event } => Ok(event),
            _ => Err(ToolError::Shape),
        }
    }

    async fn new_thread(
        &mut self,
        review_id: ReviewId,
        kind: CommentKind,
        anchor: Anchor,
        body: String,
    ) -> Result<Value, ToolError> {
        let (ts, r) = moord::ids::fresh_parts();
        let comment_id = CommentId::from_parts(ts, r);
        let event = self
            .mutate(Mutation::AddComment {
                review_id,
                comment_id,
                kind,
                anchor,
                body,
            })
            .await?;
        Ok(json!({
            "comment_id": comment_id,
            "thread_id": ThreadId::from_parts(comment_id.timestamp_ms(), comment_id.random()),
            "event": event,
        }))
    }

    async fn snapshot(&self, review_id: ReviewId) -> Result<ReviewSnapshot, ToolError> {
        match self
            .conn()?
            .request(Request::ReviewSnapshot { review_id })
            .await?
        {
            Response::ReviewSnapshot { snapshot } => Ok(snapshot),
            _ => Err(ToolError::Shape),
        }
    }

    async fn files(
        &self,
        review_id: ReviewId,
    ) -> Result<Vec<moor_protocol::FileChange>, ToolError> {
        match self
            .conn()?
            .request(Request::ListFiles { review_id })
            .await?
        {
            Response::Files { files } => Ok(files),
            _ => Err(ToolError::Shape),
        }
    }

    /// The changed file at `path`, disambiguated by `repo_id` when needed.
    async fn file(
        &self,
        review_id: ReviewId,
        repo_id: Option<RepoId>,
        path: &str,
    ) -> Result<moor_protocol::FileChange, ToolError> {
        let files = self.files(review_id).await?;
        let mut matches = files
            .into_iter()
            .filter(|f| f.path.as_str() == path && repo_id.is_none_or(|r| r == f.repo_id));
        let first = matches
            .next()
            .ok_or_else(|| ToolError::Invalid(format!("{path} is not changed in this review")))?;
        if matches.next().is_some() {
            return Err(ToolError::Invalid(format!(
                "{path} is changed in more than one repo; pass repo_id"
            )));
        }
        Ok(first)
    }

    /// The blob at `path` on `side`, looked up through the review's resolved
    /// target trees so unchanged files resolve too.
    async fn blob(
        &self,
        review_id: ReviewId,
        repo_id: Option<RepoId>,
        path: &RepoPath,
        side: Side,
    ) -> Result<(RepoId, BlobOid), ToolError> {
        let snap = self.snapshot(review_id).await?;
        let resolved = snap
            .resolved
            .ok_or_else(|| ToolError::Invalid("review targets are not resolved yet".into()))?;
        let candidates: Vec<_> = resolved
            .iter()
            .filter(|t| repo_id.is_none_or(|r| r == t.repo_id))
            .collect();
        let mut found = None;
        for t in candidates {
            let r = match side {
                Side::Base => &t.base,
                Side::Head => &t.head,
            };
            let ref_spec = match &r.source {
                moor_protocol::ResolvedSource::Commit { oid } => RefSpec::Commit { oid: *oid },
                moor_protocol::ResolvedSource::WorkingTree { .. } => RefSpec::WorkingTree,
            };
            let Response::TreeSnapshot { snapshot } = self
                .conn()?
                .request(Request::TreeSnapshot {
                    repo_id: t.repo_id,
                    ref_spec,
                })
                .await?
            else {
                return Err(ToolError::Shape);
            };
            let oid = snapshot.entries.iter().find_map(|e| match &e.kind {
                TreeEntryKind::File { oid, .. } | TreeEntryKind::Symlink { oid }
                    if e.path == *path =>
                {
                    Some(*oid)
                }
                _ => None,
            });
            if let Some(oid) = oid {
                if found.is_some() {
                    return Err(ToolError::Invalid(format!(
                        "{path} exists in more than one repo; pass repo_id"
                    )));
                }
                found = Some((t.repo_id, oid));
            }
        }
        found.ok_or_else(|| ToolError::Invalid(format!("{path} not found on the {side:?} side")))
    }

    async fn collect_render(
        &self,
        request: Request,
    ) -> Result<
        (
            moor_protocol::FileRenderHeader,
            Vec<moor_protocol::RenderChunk>,
        ),
        ToolError,
    > {
        let (_, mut rx) = self.conn()?.stream(request).await?;
        let mut header = None;
        let mut chunks = Vec::new();
        while let Some(item) = rx.recv().await {
            match item? {
                StreamItem::Header { header: h } => header = Some(h),
                StreamItem::Chunk { chunk, .. } => chunks.push(chunk),
                StreamItem::ReviewSnapshot { .. } | StreamItem::TreeSnapshot { .. } => {}
            }
        }
        chunks.sort_by_key(|c| c.index);
        Ok((header.ok_or(ToolError::Shape)?, chunks))
    }

    async fn subscribe_events(&self, p: tools::SubscribeEvents) -> Result<Value, ToolError> {
        let scope = match (p.review_id, p.workspace_id, p.awaiting_agent) {
            (Some(review_id), _, _) => SubscribeScope::Review { review_id },
            (None, Some(workspace_id), _) => SubscribeScope::Workspace { workspace_id },
            (None, None, Some(agent)) => SubscribeScope::AwaitingAgent { agent },
            (None, None, None) => SubscribeScope::All,
        };
        let since = match p.since_seq {
            Some(seq) => Since::After { seq },
            None => Since::Now,
        };
        let client = self.conn()?;
        // Drop anything left over from an earlier poll.
        while tokio::time::timeout(Duration::ZERO, client.next_unsolicited())
            .await
            .is_ok_and(|m| m.is_some())
        {}
        let Response::Subscribed { seq: head } = client
            .request(Request::Subscribe {
                scope: scope.clone(),
                since,
            })
            .await?
        else {
            return Err(ToolError::Shape);
        };
        let deadline = tokio::time::Instant::now() + Duration::from_millis(p.timeout_ms);
        let mut events: Vec<Event> = Vec::new();
        let mut last_seq: Seq = p.since_seq.unwrap_or(head);
        while events.len() < p.max {
            // Once something arrived, only drain what is already queued.
            let wait = if events.is_empty() {
                deadline.saturating_duration_since(tokio::time::Instant::now())
            } else {
                Duration::from_millis(20)
            };
            match tokio::time::timeout(wait, client.next_unsolicited()).await {
                Ok(Some(Unsolicited::Event(e))) => {
                    last_seq = e.seq;
                    events.push(e);
                }
                Ok(Some(Unsolicited::Error(RpcError::SeqTooOld { oldest }))) => {
                    return Err(ToolError::Invalid(format!(
                        "since_seq is older than the daemon's backlog; restart from {oldest}"
                    )));
                }
                Ok(Some(_)) => {}
                Ok(None) => return Err(ClientError::Closed.into()),
                Err(_) => break,
            }
        }
        let _ = client.request(Request::Unsubscribe { scope }).await;
        Ok(json!({ "events": events, "last_seq": last_seq }))
    }
}

fn line_range(start: u32, end: Option<u32>) -> Result<LineRange, ToolError> {
    let s = LineNo::new(start).ok_or_else(|| ToolError::Invalid("lines start at 1".into()))?;
    let e = LineNo::new(end.unwrap_or(start))
        .ok_or_else(|| ToolError::Invalid("lines start at 1".into()))?;
    Ok(LineRange::new(s, e)?)
}

/// MCP tool result: the value as pretty JSON text plus as structured content.
fn tool_ok(v: &Value) -> Value {
    json!({
        "content": [{ "type": "text", "text": serde_json::to_string_pretty(v).unwrap_or_default() }],
        "structuredContent": v,
    })
}

fn tool_err(e: &ToolError) -> Value {
    json!({
        "content": [{ "type": "text", "text": e.to_string() }],
        "isError": true,
    })
}
