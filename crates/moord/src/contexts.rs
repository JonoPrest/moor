//! Reaching a daemon through a [`Context`]: connect (starting the daemon on
//! demand where the context allows), probe status, start, stop. The CLI,
//! MCP shim and desktop app all go through here so a machine's daemon is
//! managed one way.

use std::path::PathBuf;
use std::process::Stdio;

use moor_config::Context;
use moor_protocol::{Author, BuildInfo, Request, Response};

use crate::client::{Client, ClientError, Identity};
use crate::launch::{self, DaemonSpec};

#[derive(Debug, thiserror::Error)]
pub enum ContextError {
    #[error("{0}")]
    Client(Box<ClientError>),
    #[error("{what}: {source}")]
    Io {
        what: String,
        #[source]
        source: std::io::Error,
    },
    #[error("a WebSocket context's daemon is managed elsewhere")]
    NotManaged,
    #[error("daemon is not running")]
    NotRunning,
    #[error("unexpected response shape")]
    Shape,
}

impl From<ClientError> for ContextError {
    fn from(e: ClientError) -> Self {
        ContextError::Client(Box::new(e))
    }
}

fn io(what: impl Into<String>) -> impl FnOnce(std::io::Error) -> ContextError {
    move |source| ContextError::Io {
        what: what.into(),
        source,
    }
}

/// Whether a context's daemon is up.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Status {
    Running {
        daemon: BuildInfo,
    },
    Stopped,
    /// Could not even ask, e.g. ssh failed. Carries the reason.
    Unreachable {
        reason: String,
    },
}

/// `$XDG_DATA_HOME/moor` or `~/.local/share/moor`.
pub fn default_data_dir() -> std::io::Result<PathBuf> {
    if let Ok(x) = std::env::var("XDG_DATA_HOME") {
        return Ok(PathBuf::from(x).join("moor"));
    }
    let home = std::env::var("HOME")
        .map_err(|_| std::io::Error::new(std::io::ErrorKind::NotFound, "HOME is not set"))?;
    Ok(PathBuf::from(home).join(".local/share/moor"))
}

/// Resolve a `Local` context's paths.
pub fn local_spec(
    data_dir: Option<&PathBuf>,
    socket: Option<&PathBuf>,
) -> std::io::Result<DaemonSpec> {
    let data_dir = match data_dir {
        Some(d) => d.clone(),
        None => default_data_dir()?,
    };
    let mut spec = DaemonSpec::for_data_dir(data_dir);
    if let Some(s) = socket {
        spec.socket.clone_from(s);
    }
    Ok(spec)
}

fn ssh_command(
    host: &str,
    moord: Option<&str>,
    args: &[String],
    ssh: Option<&str>,
) -> tokio::process::Command {
    let mut cmd = tokio::process::Command::new(ssh.unwrap_or("ssh"));
    cmd.arg(host).arg(moord.unwrap_or("moord")).args(args);
    cmd
}

/// Connect through `ctx`. `autostart` starts the daemon if nothing answers
/// (local and ssh contexts only); without it a stopped daemon is
/// [`ContextError::NotRunning`].
pub async fn connect(
    ctx: &Context,
    identity: Identity,
    autostart: bool,
) -> Result<Client, ContextError> {
    match ctx {
        Context::Local { data_dir, socket } => {
            let spec = local_spec(data_dir.as_ref(), socket.as_ref()).map_err(io("data dir"))?;
            if autostart {
                launch::ensure_daemon(&spec)
                    .await
                    .map_err(io("starting the daemon"))?;
            } else if !launch::is_listening(&spec.socket).await {
                return Err(ContextError::NotRunning);
            }
            Ok(Client::connect_unix(&spec.socket, identity).await?)
        }
        Context::Ssh {
            host,
            moord,
            args,
            ssh,
        } => {
            let mut cmd = ssh_command(host, moord.as_deref(), args, ssh.as_deref());
            cmd.arg(if autostart {
                "--stdio"
            } else {
                "--stdio-if-running"
            });
            let mut child = cmd
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .stderr(Stdio::inherit())
                .spawn()
                .map_err(io(format!("running ssh {host}")))?;
            let stdin = child.stdin.take().ok_or_else(|| ContextError::Io {
                what: "ssh stdin".into(),
                source: std::io::Error::other("not piped"),
            })?;
            let stdout = child.stdout.take().ok_or_else(|| ContextError::Io {
                what: "ssh stdout".into(),
                source: std::io::Error::other("not piped"),
            })?;
            // The child lives as long as its pipes; dropping the handle
            // does not kill it.
            let stream = tokio::io::join(stdout, stdin);
            match Client::handshake(stream, identity, moor_protocol::ProtocolVersion::CURRENT).await
            {
                Ok(c) => Ok(c),
                Err(ClientError::Closed) if !autostart => Err(ContextError::NotRunning),
                Err(e) => Err(e.into()),
            }
        }
        Context::Ws { url } => Ok(Client::connect_ws(url, identity).await?),
    }
}

