//! Starting a daemon on demand and proxying stdio to it.
//!
//! One daemon per machine (the store is single-process); every other way in
//! goes through its socket. `moord --stdio` — what `ssh host moord --stdio`
//! runs — therefore does not serve inline: it connects to the local socket,
//! starting a detached daemon first if nothing answers, and pipes bytes.
//! The CLI uses the same [`ensure_daemon`] for local contexts.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use tokio::net::UnixStream;

/// How long to wait for a freshly started daemon to listen.
pub const START_TIMEOUT: Duration = Duration::from_secs(15);

/// What to run to start a daemon.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DaemonSpec {
    /// The `moord` binary. Default: the one next to the current executable,
    /// else `moord` on `PATH`.
    pub program: PathBuf,
    pub data_dir: PathBuf,
    pub socket: PathBuf,
    /// Exit after this many seconds without connections; `None` runs forever.
    pub idle_exit: Option<u64>,
    /// Also listen for WebSocket clients here.
    pub ws: Option<std::net::SocketAddr>,
}

impl DaemonSpec {
    /// Sensible spec for `data_dir`, socket at `<data_dir>/moord.sock`.
    #[must_use]
    pub fn for_data_dir(data_dir: PathBuf) -> Self {
        Self {
            program: sibling_binary("moord"),
            socket: data_dir.join("moord.sock"),
            data_dir,
            idle_exit: None,
            ws: None,
        }
    }

    fn args(&self) -> Vec<String> {
        let mut a = vec![
            "--data-dir".to_string(),
            self.data_dir.to_string_lossy().into_owned(),
            "--socket".to_string(),
            self.socket.to_string_lossy().into_owned(),
        ];
        if let Some(s) = self.idle_exit {
            a.push("--idle-exit".into());
            a.push(s.to_string());
        }
        if let Some(ws) = self.ws {
            a.push("--ws".into());
            a.push(ws.to_string());
        }
        a
    }
}

/// `name` next to the running executable if present, else bare `name` (PATH).
#[must_use]
pub fn sibling_binary(name: &str) -> PathBuf {
    std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|d| d.join(name)))
        .filter(|p| p.exists())
        .unwrap_or_else(|| PathBuf::from(name))
}

/// Whether something accepts connections on `socket` right now.
pub async fn is_listening(socket: &Path) -> bool {
    UnixStream::connect(socket).await.is_ok()
}

/// Start a daemon detached from this process: no inherited stdio, and
/// immune to the `SIGHUP` an ending ssh session sends (via `nohup` when
/// available). Its log goes to `<data_dir>/moord.log`.
pub fn spawn_detached(spec: &DaemonSpec) -> std::io::Result<()> {
    std::fs::create_dir_all(&spec.data_dir)?;
    let log = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(spec.data_dir.join("moord.log"))?;
    let use_nohup = Command::new("nohup")
        .arg("--version")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_ok();
    let mut cmd = if use_nohup {
        let mut c = Command::new("nohup");
        c.arg(&spec.program);
        c
    } else {
        Command::new(&spec.program)
    };
    cmd.args(spec.args())
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(log)
        .spawn()?;
    Ok(())
}

/// Make sure a daemon answers on `spec.socket`, starting one if needed.
/// Returns whether one was started.
pub async fn ensure_daemon(spec: &DaemonSpec) -> std::io::Result<bool> {
    if is_listening(&spec.socket).await {
        return Ok(false);
    }
    spawn_detached(spec)?;
    let start = Instant::now();
    while !is_listening(&spec.socket).await {
        if start.elapsed() > START_TIMEOUT {
            return Err(std::io::Error::new(
                std::io::ErrorKind::TimedOut,
                format!(
                    "daemon did not start listening on {} (see {})",
                    spec.socket.display(),
                    spec.data_dir.join("moord.log").display()
                ),
            ));
        }
        tokio::time::sleep(Duration::from_millis(30)).await;
    }
    Ok(true)
}

/// Pipe this process's stdin/stdout to the daemon socket until either side
/// closes.
pub async fn proxy_stdio(socket: &Path) -> std::io::Result<()> {
    let mut upstream = UnixStream::connect(socket).await?;
    let mut stdio = tokio::io::join(tokio::io::stdin(), tokio::io::stdout());
    tokio::io::copy_bidirectional(&mut stdio, &mut upstream).await?;
    Ok(())
}
