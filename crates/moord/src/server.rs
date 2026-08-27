//! Accept loops. Each accepted stream is served by `connection::serve` in
//! its own task.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use tokio::net::UnixListener;
use tokio_util::sync::CancellationToken;

use crate::connection;
use crate::daemon::Daemon;

/// A bound unix socket. Removes the socket file on drop.
#[derive(Debug)]
pub struct UnixServer {
    listener: UnixListener,
    path: PathBuf,
}

impl UnixServer {
    /// Bind `path`, replacing a stale socket file left by a crashed daemon.
    /// A live daemon on the same path is detected by connecting first.
    pub fn bind(path: &Path) -> std::io::Result<Self> {
        if path.exists() {
            if std::os::unix::net::UnixStream::connect(path).is_ok() {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::AddrInUse,
                    format!("another moord is listening on {}", path.display()),
                ));
            }
            std::fs::remove_file(path)?;
        }
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        Ok(Self {
            listener: UnixListener::bind(path)?,
            path: path.to_path_buf(),
        })
    }

    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Accept until `shutdown` fires. In-flight connections are dropped when
    /// the returned future completes.
    pub async fn run(self, daemon: Arc<Daemon>, shutdown: CancellationToken) {
        let tasks = tokio::task::JoinSet::<()>::new();
        let mut tasks = tasks;
        loop {
            tokio::select! {
                () = shutdown.cancelled() => break,
                accepted = self.listener.accept() => match accepted {
                    Ok((stream, _)) => {
                        let d = Arc::clone(&daemon);
                        tasks.spawn(async move {
                            if let Err(e) = connection::serve(d, stream).await {
                                tracing::debug!(error = %e, "connection ended");
                            }
                        });
                    }
                    Err(e) => {
                        tracing::warn!(error = %e, "accept failed");
                    }
                },
            }
        }
        tasks.abort_all();
    }
}

impl Drop for UnixServer {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.path);
    }
}

/// Serve exactly one client over stdin/stdout (`ssh host moord --stdio`).
pub async fn serve_stdio(daemon: Arc<Daemon>) -> Result<(), connection::ConnectionError> {
    let stream = tokio::io::join(tokio::io::stdin(), tokio::io::stdout());
    connection::serve(daemon, stream).await
}
