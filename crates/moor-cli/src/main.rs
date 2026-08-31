//! `moor`: command-line client for `moord` (plan 2.6). Every subcommand is a
//! printer over [`moord::ops::Ops`]; `--json` prints the protocol values
//! verbatim for scripting.

use std::path::{Path, PathBuf};
use std::time::Duration;

use anyhow::{Context as _, bail};
use clap::{Args, Parser, Subcommand, ValueEnum};
use moor_config::Context;
use moor_protocol::{
    AgentVia, Anchor, Author, BuildInfo, ClientId, CommentKind, Event, EventBody, Mutation,
    NonEmpty, RefSpec, RenderOpts, RepoId, RepoPath, ReviewId, ReviewTarget, Seq, Side, Since,
    SubscribeScope, ThreadId, WorkspaceId,
};
use moord::client::Identity;
use moord::contexts::{self, Status};
use moord::ops::Ops;
use moord::render_text;
use serde::Serialize;
use std::fmt::Write as _;

#[derive(Debug, Parser)]
#[command(name = "moor", version, about)]
struct Cli {
    /// Named context from the config file (see `moor context`). Default:
    /// `local`, an implicit daemon on this machine. There is no persisted
    /// "current" context: pass this flag or set `MOOR_CONTEXT`.
    #[arg(long, short = 'c', env = "MOOR_CONTEXT", global = true)]
    context: Option<String>,
    /// Config file. Default: `$XDG_CONFIG_HOME/moor/config.toml`.
    #[arg(long, env = "MOOR_CONFIG", global = true)]
    config: Option<PathBuf>,
    /// Ad-hoc local context: this daemon socket. Overrides `--context`.
    #[arg(long, env = "MOOR_SOCKET", global = true)]
    socket: Option<PathBuf>,
    /// Ad-hoc context: a daemon WebSocket URL (`ws://host:port`).
    #[arg(long, env = "MOOR_WS_URL", global = true)]
    ws: Option<String>,
    /// Ad-hoc local context: data dir (socket at `<data-dir>/moord.sock`).
    #[arg(long, env = "MOOR_DATA_DIR", global = true)]
    data_dir: Option<PathBuf>,
    /// Fail instead of starting the daemon when it is not running.
    #[arg(long, global = true)]
    no_autostart: bool,
    /// Print protocol values as JSON instead of text.
    #[arg(long, global = true)]
    json: bool,
    /// Your name for attribution. Default: `$USER`.
    #[arg(long, env = "MOOR_USER", global = true)]
    user: Option<String>,
    /// Act as this agent (attribution `Agent{via: Cli}`) instead of a human.
    #[arg(long, env = "MOOR_AGENT", global = true)]
    agent: Option<String>,
    /// With no subcommand: serve the browser UI until Ctrl-C, vite-style.
    /// A path (`moor .`) opens (or creates) that directory's review; no
    /// path opens the workspace menu. `--headless` only ensures the
    /// review exists (e.g. `moor -c hetzner ~/proj --headless` on a
    /// remote context); `--ui desktop` launches the Tauri app instead.
    #[arg(value_name = "PATH")]
    path: Option<PathBuf>,
    #[arg(long, value_enum, default_value_t = Ui::Web, conflicts_with = "headless")]
    ui: Ui,
    /// Shorthand for `--ui headless`.
    #[arg(long)]
    headless: bool,
    #[command(subcommand)]
    cmd: Option<Cmd>,
}

/// How bare `moor` presents the review. A configurable default is future
/// work; `Tui` is reserved.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
enum Ui {
    Web,
    Desktop,
    Headless,
}

#[derive(Debug, Subcommand)]
enum Cmd {
    /// Named daemons to talk to (local, ssh, websocket), like kubectl contexts.
    #[command(subcommand)]
    Context(ContextCmd),
    /// Start, stop or inspect the current context's daemon.
    #[command(subcommand)]
    Daemon(DaemonCmd),
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
        context_lines: u32,
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
enum ContextCmd {
    /// Configured contexts.
    List,
    /// The selected context's name and details (`-c`, `MOOR_CONTEXT`, or
    /// the implicit `local`).
    Show,
    /// A daemon on this machine.
    AddLocal {
        name: String,
        #[arg(long)]
        data_dir: Option<PathBuf>,
        #[arg(long)]
        socket: Option<PathBuf>,
    },
    /// A daemon on another machine via `ssh HOST moord --stdio`.
    AddSsh {
        name: String,
        /// Host as understood by your ssh config (`user@host`, alias).
        host: String,
        /// Remote `moord` binary. Default: `moord` on the remote PATH.
        #[arg(long)]
        moord: Option<String>,
        /// Extra arguments for the remote daemon, e.g. `--data-dir /x`.
        #[arg(long = "arg")]
        args: Vec<String>,
    },
    /// A daemon already listening for WebSocket clients.
    AddWs {
        name: String,
        url: String,
    },
    Remove {
        name: String,
    },
}

#[derive(Debug, Subcommand)]
enum DaemonCmd {
    /// Whether the daemon is running (every context with `--all`).
    Status {
        #[arg(long)]
        all: bool,
    },
    /// Start the daemon if it is not running.
    Start,
    /// Ask the daemon to exit; it restarts on the next connection.
    Stop,
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
        /// Default: the current directory.
        path: Option<PathBuf>,
        /// Display name. Default: the directory name.
        #[arg(long)]
        name: Option<String>,
    },
}

