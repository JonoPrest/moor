//! The MCP server: `initialize`, `tools/list`, `tools/call`, proxied to the
//! daemon through [`moord::ops::Ops`]. One request at a time, in order — MCP
//! clients pipeline rarely and the daemon connection is shared, so
//! serialising keeps event long-polls from interleaving with other calls.

use std::path::PathBuf;
use std::time::Duration;

use moor_protocol::{
    AgentVia, Anchor, Author, BuildInfo, ClientId, CommentKind, Human, Mutation, NonEmpty,
    RenderOpts, RepoPath, ReviewTarget, Since, SubscribeScope,
};
use moord::client::{Client, Identity};
use moord::ops::{Ops, OpsError};
use moord::render_text as text;
use serde_json::{Value, json};

use crate::jsonrpc::{self, Incoming, Outgoing};
use crate::tools;

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
    #[error(transparent)]
    Ops(#[from] OpsError),
}

impl From<moord::client::ClientError> for ToolError {
    fn from(e: moord::client::ClientError) -> Self {
        ToolError::Ops(e.into())
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
    ops: Option<Ops>,
}

impl Server {
    #[must_use]
    pub fn new(endpoint: Endpoint, agent: AgentIdentity, build: BuildInfo) -> Self {
        Self {
            endpoint,
            agent,
            build,
            ops: None,
        }
    }

    /// The daemon connection, once `initialize` has run.
    #[must_use]
    pub fn client(&self) -> Option<&Client> {
        self.ops.as_ref().map(Ops::client)
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
        self.ops = Some(Ops::new(client));
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

    fn ops(&self) -> Result<&Ops, ToolError> {
        self.ops.as_ref().ok_or(ToolError::NotInitialized)
    }

    fn ops_mut(&mut self) -> Result<&mut Ops, ToolError> {
        self.ops.as_mut().ok_or(ToolError::NotInitialized)
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
        let ops = self.ops()?;
        match name {
            "list_workspaces" => Ok(json!({ "workspaces": ops.workspaces().await? })),
            "list_reviews" => {
                let p: tools::ListReviews = serde_json::from_value(args)?;
                Ok(json!({ "reviews": ops.reviews(p.workspace_id).await? }))
            }
            "get_review" => {
                let p: tools::ByReview = serde_json::from_value(args)?;
                let snap = ops.snapshot(p.review_id).await?;
                let files = ops.files(p.review_id).await?;
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
                let render_opts = RenderOpts {
                    ignore_whitespace: p.ignore_whitespace,
                    context_lines: p
                        .context_lines
                        .unwrap_or(RenderOpts::default().context_lines),
                };
                let (file, header, chunks) = ops
                    .diff(p.review_id, p.repo_id, &p.path, render_opts)
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
                let (repo_id, blob_oid, header, chunks) =
                    ops.file_at(p.review_id, p.repo_id, &path, p.side).await?;
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
                let snap = ops.snapshot(p.review_id).await?;
                Ok(json!({ "threads": snap.threads, "comments": snap.comments, "seq": snap.seq }))
            }
            "subscribe_events" => {
                let p: tools::SubscribeEvents = serde_json::from_value(args)?;
                let scope = match (p.review_id, p.workspace_id, p.awaiting_agent) {
                    (Some(review_id), _, _) => SubscribeScope::Review { review_id },
                    (None, Some(workspace_id), _) => SubscribeScope::Workspace { workspace_id },
                    (None, None, Some(agent)) => SubscribeScope::AwaitingAgent { agent },
                    (None, None, None) => SubscribeScope::All,
                };
                let since = p.since_seq.map_or(Since::Now, |seq| Since::After { seq });
                let polled = ops
                    .poll_events(scope, since, Duration::from_millis(p.timeout_ms), p.max)
                    .await?;
                Ok(json!({ "events": polled.events, "last_seq": polled.last_seq }))
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
                let ops = self.ops_mut()?;
                let (review_id, event) =
                    ops.create_review(p.workspace_id, p.title, targets).await?;
                let snap = ops.snapshot(review_id).await?;
                Ok(json!({ "review": snap.review, "resolved": snap.resolved, "event": event }))
            }
            "update_review" => {
                let p: tools::UpdateReview = serde_json::from_value(args)?;
                let event = self
                    .ops_mut()?
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
                let (comment_id, event) = self
                    .ops_mut()?
                    .reply(p.review_id, p.thread_id, p.body)
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
                let event = self.ops_mut()?.mutate(m).await?;
                Ok(json!({ "event": event }))
            }
            "request_review" => {
                let p: tools::RequestReview = serde_json::from_value(args)?;
                let event = self
                    .ops_mut()?
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
        let ops = self.ops_mut()?;
        let anchor = match (p.path, p.start_line) {
            (None, Some(_)) => {
                return Err(ToolError::Invalid("start_line needs a path".into()));
            }
            (None, None) => Anchor::Review,
            (Some(path), start) => {
                let path = RepoPath::new(path)?;
                ops.anchor(
                    p.review_id,
                    p.repo_id,
                    &path,
                    p.side,
                    start.map(|s| (s, p.end_line)),
                )
                .await?
            }
        };
        let (t, event) = ops
            .new_thread(p.review_id, CommentKind::Note, anchor, p.body)
            .await?;
        Ok(thread_json(t, &event))
    }

    async fn suggest(&mut self, p: tools::Suggest) -> Result<Value, ToolError> {
        let ops = self.ops_mut()?;
        let path = RepoPath::new(p.path)?;
        let anchor = ops
            .anchor(
                p.review_id,
                p.repo_id,
                &path,
                p.side,
                Some((p.start_line, p.end_line)),
            )
            .await?;
        let (t, event) = ops
            .new_thread(
                p.review_id,
                CommentKind::Suggestion { patch: p.patch },
                anchor,
                p.body,
            )
            .await?;
        Ok(thread_json(t, &event))
    }
}

fn thread_json(t: moord::ops::NewThread, event: &moor_protocol::Event) -> Value {
    json!({ "comment_id": t.comment_id, "thread_id": t.thread_id, "event": event })
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
