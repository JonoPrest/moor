//! The tool table: names, descriptions and JSON Schemas advertised by
//! `tools/list`, plus the typed parameter structs `tools/call` decodes into.

use moor_protocol::{RefSpec, RepoId, ReviewId, ReviewStatus, Seq, Side, ThreadId, WorkspaceId};
use serde::Deserialize;
use serde_json::{Value, json};
use strum::{Display, EnumDiscriminants, EnumIter, IntoStaticStr};

/// A decoded `tools/call`: the variant is the tool, the payload its
/// validated arguments. Adding a tool means adding a variant here; the
/// exhaustive matches in `server.rs` and the table test below then insist on
/// a handler and a `tools/list` entry.
#[derive(Debug, Deserialize, EnumDiscriminants)]
#[strum_discriminants(
    name(ToolName),
    derive(EnumIter, IntoStaticStr, Display, Hash, PartialOrd, Ord, Deserialize),
    strum(serialize_all = "snake_case"),
    serde(rename_all = "snake_case")
)]
#[serde(tag = "name", content = "arguments", rename_all = "snake_case")]
pub enum ToolCall {
    ListWorkspaces(NoArgs),
    ListReviews(ListReviews),
    GetReview(ByReview),
    CreateReview(CreateReview),
    UpdateReview(UpdateReview),
    GetDiff(GetDiff),
    GetFile(GetFile),
    ListComments(ByReview),
    AddComment(AddComment),
    Suggest(Suggest),
    Reply(Reply),
    Resolve(Resolve),
    RequestReview(RequestReview),
    SubscribeEvents(SubscribeEvents),
}

/// A call that only reads.
#[derive(Debug)]
pub enum QueryCall {
    ListWorkspaces,
    ListReviews(ListReviews),
    GetReview(ByReview),
    GetDiff(GetDiff),
    GetFile(GetFile),
    ListComments(ByReview),
    SubscribeEvents(SubscribeEvents),
}

/// A call that appends to the log.
#[derive(Debug)]
pub enum MutatingCall {
    CreateReview(CreateReview),
    UpdateReview(UpdateReview),
    AddComment(AddComment),
    Suggest(Suggest),
    Reply(Reply),
    Resolve(Resolve),
    RequestReview(RequestReview),
}

/// A `ToolCall` sorted by whether it mutates, so the read/write split is a
/// type rather than a name list.
#[derive(Debug)]
pub enum Call {
    Query(QueryCall),
    Mutating(MutatingCall),
}

impl ToolCall {
    /// Decode a tool's arguments. `name` was already parsed, so the only
    /// failures are argument shape errors.
    pub fn parse(name: ToolName, arguments: Value) -> Result<Self, serde_json::Error> {
        let name: &'static str = name.into();
        let mut call = serde_json::Map::new();
        call.insert("name".into(), Value::String(name.into()));
        call.insert("arguments".into(), arguments);
        serde_json::from_value(Value::Object(call))
    }

    #[must_use]
    pub fn name(&self) -> ToolName {
        ToolName::from(self)
    }

    #[must_use]
    pub fn classify(self) -> Call {
        match self {
            ToolCall::ListWorkspaces(NoArgs {}) => Call::Query(QueryCall::ListWorkspaces),
            ToolCall::ListReviews(p) => Call::Query(QueryCall::ListReviews(p)),
            ToolCall::GetReview(p) => Call::Query(QueryCall::GetReview(p)),
            ToolCall::GetDiff(p) => Call::Query(QueryCall::GetDiff(p)),
            ToolCall::GetFile(p) => Call::Query(QueryCall::GetFile(p)),
            ToolCall::ListComments(p) => Call::Query(QueryCall::ListComments(p)),
            ToolCall::SubscribeEvents(p) => Call::Query(QueryCall::SubscribeEvents(p)),
            ToolCall::CreateReview(p) => Call::Mutating(MutatingCall::CreateReview(p)),
            ToolCall::UpdateReview(p) => Call::Mutating(MutatingCall::UpdateReview(p)),
            ToolCall::AddComment(p) => Call::Mutating(MutatingCall::AddComment(p)),
            ToolCall::Suggest(p) => Call::Mutating(MutatingCall::Suggest(p)),
            ToolCall::Reply(p) => Call::Mutating(MutatingCall::Reply(p)),
            ToolCall::Resolve(p) => Call::Mutating(MutatingCall::Resolve(p)),
            ToolCall::RequestReview(p) => Call::Mutating(MutatingCall::RequestReview(p)),
        }
    }
}

