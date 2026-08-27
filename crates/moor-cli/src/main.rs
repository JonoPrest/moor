//! `moor`: command-line client for `moord` (plan 2.6). Every subcommand is a
//! printer over [`moord::ops::Ops`]; `--json` prints the protocol values
//! verbatim for scripting.

use std::path::PathBuf;
use std::time::Duration;

use anyhow::{Context, bail};
use clap::{Args, Parser, Subcommand, ValueEnum};
use moor_protocol::{
    AgentVia, Anchor, Author, BuildInfo, ClientId, CommentKind, Event, EventBody, Mutation,
    NonEmpty, RefSpec, RenderOpts, RepoId, RepoPath, ReviewId, ReviewTarget, Seq, Side, Since,
    SubscribeScope, ThreadId, WorkspaceId,
};
use moord::client::{Client, Identity};
use moord::ops::Ops;
use moord::render_text;
use serde::Serialize;
use std::fmt::Write as _;

#[derive(Debug, Parser)]
#[command(name = "moor", version, about)]
struct Cli {
    /// Daemon unix socket. Default: `<data-dir>/moord.sock`.
    #[arg(long, env = "MOOR_SOCKET", global = true)]
    socket: Option<PathBuf>,
    /// Daemon WebSocket URL (`ws://host:port`); overrides `--socket`.
    #[arg(long, env = "MOOR_WS_URL", global = true)]
    ws: Option<String>,
    /// Where state lives, used only to find the default socket.
    #[arg(long, env = "MOOR_DATA_DIR", global = true)]
    data_dir: Option<PathBuf>,
    /// Print protocol values as JSON instead of text.
    #[arg(long, global = true)]
    json: bool,
    /// Your name for attribution. Default: `$USER`.
    #[arg(long, env = "MOOR_USER", global = true)]
    user: Option<String>,
    /// Act as this agent (attribution `Agent{via: Cli}`) instead of a human.
    #[arg(long, env = "MOOR_AGENT", global = true)]
    agent: Option<String>,
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Debug, Subcommand)]
enum Cmd {
    /// Workspaces and their repos.
    #[command(subcommand)]
    Workspace(WorkspaceCmd),
    /// Reviews.
    #[command(subcommand)]
    Review(ReviewCmd),
    /// Changed files in a review.
    Files { review: ReviewId },
    /// Diff of one changed file.
    Diff {
        review: ReviewId,
        path: String,
        #[arg(long)]
        repo: Option<RepoId>,
        /// Ignore whitespace.
        #[arg(short = 'w', long)]
        ignore_whitespace: bool,
        /// Context lines.
        #[arg(short = 'U', long, default_value_t = 3)]
        context: u32,
    },
    /// A whole file at the review's head (or base).
    Show {
        review: ReviewId,
        path: String,
        #[arg(long)]
        repo: Option<RepoId>,
        #[arg(long, value_enum, default_value_t = SideArg::Head)]
        side: SideArg,
    },
    /// Comments and threads.
    #[command(subcommand)]
    Comment(CommentCmd),
    /// Print events; `--follow` keeps waiting for more.
    Events {
        #[arg(long)]
        follow: bool,
        #[arg(long)]
        review: Option<ReviewId>,
        #[arg(long)]
        workspace: Option<WorkspaceId>,
        /// Only `ReviewRequested` events addressed to this agent name.
        #[arg(long)]
        awaiting: Option<String>,
        /// Replay everything after this log position first.
        #[arg(long)]
        since: Option<u64>,
    },
}

#[derive(Debug, Subcommand)]
enum WorkspaceCmd {
    /// Create a workspace; prints its id.
    Add {
        name: String,
    },
    List,
    /// Attach a git repository to a workspace; prints the repo id.
    Attach {
        workspace: WorkspaceId,
        path: PathBuf,
        /// Display name. Default: the directory name.
        #[arg(long)]
        name: Option<String>,
    },
}

