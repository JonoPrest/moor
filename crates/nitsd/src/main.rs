//! `nitsd` binary. See `nitsd --help`.

use std::path::PathBuf;
use std::sync::Arc;

use anyhow::Context;
use clap::Parser;
use nits_protocol::BuildInfo;
use nits_review_core::DataDir;
use nitsd::Daemon;
use nitsd::launch::{DaemonSpec, ensure_daemon, proxy_stdio};
use nitsd::server::{UnixServer, WsServer};

/// The Nits daemon.
#[derive(Debug, Parser)]
#[command(version)]
struct Args {
    /// Where state lives. Default: `$XDG_DATA_HOME/nits` or `~/.local/share/nits`.
    #[arg(long, env = "NITS_DATA_DIR")]
    data_dir: Option<PathBuf>,
    /// Socket to listen on. Default: `<data-dir>/nitsd.sock`.
    #[arg(long, env = "NITS_SOCKET")]
    socket: Option<PathBuf>,
    /// Proxy stdin/stdout to the daemon on `--socket`, starting it if
    /// needed (`ssh host nitsd --stdio`).
    #[arg(long)]
    stdio: bool,
    /// Like `--stdio` but exit with status 3 instead of starting a daemon
    /// when none is running. Lets a client probe or stop a remote daemon.
    #[arg(long, conflicts_with = "stdio")]
    stdio_if_running: bool,
    /// Exit after this many seconds with no client connected.
    #[arg(long, env = "NITS_IDLE_EXIT")]
    idle_exit: Option<u64>,
    /// Also listen for WebSocket clients on this address, e.g.
    /// `127.0.0.1:7677`. Off unless given.
    #[arg(long, env = "NITS_WS")]
    ws: Option<std::net::SocketAddr>,
}

fn default_data_dir() -> anyhow::Result<PathBuf> {
    if let Some(x) = std::env::var_os("XDG_DATA_HOME") {
        return Ok(PathBuf::from(x).join("nits"));
    }
    let home = std::env::var_os("HOME").context("neither XDG_DATA_HOME nor HOME is set")?;
    Ok(PathBuf::from(home).join(".local/share/nits"))
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
    let socket = args.socket.unwrap_or_else(|| data_dir.join("nitsd.sock"));

    if args.stdio || args.stdio_if_running {
        // Proxy to the one daemon on this machine, starting it if needed.
        // An auto-started daemon retires itself after half an hour idle.
        let spec = DaemonSpec {
            idle_exit: Some(args.idle_exit.unwrap_or(1800)),
            ws: args.ws,
            socket,
            ..DaemonSpec::for_data_dir(data_dir.clone())
        };
        if args.stdio {
            ensure_daemon(&spec)
                .await
                .with_context(|| "starting the daemon")?;
        } else if !nitsd::launch::is_listening(&spec.socket).await {
            std::process::exit(3);
        }
        proxy_stdio(&spec.socket).await?;
        return Ok(());
    }

    let daemon = Daemon::open(
        &DataDir::new(&data_dir),
        BuildInfo {
            name: "nitsd".into(),
            version: env!("CARGO_PKG_VERSION").into(),
        },
    )
    .with_context(|| format!("opening data dir {}", data_dir.display()))?;

    let server =
        UnixServer::bind(&socket).with_context(|| format!("binding {}", socket.display()))?;
    tracing::info!(socket = %socket.display(), data_dir = %data_dir.display(), "listening");
    let watcher = nitsd::watcher::Watcher::start(Arc::clone(&daemon));
    let shutdown = daemon.shutdown().clone();
    let signal = shutdown.clone();
    tokio::spawn(async move {
        let _ = tokio::signal::ctrl_c().await;
        tracing::info!("shutting down");
        signal.cancel();
    });
    if let Some(idle) = args.idle_exit {
        let idle = std::time::Duration::from_secs(idle);
        let d = Arc::clone(&daemon);
        let token = shutdown.clone();
        tokio::spawn(async move {
            let mut quiet_since = tokio::time::Instant::now();
            loop {
                tokio::time::sleep(std::time::Duration::from_secs(1)).await;
                if d.connections() > 0 {
                    quiet_since = tokio::time::Instant::now();
                } else if quiet_since.elapsed() >= idle {
                    tracing::info!(idle_secs = idle.as_secs(), "idle; exiting");
                    token.cancel();
                    return;
                }
            }
        });
    }
    let ws = match args.ws {
        Some(addr) => {
            let ws = WsServer::bind(addr)
                .await
                .with_context(|| format!("binding ws {addr}"))?;
            tracing::info!(ws = %ws.addr(), "listening");
            Some(tokio::spawn(ws.run(Arc::clone(&daemon), shutdown.clone())))
        }
        None => None,
    };
    server.run(Arc::clone(&daemon), shutdown).await;
    if let Some(ws) = ws {
        let _ = ws.await;
    }
    watcher.stop();
    Ok(())
}