/// Probe without starting anything.
pub async fn status(ctx: &Context) -> Status {
    match ctx {
        Context::Local { data_dir, socket } => {
            let spec = match local_spec(data_dir.as_ref(), socket.as_ref()) {
                Ok(s) => s,
                Err(e) => {
                    return Status::Unreachable {
                        reason: e.to_string(),
                    };
                }
            };
            if !launch::is_listening(&spec.socket).await {
                return Status::Stopped;
            }
            match Client::connect_unix(&spec.socket, probe_identity()).await {
                Ok(c) => Status::Running {
                    daemon: c.welcome.daemon.clone(),
                },
                Err(e) => Status::Unreachable {
                    reason: e.to_string(),
                },
            }
        }
        Context::Ssh { .. } => match connect(ctx, probe_identity(), false).await {
            Ok(c) => Status::Running {
                daemon: c.welcome.daemon.clone(),
            },
            Err(ContextError::NotRunning) => Status::Stopped,
            Err(e) => Status::Unreachable {
                reason: e.to_string(),
            },
        },
        Context::Ws { url } => match Client::connect_ws(url, probe_identity()).await {
            Ok(c) => Status::Running {
                daemon: c.welcome.daemon.clone(),
            },
            Err(ClientError::Codec(_)) => Status::Stopped,
            Err(e) => Status::Unreachable {
                reason: e.to_string(),
            },
        },
    }
}

/// Start the daemon if it is not running. Returns whether it was started.
pub async fn start(ctx: &Context) -> Result<bool, ContextError> {
    match ctx {
        Context::Local { data_dir, socket } => {
            let spec = local_spec(data_dir.as_ref(), socket.as_ref()).map_err(io("data dir"))?;
            launch::ensure_daemon(&spec)
                .await
                .map_err(io("starting the daemon"))
        }
        Context::Ssh { .. } => {
            let was_running = matches!(status(ctx).await, Status::Running { .. });
            drop(connect(ctx, probe_identity(), true).await?);
            Ok(!was_running)
        }
        Context::Ws { .. } => Err(ContextError::NotManaged),
    }
}

/// Ask the daemon to exit. Ok(false) if it was not running.
pub async fn stop(ctx: &Context) -> Result<bool, ContextError> {
    if matches!(ctx, Context::Ws { .. }) {
        return Err(ContextError::NotManaged);
    }
    let client = match connect(ctx, probe_identity(), false).await {
        Ok(c) => c,
        Err(ContextError::NotRunning) => return Ok(false),
        Err(e) => return Err(e),
    };
    match client.request(Request::Shutdown).await? {
        Response::ShuttingDown => Ok(true),
        _ => Err(ContextError::Shape),
    }
}

fn probe_identity() -> Identity {
    let (ts, r) = crate::ids::fresh_parts();
    Identity {
        client_id: moor_protocol::ClientId::from_parts(ts, r),
        client: BuildInfo {
            name: "moor-probe".into(),
            version: env!("CARGO_PKG_VERSION").into(),
        },
        author: Author::Daemon {
            machine: gethostname::gethostname().to_string_lossy().into_owned(),
        },
    }
}