#[derive(Debug, Subcommand)]
enum ReviewCmd {
    /// Create a review; prints its id.
    Create {
        #[arg(long)]
        workspace: WorkspaceId,
        /// Base ref: branch name, tag:NAME, full commit oid, `HEAD`,
        /// `upstream`, or `worktree`.
        #[arg(long)]
        base: String,
        /// Head ref, same forms as `--base`.
        #[arg(long)]
        head: String,
        /// Repo to review. Optional when the workspace has exactly one.
        #[arg(long)]
        repo: Option<RepoId>,
        #[arg(long)]
        title: Option<String>,
    },
    List {
        #[arg(long)]
        workspace: WorkspaceId,
    },
    /// Review, targets, files and thread counts.
    Show { review: ReviewId },
}

#[derive(Debug, Subcommand)]
enum CommentCmd {
    /// Start a thread on the review, a file, or a line range.
    Add(AddComment),
    Reply {
        review: ReviewId,
        thread: ThreadId,
        #[arg(long)]
        body: String,
    },
    Resolve {
        review: ReviewId,
        thread: ThreadId,
    },
    List {
        review: ReviewId,
    },
}

#[derive(Debug, Args)]
struct AddComment {
    review: ReviewId,
    #[arg(long)]
    body: String,
    /// Anchor to this file (whole file unless `--line`/`--lines`).
    #[arg(long)]
    path: Option<String>,
    #[arg(long)]
    repo: Option<RepoId>,
    #[arg(long, value_enum, default_value_t = SideArg::Head)]
    side: SideArg,
    /// One line.
    #[arg(long, conflicts_with = "lines")]
    line: Option<u32>,
    /// A range, `START:END`.
    #[arg(long, value_parser = parse_lines)]
    lines: Option<(u32, u32)>,
    /// Attach a unified-diff suggestion the reviewer can apply.
    #[arg(long)]
    patch: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
enum SideArg {
    Base,
    Head,
}

impl From<SideArg> for Side {
    fn from(s: SideArg) -> Self {
        match s {
            SideArg::Base => Side::Base,
            SideArg::Head => Side::Head,
        }
    }
}

fn parse_lines(s: &str) -> Result<(u32, u32), String> {
    let (a, b) = s
        .split_once(':')
        .ok_or_else(|| "expected START:END".to_string())?;
    let a = a.parse().map_err(|e| format!("start: {e}"))?;
    let b = b.parse().map_err(|e| format!("end: {e}"))?;
    Ok((a, b))
}

/// `worktree` / `upstream` / `HEAD` / `tag:NAME` / 40-hex commit / branch.
fn parse_ref(s: &str) -> anyhow::Result<RefSpec> {
    Ok(match s {
        "worktree" | "wt" | "working-tree" => RefSpec::WorkingTree,
        "upstream" | "@{upstream}" | "@{u}" => RefSpec::Upstream,
        "HEAD" => RefSpec::Head,
        _ => {
            if let Some(tag) = s.strip_prefix("tag:") {
                RefSpec::Tag { name: tag.into() }
            } else if s.len() == 40 && s.bytes().all(|b| b.is_ascii_hexdigit()) {
                RefSpec::Commit { oid: s.parse()? }
            } else {
                RefSpec::Branch { name: s.into() }
            }
        }
    })
}

fn default_data_dir() -> anyhow::Result<PathBuf> {
    if let Ok(x) = std::env::var("XDG_DATA_HOME") {
        return Ok(PathBuf::from(x).join("moor"));
    }
    let home = std::env::var("HOME").context("HOME is not set")?;
    Ok(PathBuf::from(home).join(".local/share/moor"))
}

async fn connect(cli: &Cli) -> anyhow::Result<Ops> {
    let machine = gethostname::gethostname().to_string_lossy().into_owned();
    let name = cli
        .user
        .clone()
        .or_else(|| std::env::var("USER").ok())
        .unwrap_or_else(|| "anonymous".into());
    let author = match &cli.agent {
        Some(agent) => Author::Agent {
            name: agent.clone(),
            model: std::env::var("MOOR_AGENT_MODEL").unwrap_or_else(|_| "unknown".into()),
            session_id: std::env::var("MOOR_SESSION_ID").unwrap_or_default(),
            invoked_by: Some(moor_protocol::Human { name, machine }),
            via: AgentVia::Cli,
        },
        None => Author::Human { name, machine },
    };
    let (ts, r) = moord::ids::fresh_parts();
    let identity = Identity {
        client_id: ClientId::from_parts(ts, r),
        client: BuildInfo {
            name: "moor".into(),
            version: env!("CARGO_PKG_VERSION").into(),
        },
        author,
    };
    let client = if let Some(url) = &cli.ws {
        Client::connect_ws(url, identity).await?
    } else {
        let socket = match &cli.socket {
            Some(s) => s.clone(),
            None => cli
                .data_dir
                .clone()
                .map_or_else(default_data_dir, Ok)?
                .join("moord.sock"),
        };
        Client::connect_unix(&socket, identity)
            .await
            .with_context(|| format!("connecting to {} (is moord running?)", socket.display()))?
    };
    Ok(Ops::new(client))
}

/// `review show` output: the snapshot plus the changed files.
#[derive(Debug, Serialize)]
struct Shown<'a> {
    #[serde(flatten)]
    snapshot: &'a moor_protocol::ReviewSnapshot,
    files: &'a [moor_protocol::FileChange],
}