#[derive(Debug, Subcommand)]
enum ReviewCmd {
    /// Create a review; prints its id.
    Create {
        /// Default: the workspace whose attached repo contains the current
        /// directory.
        #[arg(long)]
        workspace: Option<WorkspaceId>,
        /// Base ref: branch name, tag:NAME, full commit oid, `HEAD`,
        /// `upstream`, or `worktree`.
        #[arg(long)]
        base: String,
        /// Head ref, same forms as `--base`.
        #[arg(long)]
        head: String,
        /// Repo to review. Default: the one containing the current
        /// directory, else the workspace's only repo.
        #[arg(long)]
        repo: Option<RepoId>,
        #[arg(long)]
        title: Option<String>,
    },
    List {
        /// Default: the workspace whose attached repo contains the current
        /// directory.
        #[arg(long)]
        workspace: Option<WorkspaceId>,
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

/// The context to use: ad-hoc flags beat `--context` beats the config.
fn resolve_context(cli: &Cli, cfg: &moor_config::Config) -> anyhow::Result<(String, Context)> {
    if let Some(url) = &cli.ws {
        return Ok(("--ws".into(), Context::Ws { url: url.clone() }));
    }
    if cli.socket.is_some() || cli.data_dir.is_some() {
        return Ok((
            "--socket".into(),
            Context::Local {
                data_dir: cli.data_dir.clone(),
                socket: cli.socket.clone(),
            },
        ));
    }
    Ok(cfg.resolve(cli.context.as_deref())?)
}

fn config_path(cli: &Cli) -> anyhow::Result<PathBuf> {
    match &cli.config {
        Some(p) => Ok(p.clone()),
        None => Ok(moor_config::Config::default_path()?),
    }
}

fn identity(cli: &Cli) -> Identity {
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
    Identity {
        client_id: ClientId::from_parts(ts, r),
        client: BuildInfo {
            name: "moor".into(),
            version: env!("CARGO_PKG_VERSION").into(),
        },
        author,
    }
}

async fn connect(cli: &Cli, ctx: &Context) -> anyhow::Result<Ops> {
    let client = contexts::connect(ctx, identity(cli), !cli.no_autostart)
        .await
        .with_context(|| format!("connecting to {}", ctx.describe()))?;
    Ok(Ops::new(client))
}

/// One line of `daemon status --json`.
#[derive(Debug, Serialize)]
struct Row<'a> {
    context: &'a str,
    target: &'a str,
    status: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    daemon: Option<&'a BuildInfo>,
    #[serde(skip_serializing_if = "Option::is_none")]
    reason: Option<&'a str>,
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
    let cfg_path = config_path(&cli)?;
    let mut cfg = moor_config::Config::load(&cfg_path)?;
    if let Some(Cmd::Context(c)) = cli.cmd {
        return context_cmd(&mut cfg, &cfg_path, cli.context.as_deref(), c, json);
    }
    let (name, ctx) = resolve_context(&cli, &cfg)?;
    if let Some(Cmd::Daemon(c)) = cli.cmd {
        return daemon_cmd(&cfg, &name, &ctx, c, json).await;
    }
    let mut ops = connect(&cli, &ctx).await?;
    let Some(cmd) = cli.cmd else {
        return open_ui(&cli, &ctx, &mut ops).await;
    };
    match cmd {
        Cmd::Context(_) | Cmd::Daemon(_) => unreachable!("handled above"),
        Cmd::Workspace(c) => workspace(&mut ops, c, json).await,
        Cmd::Review(c) => review(&mut ops, c, json).await,
        Cmd::Comment(c) => comment(&mut ops, c, json).await,
        Cmd::Files { .. } | Cmd::Diff { .. } | Cmd::Show { .. } => content(&ops, cmd, json).await,
        Cmd::Events {
            follow,
            review,
            workspace,
            awaiting,
            since,
        } => events(&ops, follow, review, workspace, awaiting, since, json).await,
    }
}

/// Bare `moor [path]`: with a path, find or create that directory's
/// review (head = working tree); then serve the browser UI in the
/// foreground on a free port and print the URL (deep-linked when a
/// review was resolved). Without a path: the workspace menu.
async fn open_ui(cli: &Cli, ctx: &Context, ops: &mut Ops) -> anyhow::Result<()> {
    let review_id = if let Some(path) = &cli.path {
        Some(directory_review(ops, ctx, path).await?)
    } else {
        anyhow::ensure!(
            !cli.headless && cli.ui != Ui::Headless,
            "--headless needs a path: it only ensures a review exists"
        );
        None
    };
    let ui = if cli.headless { Ui::Headless } else { cli.ui };
    match (ui, review_id) {
        (Ui::Headless, Some(id)) => {
            println!("{id}");
            return Ok(());
        }
        (Ui::Headless, None) => unreachable!("checked above"),
        (Ui::Desktop, _) => {
            // The Tauri app next to this binary (no deep link yet).
            let app = std::env::current_exe()
                .ok()
                .and_then(|p| p.parent().map(|d| d.join("moor-desktop")))
                .filter(|p| p.exists())
                .ok_or_else(|| anyhow::anyhow!("moor-desktop not found next to moor"))?;
            let mut child = std::process::Command::new(app);
            if let Some(c) = &cli.context {
                child.arg(c);
            }
            child.spawn().context("launching moor-desktop")?;
            return Ok(());
        }
        (Ui::Web, _) => {}
    }
    // The bridge runs its own host, so it needs the daemon socket, not
    // our already-open client.
    let socket = match ctx {
        Context::Local { data_dir, socket } => {
            moord::contexts::local_spec(data_dir.as_ref(), socket.as_ref())?.socket
        }
        Context::Ssh { .. } | Context::Ws { .. } => {
            anyhow::bail!(
                "the web UI needs a local context so far (remote viewing: PLAN 4.6); \
                 `--headless` works on remote contexts"
            )
        }
    };
    let host = moor_client_host::local_config(
        &socket,
        web_identity(cli),
        moor_client_core::IdSeed(moord::ids::fresh_parts().1),
        moor_client_host::KvConfig::Memory,
    );
    let addr = std::net::SocketAddr::from((std::net::Ipv4Addr::LOCALHOST, 0));
    let server = moor_client_web::serve(addr, host).await?;
    let query = review_id.map_or_else(String::new, |id| format!("?review={id}"));
    println!("\n  moor: http://{}/{query}\n", server.addr());
    tokio::signal::ctrl_c().await?;
    server.stop();
    Ok(())
}

/// The review for `path`'s repo: locate (attaching workspace+repo on
/// first use), then find or create the working-tree review. On a remote
/// context nothing local is consulted: the path goes to the daemon
/// verbatim (it must be the repo root on that machine) and the base
/// fallback is `main`.
async fn directory_review(
    ops: &mut Ops,
    ctx: &Context,
    path: &Path,
) -> anyhow::Result<moor_protocol::ReviewId> {
    let local = matches!(ctx, Context::Local { .. });
    let (root, fallback_base) = if local {
        let root = repo_root(path)
            .with_context(|| format!("{} is not inside a git repository", path.display()))?;
        let base = default_branch(&root);
        (root, base)
    } else {
        (path.to_path_buf(), "main".to_owned())
    };
    let dir_name = root
        .file_name()
        .map_or_else(|| "repo".into(), |n| n.to_string_lossy().into_owned());
    let located = if local {
        match ops.locate(&root).await {
            Ok(l) => l,
            Err(moord::ops::OpsError::Invalid(_)) => {
                // First time here: a workspace named after the directory.
                let (ws_id, _) = ops.create_workspace(dir_name.clone()).await?;
                let path = root.to_string_lossy().into_owned();
                ops.attach_repo(ws_id, path, dir_name.clone()).await?;
                ops.locate(&root).await?
            }
            Err(e) => return Err(e.into()),
        }
    } else {
        // `locate` canonicalises locally, so match the daemon's stored
        // paths by string; attach if unknown (the daemon canonicalises
        // and checks it is a git work tree on its machine).
        let wanted = root.to_string_lossy().into_owned();
        let find = |workspaces: &[moor_protocol::Workspace]| {
            workspaces.iter().find_map(|ws| {
                ws.repos
                    .iter()
                    .find(|r| r.path == wanted)
                    .map(|r| moord::ops::Located {
                        workspace: ws.clone(),
                        repo: r.clone(),
                    })
            })
        };
        if let Some(l) = find(&ops.workspaces().await?) {
            l
        } else {
            let (ws_id, _) = ops.create_workspace(dir_name.clone()).await?;
            ops.attach_repo(ws_id, wanted.clone(), dir_name.clone())
                .await?;
            find(&ops.workspaces().await?).ok_or_else(|| {
                anyhow::anyhow!("attached {wanted} but the daemon reports it at a different path; pass that path")
            })?
        }
    };
    working_tree_review(ops, &located, &fallback_base, &dir_name).await
}

/// The open review whose head is this repo's working tree, created if
/// missing (base: upstream, else `fallback_base`).
async fn working_tree_review(
    ops: &mut Ops,
    located: &moord::ops::Located,
    fallback_base: &str,
    dir_name: &str,
) -> anyhow::Result<ReviewId> {
    let reviews = ops.reviews(located.workspace.id).await?;
    let existing = reviews.iter().find(|r| {
        r.status == moor_protocol::ReviewStatus::Open
            && r.targets
                .iter()
                .any(|t| t.repo_id == located.repo.id && t.head == RefSpec::WorkingTree)
    });
    let review_id = if let Some(r) = existing {
        eprintln!("review: {} \"{}\"", r.id, r.title);
        r.id
    } else {
        {
            let target = moor_protocol::ReviewTarget {
                repo_id: located.repo.id,
                base: RefSpec::Upstream,
                head: RefSpec::WorkingTree,
            };
            let targets = moor_protocol::NonEmpty::singleton(target);
            let created = ops
                .create_review(located.workspace.id, dir_name.to_owned(), targets)
                .await;
            let (id, _) = if let Ok(created) = created {
                created
            } else {
                {
                    // No upstream configured: fall back to the default branch.
                    let target = moor_protocol::ReviewTarget {
                        repo_id: located.repo.id,
                        base: RefSpec::Branch {
                            name: fallback_base.to_owned(),
                        },
                        head: RefSpec::WorkingTree,
                    };
                    ops.create_review(
                        located.workspace.id,
                        dir_name.to_owned(),
                        moor_protocol::NonEmpty::singleton(target),
                    )
                    .await?
                }
            };
            eprintln!("review: {id} \"{dir_name}\" (created)");
            id
        }
    };
    Ok(review_id)
}

/// Walk up to the nearest `.git` (a dir in a main checkout, a file in a
/// linked worktree); each worktree is its own root.
fn repo_root(from: &Path) -> Option<PathBuf> {
    let mut dir = from.to_path_buf();
    loop {
        if dir.join(".git").exists() {
            return std::fs::canonicalize(dir).ok();
        }
        if !dir.pop() {
            return None;
        }
    }
}

/// `origin/HEAD`'s target, else `main`.
fn default_branch(root: &Path) -> String {
    std::process::Command::new("git")
        .args(["symbolic-ref", "--short", "refs/remotes/origin/HEAD"])
        .current_dir(root)
        .output()
        .ok()
        .filter(|o| o.status.success())
        .and_then(|o| {
            String::from_utf8(o.stdout)
                .ok()
                .and_then(|s| s.trim().strip_prefix("origin/").map(str::to_owned))
        })
        .unwrap_or_else(|| "main".into())
}

fn web_identity(cli: &Cli) -> moor_client_host::Identity {
    let id = identity(cli);
    moor_client_host::Identity {
        client_id: id.client_id,
        client: id.client,
        author: id.author,
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
            let path = path.unwrap_or_else(|| PathBuf::from("."));
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
            let (workspace, repo_id) = match (workspace, repo) {
                (Some(w), Some(r)) => (w, r),
                (Some(workspace), None) => {
                    let ws = ops.workspaces().await?;
                    let w = ws
                        .iter()
                        .find(|w| w.id == workspace)
                        .ok_or_else(|| anyhow::anyhow!("no workspace {workspace}"))?;
                    let repo_id = match w.repos.as_slice() {
                        [only] => only.id,
                        [] => bail!("workspace has no repos; attach one first"),
                        _ => match ops.locate(Path::new(".")).await {
                            Ok(l) if l.workspace.id == workspace => l.repo.id,
                            _ => bail!("workspace has several repos; pass --repo"),
                        },
                    };
                    (workspace, repo_id)
                }
                (None, repo) => {
                    let l = ops.locate(Path::new(".")).await?;
                    (l.workspace.id, repo.unwrap_or(l.repo.id))
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
            let workspace = match workspace {
                Some(w) => w,
                None => ops.locate(Path::new(".")).await?.workspace.id,
            };
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
            context_lines,
        } => {
            let render_opts = RenderOpts {
                ignore_whitespace,
                context_lines,
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
                Duration::from_hours(1)
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

fn context_cmd(
    cfg: &mut moor_config::Config,
    path: &Path,
    selected: Option<&str>,
    cmd: ContextCmd,
    json: bool,
) -> anyhow::Result<()> {
    match cmd {
        ContextCmd::List => emit(json, cfg, || {
            let mut rows: Vec<String> = cfg
                .contexts
                .iter()
                .map(|(n, c)| format!("{n}\t{}", c.describe()))
                .collect();
            if !cfg.contexts.contains_key(moor_config::DEFAULT_CONTEXT) {
                rows.insert(
                    0,
                    format!("{}\tlocal (implicit)", moor_config::DEFAULT_CONTEXT),
                );
            }
            rows.join("\n")
        }),
        ContextCmd::Show => {
            let (name, ctx) = cfg.resolve(selected)?;
            emit(json, &(&name, &ctx), || {
                format!("{name}\t{}", ctx.describe())
            })
        }
        ContextCmd::AddLocal {
            name,
            data_dir,
            socket,
        } => add(cfg, path, &name, &Context::Local { data_dir, socket }, json),
        ContextCmd::AddSsh {
            name,
            host,
            moord,
            args,
        } => add(
            cfg,
            path,
            &name,
            &Context::Ssh {
                host,
                moord,
                args,
                ssh: None,
            },
            json,
        ),
        ContextCmd::AddWs { name, url } => add(cfg, path, &name, &Context::Ws { url }, json),
        ContextCmd::Remove { name } => {
            let removed = cfg.remove(&name)?;
            cfg.save(path)?;
            emit(json, &removed, || format!("removed {name}"))
        }
    }
}

/// Add (or replace) a context.
fn add(
    cfg: &mut moor_config::Config,
    path: &Path,
    name: &str,
    ctx: &Context,
    json: bool,
) -> anyhow::Result<()> {
    cfg.contexts.insert(name.to_string(), ctx.clone());
    cfg.save(path)?;
    emit(json, &(&name, &ctx), || {
        format!("added {name}\t{}", ctx.describe())
    })
}

async fn daemon_cmd(
    cfg: &moor_config::Config,
    name: &str,
    ctx: &Context,
    cmd: DaemonCmd,
    json: bool,
) -> anyhow::Result<()> {
    match cmd {
        DaemonCmd::Status { all } => {
            let mut targets: Vec<(String, Context)> = if all {
                let mut v: Vec<(String, Context)> = cfg
                    .contexts
                    .iter()
                    .map(|(n, c)| (n.clone(), c.clone()))
                    .collect();
                if !cfg.contexts.contains_key(moor_config::DEFAULT_CONTEXT) {
                    v.insert(0, cfg.resolve(Some(moor_config::DEFAULT_CONTEXT))?);
                }
                v
            } else {
                vec![(name.to_string(), ctx.clone())]
            };
            let mut rows = Vec::new();
            for (n, c) in targets.drain(..) {
                rows.push((n, c.describe(), contexts::status(&c).await));
            }
            let json_rows: Vec<Row<'_>> = rows
                .iter()
                .map(|(n, t, s)| Row {
                    context: n,
                    target: t,
                    status: match s {
                        Status::Running { .. } => "running",
                        Status::Stopped => "stopped",
                        Status::Unreachable { .. } => "unreachable",
                    },
                    daemon: match s {
                        Status::Running { daemon } => Some(daemon),
                        _ => None,
                    },
                    reason: match s {
                        Status::Unreachable { reason } => Some(reason),
                        _ => None,
                    },
                })
                .collect();
            emit(json, &json_rows, || {
                rows.iter()
                    .map(|(n, t, s)| {
                        let st = match s {
                            Status::Running { daemon } => {
                                format!("running ({} {})", daemon.name, daemon.version)
                            }
                            Status::Stopped => "stopped".into(),
                            Status::Unreachable { reason } => format!("unreachable: {reason}"),
                        };
                        format!("{n}\t{t}\t{st}")
                    })
                    .collect::<Vec<_>>()
                    .join("\n")
            })
        }
        DaemonCmd::Start => {
            let started = contexts::start(ctx).await?;
            emit(json, &started, || {
                if started {
                    "started"
                } else {
                    "already running"
                }
                .into()
            })
        }
        DaemonCmd::Stop => {
            let stopped = contexts::stop(ctx).await?;
            emit(json, &stopped, || {
                if stopped { "stopping" } else { "not running" }.into()
            })
        }
    }
}