/// Arguments of a tool that takes none. Rejects stray keys.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NoArgs {}

/// One advertised tool.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Tool {
    pub name: ToolName,
    pub description: &'static str,
    pub input_schema: Value,
}

fn id(desc: &str) -> Value {
    json!({ "type": "string", "description": desc })
}

fn ref_spec() -> Value {
    json!({
        "type": "object",
        "description": "A git ref: {\"type\":\"Branch\",\"name\":..}, {\"type\":\"Commit\",\"oid\":..}, {\"type\":\"Tag\",\"name\":..}, {\"type\":\"WorkingTree\"} or {\"type\":\"Upstream\"}",
        "required": ["type"],
        "properties": { "type": { "type": "string", "enum": ["Branch", "Commit", "Tag", "WorkingTree", "Upstream"] } }
    })
}

/// Every tool, in the order `tools/list` returns them.
#[must_use]
#[allow(clippy::too_many_lines)] // a table, not logic
pub fn all() -> Vec<Tool> {
    vec![
        Tool {
            name: ToolName::ListWorkspaces,
            description: "Workspaces known to the daemon, each with its attached repos.",
            input_schema: json!({ "type": "object", "properties": {}, "additionalProperties": false }),
        },
        Tool {
            name: ToolName::ListReviews,
            description: "Reviews in a workspace. Without workspace_id: the workspace whose attached repo contains this server's working directory.",
            input_schema: json!({
                "type": "object", "additionalProperties": false,
                "properties": { "workspace_id": id("Workspace ULID (optional)") }
            }),
        },
        Tool {
            name: ToolName::GetReview,
            description: "A review with its resolved targets, changed files, threads and comments.",
            input_schema: json!({
                "type": "object", "additionalProperties": false,
                "required": ["review_id"],
                "properties": { "review_id": id("Review ULID") }
            }),
        },
        Tool {
            name: ToolName::CreateReview,
            description: "Create a review over one or more repos. Returns the new review. Without workspace_id: the workspace containing this server's working directory; a target may omit repo_id to mean the repo containing it.",
            input_schema: json!({
                "type": "object", "additionalProperties": false,
                "required": ["title", "targets"],
                "properties": {
                    "workspace_id": id("Workspace ULID (optional)"),
                    "title": { "type": "string" },
                    "targets": {
                        "type": "array", "minItems": 1,
                        "items": {
                            "type": "object", "additionalProperties": false,
                            "required": ["base", "head"],
                            "properties": { "repo_id": id("Repo ULID (optional)"), "base": ref_spec(), "head": ref_spec() }
                        }
                    }
                }
            }),
        },
        Tool {
            name: ToolName::UpdateReview,
            description: "Rename a review or change its status (Open / Archived).",
            input_schema: json!({
                "type": "object", "additionalProperties": false,
                "required": ["review_id", "title", "status"],
                "properties": {
                    "review_id": id("Review ULID"),
                    "title": { "type": "string" },
                    "status": { "type": "string", "enum": ["Open", "Archived"] }
                }
            }),
        },
        Tool {
            name: ToolName::GetDiff,
            description: "The diff of one changed file in a review, as numbered text (old-line new-line mark text).",
            input_schema: json!({
                "type": "object", "additionalProperties": false,
                "required": ["review_id", "path"],
                "properties": {
                    "review_id": id("Review ULID"),
                    "repo_id": id("Repo ULID; only needed when the path exists in more than one repo"),
                    "path": { "type": "string", "description": "Path relative to the repo root" },
                    "ignore_whitespace": { "type": "boolean", "default": false },
                    "context_lines": { "type": "integer", "minimum": 0, "default": 3 }
                }
            }),
        },
        Tool {
            name: ToolName::GetFile,
            description: "Full contents of a file at the review's base or head, numbered. Works for unchanged files too.",
            input_schema: json!({
                "type": "object", "additionalProperties": false,
                "required": ["review_id", "path"],
                "properties": {
                    "review_id": id("Review ULID"),
                    "repo_id": id("Repo ULID; only needed when the review spans several repos"),
                    "path": { "type": "string" },
                    "side": { "type": "string", "enum": ["Base", "Head"], "default": "Head" }
                }
            }),
        },
        Tool {
            name: ToolName::ListComments,
            description: "Threads and comments on a review, with anchors and resolution state.",
            input_schema: json!({
                "type": "object", "additionalProperties": false,
                "required": ["review_id"],
                "properties": { "review_id": id("Review ULID") }
            }),
        },
        Tool {
            name: ToolName::AddComment,
            description: "Start a thread. Anchor to the whole review (no path), a file (path only) or a line range (path + start_line [+ end_line]) on the given side.",
            input_schema: json!({
                "type": "object", "additionalProperties": false,
                "required": ["review_id", "body"],
                "properties": {
                    "review_id": id("Review ULID"),
                    "repo_id": id("Repo ULID; only needed when the path exists in more than one repo"),
                    "path": { "type": "string" },
                    "side": { "type": "string", "enum": ["Base", "Head"], "default": "Head" },
                    "start_line": { "type": "integer", "minimum": 1 },
                    "end_line": { "type": "integer", "minimum": 1, "description": "Defaults to start_line" },
                    "body": { "type": "string" }
                }
            }),
        },
        Tool {
            name: ToolName::Suggest,
            description: "Start a thread carrying a suggested change: a unified diff against the anchored blob that a human can apply.",
            input_schema: json!({
                "type": "object", "additionalProperties": false,
                "required": ["review_id", "path", "start_line", "patch", "body"],
                "properties": {
                    "review_id": id("Review ULID"),
                    "repo_id": id("Repo ULID; only needed when the path exists in more than one repo"),
                    "path": { "type": "string" },
                    "side": { "type": "string", "enum": ["Base", "Head"], "default": "Head" },
                    "start_line": { "type": "integer", "minimum": 1 },
                    "end_line": { "type": "integer", "minimum": 1 },
                    "patch": { "type": "string", "description": "Unified diff against the file at that side" },
                    "body": { "type": "string", "description": "Why" }
                }
            }),
        },
        Tool {
            name: ToolName::Reply,
            description: "Reply in an existing thread.",
            input_schema: json!({
                "type": "object", "additionalProperties": false,
                "required": ["review_id", "thread_id", "body"],
                "properties": {
                    "review_id": id("Review ULID"),
                    "thread_id": id("Thread ULID (equals its root comment id)"),
                    "body": { "type": "string" }
                }
            }),
        },
        Tool {
            name: ToolName::Resolve,
            description: "Mark a thread resolved (or reopen it with resolved=false).",
            input_schema: json!({
                "type": "object", "additionalProperties": false,
                "required": ["review_id", "thread_id"],
                "properties": {
                    "review_id": id("Review ULID"),
                    "thread_id": id("Thread ULID"),
                    "resolved": { "type": "boolean", "default": true }
                }
            }),
        },
        Tool {
            name: ToolName::RequestReview,
            description: "Ask a named agent to review. Subscribers with scope AwaitingAgent for that name are notified.",
            input_schema: json!({
                "type": "object", "additionalProperties": false,
                "required": ["review_id", "agent", "note"],
                "properties": {
                    "review_id": id("Review ULID"),
                    "agent": { "type": "string" },
                    "note": { "type": "string" }
                }
            }),
        },
        Tool {
            name: ToolName::SubscribeEvents,
            description: "Long-poll for events. Returns events matching the scope after since_seq, waiting up to timeout_ms for at least one. Pass the returned last_seq back as since_seq to continue.",
            input_schema: json!({
                "type": "object", "additionalProperties": false,
                "properties": {
                    "review_id": id("Limit to one review"),
                    "workspace_id": id("Limit to one workspace"),
                    "awaiting_agent": { "type": "string", "description": "Only ReviewRequested events addressed to this agent name" },
                    "since_seq": { "type": "integer", "minimum": 0, "description": "Replay after this log position; omit for live only" },
                    "timeout_ms": { "type": "integer", "minimum": 0, "default": 30000 },
                    "max": { "type": "integer", "minimum": 1, "default": 100 }
                }
            }),
        },
    ]
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ListReviews {
    #[serde(default)]
    pub workspace_id: Option<WorkspaceId>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ByReview {
    pub review_id: ReviewId,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TargetSpec {
    #[serde(default)]
    pub repo_id: Option<RepoId>,
    pub base: RefSpec,
    pub head: RefSpec,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CreateReview {
    #[serde(default)]
    pub workspace_id: Option<WorkspaceId>,
    pub title: String,
    pub targets: Vec<TargetSpec>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UpdateReview {
    pub review_id: ReviewId,
    pub title: String,
    pub status: ReviewStatus,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GetDiff {
    pub review_id: ReviewId,
    pub repo_id: Option<RepoId>,
    pub path: String,
    #[serde(default)]
    pub ignore_whitespace: bool,
    pub context_lines: Option<u32>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GetFile {
    pub review_id: ReviewId,
    pub repo_id: Option<RepoId>,
    pub path: String,
    #[serde(default = "head")]
    pub side: Side,
}

fn head() -> Side {
    Side::Head
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AddComment {
    pub review_id: ReviewId,
    pub repo_id: Option<RepoId>,
    pub path: Option<String>,
    #[serde(default = "head")]
    pub side: Side,
    pub start_line: Option<u32>,
    pub end_line: Option<u32>,
    pub body: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Suggest {
    pub review_id: ReviewId,
    pub repo_id: Option<RepoId>,
    pub path: String,
    #[serde(default = "head")]
    pub side: Side,
    pub start_line: u32,
    pub end_line: Option<u32>,
    pub patch: String,
    pub body: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Reply {
    pub review_id: ReviewId,
    pub thread_id: ThreadId,
    pub body: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Resolve {
    pub review_id: ReviewId,
    pub thread_id: ThreadId,
    #[serde(default = "yes")]
    pub resolved: bool,
}

fn yes() -> bool {
    true
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RequestReview {
    pub review_id: ReviewId,
    pub agent: String,
    pub note: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SubscribeEvents {
    pub review_id: Option<ReviewId>,
    pub workspace_id: Option<WorkspaceId>,
    pub awaiting_agent: Option<String>,
    pub since_seq: Option<Seq>,
    #[serde(default = "default_timeout")]
    pub timeout_ms: u64,
    #[serde(default = "default_max")]
    pub max: usize,
}

fn default_timeout() -> u64 {
    30_000
}
fn default_max() -> usize {
    100
}

#[cfg(test)]
mod tests {
    use strum::IntoEnumIterator;

    use super::*;

    #[test]
    fn every_tool_is_advertised_once_in_order() {
        let advertised: Vec<ToolName> = all().into_iter().map(|t| t.name).collect();
        let expected: Vec<ToolName> = ToolName::iter().collect();
        assert_eq!(advertised, expected);
    }

    #[test]
    fn names_round_trip_through_the_wire_form() {
        for name in ToolName::iter() {
            let wire: &'static str = name.into();
            assert_eq!(name.to_string(), wire);
            assert!(wire.chars().all(|c| c.is_ascii_lowercase() || c == '_'));
        }
        let call = ToolCall::parse(ToolName::ListWorkspaces, json!({})).unwrap();
        assert_eq!(call.name(), ToolName::ListWorkspaces);
        assert!(ToolCall::parse(ToolName::ListWorkspaces, json!({ "x": 1 })).is_err());
        assert!(ToolCall::parse(ToolName::GetReview, json!({})).is_err());
    }
}