/// Print `v` as JSON when `--json`, else `text`.
fn emit<T: Serialize>(json: bool, v: &T, text: impl FnOnce() -> String) -> anyhow::Result<()> {
    if json {
        println!("{}", serde_json::to_string_pretty(v)?);
    } else {
        let t = text();
        if !t.is_empty() {
            println!("{}", t.trim_end_matches('\n'));
        }
    }
    Ok(())
}

fn event_line(e: &Event) -> String {
    let who = match &e.author {
        Author::Human { name, .. } => name.clone(),
        Author::Agent { name, .. } => format!("{name} (agent)"),
        Author::Daemon { .. } => "daemon".into(),
    };
    let what = match &e.body {
        EventBody::WorkspaceCreated { workspace, .. } => {
            format!("workspace created {}", workspace.name)
        }
        EventBody::WorkspaceUpdated { .. } => "workspace updated".into(),
        EventBody::RepoAttached { repo, .. } => format!("repo attached {}", repo.path),
        EventBody::RepoDetached { repo_id, .. } => format!("repo detached {repo_id}"),
        EventBody::ReviewCreated { review } => {
            format!("review created {} {}", review.id, review.title)
        }
        EventBody::ReviewUpdated { review_id, .. } => format!("review updated {review_id}"),
        EventBody::ReviewDeleted { review_id } => format!("review deleted {review_id}"),
        EventBody::ReviewTargetsResolved { review_id, .. } => {
            format!("targets resolved {review_id}")
        }
        EventBody::CommentCreated { comment } => format!(
            "comment {} on {}: {}",
            comment.id,
            anchor_text(&comment.anchor),
            comment.body
        ),
        EventBody::CommentEdited { comment_id, .. } => format!("comment edited {comment_id}"),
        EventBody::CommentDeleted { comment_id, .. } => format!("comment deleted {comment_id}"),
        EventBody::CommentReanchored { .. } => "comments re-anchored".into(),
        EventBody::ThreadResolved { thread_id, .. } => format!("thread resolved {thread_id}"),
        EventBody::ThreadUnresolved { thread_id, .. } => format!("thread reopened {thread_id}"),
        EventBody::FileViewed { path, .. } => format!("viewed {path}"),
        EventBody::FileUnviewed { path, .. } => format!("unviewed {path}"),
        EventBody::ReviewRequested { agent, note, .. } => {
            format!("review requested from {agent}: {note}")
        }
        EventBody::SuggestionApplied { comment_id, .. } => {
            format!("suggestion applied {comment_id}")
        }
    };
    format!("#{} {who}: {what}", e.seq)
}

