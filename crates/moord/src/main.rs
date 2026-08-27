//! `moord` binary. See `moord --help`.

use std::path::PathBuf;
use std::sync::Arc;

use anyhow::Context;
use clap::Parser;
use moor_protocol::BuildInfo;
use moor_review_core::DataDir;
use moord::Daemon;
use moord::server::{UnixServer, serve_stdio};
use tokio_util::sync::CancellationToken;

/// The Moor daemon.
#[derive(Debug, Parser)]
#[command(version)]
struct Args {
    /// Where state lives. Default: `$XDG_DATA_HOME/moor` or `~/.local/share/moor`.
    #[arg(long, env = "MOOR_DATA_DIR")]
    data_dir: Option<PathBuf>,
    /// Socket to listen on. Default: `<data-dir>/moord.sock`.
    #[arg(long, env = "MOOR_SOCKET")]
    socket: Option<PathBuf>,
    /// Serve one client on stdin/stdout instead of listening
    /// (`ssh host moord --stdio`).
    #[arg(long)]
    stdio: bool,
}

fn default_data_dir() -> anyhow::Result<PathBuf> {
    if let Some(x) = std::env::var_os("XDG_DATA_HOME") {
        return Ok(PathBuf::from(x).join("moor"));
    }
    let home = std::env::var_os("HOME").context("neither XDG_DATA_HOME nor HOME is set")?;
    Ok(PathBuf::from(home).join(".local/share/moor"))
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .with_writer(std::io::stderr)
        .init();
    let args = Args::parse();
    let data_dir = match args.data_dir {
        Some(d) => d,
        None => default_data_dir()?,
    };
    let socket = args.socket.unwrap_or_else(|| data_dir.join("moord.sock"));
    let daemon = Daemon::open(
        &DataDir::new(&data_dir),
        BuildInfo {
            name: "moord".into(),
            version: env!("CARGO_PKG_VERSION").into(),
        },
    )
    .with_context(|| format!("opening data dir {}", data_dir.display()))?;

    if args.stdio {
        serve_stdio(daemon).await?;
        return Ok(());
    }

    let server =
        UnixServer::bind(&socket).with_context(|| format!("binding {}", socket.display()))?;
    tracing::info!(socket = %socket.display(), data_dir = %data_dir.display(), "listening");
    let shutdown = CancellationToken::new();
    let signal = shutdown.clone();
    tokio::spawn(async move {
        let _ = tokio::signal::ctrl_c().await;
        tracing::info!("shutting down");
        signal.cancel();
    });
    server.run(Arc::clone(&daemon), shutdown).await;
    Ok(())
}