fn anchor_text(a: &Anchor) -> String {
    match a {
        Anchor::Review => "review".into(),
        Anchor::File { path, .. } => path.to_string(),
        Anchor::Lines {
            path, side, lines, ..
        } => {
            format!("{path}:{}-{} ({side:?})", lines.start(), lines.end())
        }
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    let json = cli.json;
    let mut ops = connect(&cli).await?;
    match cli.cmd {
        Cmd::Workspace(c) => workspace(&mut ops, c, json).await,
        Cmd::Review(c) => review(&mut ops, c, json).await,
        Cmd::Comment(c) => comment(&mut ops, c, json).await,
        Cmd::Files { .. } | Cmd::Diff { .. } | Cmd::Show { .. } => {
            content(&ops, cli.cmd, json).await
        }
        Cmd::Events {
            follow,
            review,
            workspace,
            awaiting,
            since,
        } => events(&ops, follow, review, workspace, awaiting, since, json).await,
    }
}

async fn workspace(ops: &mut Ops, cmd: WorkspaceCmd, json: bool) -> anyhow::Result<()> {
    match cmd {
        WorkspaceCmd::Add { name } => {
            let (id, event) = ops.create_workspace(name).await?;
            emit(json, &event, || id.to_string())
        }
        WorkspaceCmd::List => {
            let ws = ops.workspaces().await?;
            emit(json, &ws, || {
                ws.iter()
                    .map(|w| {
                        let repos: Vec<String> = w
                            .repos
                            .iter()
                            .map(|r| format!("\n  {} {} {}", r.id, r.display_name, r.path))
                            .collect();
                        format!("{} {}{}", w.id, w.name, repos.concat())
                    })
                    .collect::<Vec<_>>()
                    .join("\n")
            })
        }
        WorkspaceCmd::Attach {
            workspace,
            path,
            name,
        } => {
            let path =
                std::fs::canonicalize(&path).with_context(|| format!("{}", path.display()))?;
            let display = name.unwrap_or_else(|| {
                path.file_name()
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_default()
            });
            let (id, event) = ops
                .attach_repo(workspace, path.to_string_lossy().into_owned(), display)
                .await?;
            emit(json, &event, || id.to_string())
        }
    }
}

async fn review(ops: &mut Ops, cmd: ReviewCmd, json: bool) -> anyhow::Result<()> {
    match cmd {
        ReviewCmd::Create {
            workspace,
            base,
            head,
            repo,
            title,
        } => {
            let repo_id = if let Some(r) = repo {
                r
            } else {
                let ws = ops.workspaces().await?;
                let w = ws
                    .iter()
                    .find(|w| w.id == workspace)
                    .ok_or_else(|| anyhow::anyhow!("no workspace {workspace}"))?;
                match w.repos.as_slice() {
                    [only] => only.id,
                    [] => bail!("workspace has no repos; attach one first"),
                    _ => bail!("workspace has several repos; pass --repo"),
                }
            };
            let target = ReviewTarget {
                repo_id,
                base: parse_ref(&base)?,
                head: parse_ref(&head)?,
            };
            let title = title.unwrap_or_else(|| format!("{base}..{head}"));
            let (id, event) = ops
                .create_review(workspace, title, NonEmpty::singleton(target))
                .await?;
            emit(json, &event, || id.to_string())
        }
        ReviewCmd::List { workspace } => {
            let reviews = ops.reviews(workspace).await?;
            emit(json, &reviews, || {
                reviews
                    .iter()
                    .map(|r| format!("{} [{:?}] {}", r.id, r.status, r.title))
                    .collect::<Vec<_>>()
                    .join("\n")
            })
        }
        ReviewCmd::Show { review } => {
            let snap = ops.snapshot(review).await?;
            let files = ops.files(review).await?;
            emit(
                json,
                &Shown {
                    snapshot: &snap,
                    files: &files,
                },
                || {
                    let mut out = format!(
                        "{} [{:?}] {}\n",
                        snap.review.id, snap.review.status, snap.review.title
                    );
                    for t in &snap.review.targets {
                        let _ = writeln!(out, "  target {} {:?}..{:?}", t.repo_id, t.base, t.head);
                    }
                    for f in &files {
                        let _ = writeln!(
                            out,
                            "  {:?} {}",
                            moor_protocol::ChangeKindKind::from(&f.kind),
                            f.path
                        );
                    }
                    let _ = writeln!(
                        out,
                        "  {} threads, {} comments",
                        snap.threads.len(),
                        snap.comments.len()
                    );
                    out
                },
            )
        }
    }
}

async fn content(ops: &Ops, cmd: Cmd, json: bool) -> anyhow::Result<()> {
    match cmd {
        Cmd::Files { review } => {
            let files = ops.files(review).await?;
            emit(json, &files, || {
                files
                    .iter()
                    .map(|f| {
                        format!(
                            "{:?} {}",
                            moor_protocol::ChangeKindKind::from(&f.kind),
                            f.path
                        )
                    })
                    .collect::<Vec<_>>()
                    .join("\n")
            })
        }
        Cmd::Diff {
            review,
            path,
            repo,
            ignore_whitespace,
            context,
        } => {
            let render_opts = RenderOpts {
                ignore_whitespace,
                context_lines: context,
            };
            let (file, header, chunks) = ops.diff(review, repo, &path, render_opts).await?;
            emit(json, &(&file, &header, &chunks), || {
                render_text::render(&header, &chunks)
            })
        }
        Cmd::Show {
            review,
            path,
            repo,
            side,
        } => {
            let path = RepoPath::new(path)?;
            let (_, _, header, chunks) = ops.file_at(review, repo, &path, side.into()).await?;
            emit(json, &(&header, &chunks), || {
                render_text::render_blob(&header, &chunks)
            })
        }
        _ => unreachable!("dispatched by main"),
    }
}

async fn comment(ops: &mut Ops, cmd: CommentCmd, json: bool) -> anyhow::Result<()> {
    match cmd {
        CommentCmd::Add(a) => {
            let lines = a
                .line
                .map(|l| (l, None))
                .or(a.lines.map(|(s, e)| (s, Some(e))));
            let anchor = match a.path {
                None if lines.is_some() => bail!("--line/--lines need --path"),
                None => Anchor::Review,
                Some(p) => {
                    let p = RepoPath::new(p)?;
                    ops.anchor(a.review, a.repo, &p, a.side.into(), lines)
                        .await?
                }
            };
            let kind = match a.patch {
                Some(patch) => CommentKind::Suggestion { patch },
                None => CommentKind::Note,
            };
            let (t, event) = ops.new_thread(a.review, kind, anchor, a.body).await?;
            emit(json, &event, || t.thread_id.to_string())
        }
        CommentCmd::Reply {
            review,
            thread,
            body,
        } => {
            let (id, event) = ops.reply(review, thread, body).await?;
            emit(json, &event, || id.to_string())
        }
        CommentCmd::Resolve { review, thread } => {
            let event = ops
                .mutate(Mutation::ResolveThread {
                    review_id: review,
                    thread_id: thread,
                })
                .await?;
            emit(json, &event, String::new)
        }
        CommentCmd::List { review } => {
            let snap = ops.snapshot(review).await?;
            emit(json, &(&snap.threads, &snap.comments), || {
                let mut out = String::new();
                for t in &snap.threads {
                    let state = match t.resolution {
                        moor_protocol::ThreadResolution::Open => "open",
                        moor_protocol::ThreadResolution::Resolved { .. } => "resolved",
                    };
                    let _ = writeln!(out, "thread {} [{state}]", t.id);
                    for id in std::iter::once(&t.root).chain(t.replies.iter()) {
                        if let Some(c) = snap.comments.iter().find(|c| c.id == *id) {
                            let who = match &c.author {
                                Author::Human { name, .. } | Author::Agent { name, .. } => {
                                    name.as_str()
                                }
                                Author::Daemon { .. } => "daemon",
                            };
                            let _ = writeln!(
                                out,
                                "  {} {who} @ {}: {}",
                                c.id,
                                anchor_text(&c.anchor),
                                c.body
                            );
                        }
                    }
                }
                out
            })
        }
    }
}

async fn events(
    ops: &Ops,
    follow: bool,
    review: Option<ReviewId>,
    workspace: Option<WorkspaceId>,
    awaiting: Option<String>,
    since: Option<u64>,
    json: bool,
) -> anyhow::Result<()> {
    {
        let scope = match (review, workspace, awaiting) {
            (Some(review_id), _, _) => SubscribeScope::Review { review_id },
            (None, Some(workspace_id), _) => SubscribeScope::Workspace { workspace_id },
            (None, None, Some(agent)) => SubscribeScope::AwaitingAgent { agent },
            (None, None, None) => SubscribeScope::All,
        };
        let mut since = since.map_or(Since::Now, |n| Since::After { seq: Seq::new(n) });
        loop {
            let timeout = if follow {
                Duration::from_secs(3600)
            } else {
                Duration::ZERO
            };
            let polled = ops.poll_events(scope.clone(), since, timeout, 1000).await?;
            for e in &polled.events {
                emit(json, e, || event_line(e))?;
            }
            if !follow {
                break;
            }
            since = Since::After {
                seq: polled.last_seq,
            };
        }
        Ok(())
    }
}
