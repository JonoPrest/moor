//! `nits-mcp`: MCP stdio server proxying to a `nitsd` daemon.

use std::path::PathBuf;

use clap::Parser;
use nits_config::Context;
use nits_mcp::server::AgentIdentity;
use nits_mcp::{Endpoint, Server};
use nits_protocol::BuildInfo;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

#[derive(Debug, Parser)]
#[command(name = "nits-mcp", version, about)]
struct Args {
    /// Named context from the config file (see `nits context`). Default:
    /// `local`, an implicit daemon on this machine.
    #[arg(long, short = 'c', env = "NITS_CONTEXT")]
    context: Option<String>,
    /// Config file. Default: `$XDG_CONFIG_HOME/nits/config.toml`.
    #[arg(long, env = "NITS_CONFIG")]
    config: Option<PathBuf>,
    /// Ad-hoc local context: this daemon socket. Overrides `--context`.
    #[arg(long, env = "NITS_SOCKET")]
    socket: Option<PathBuf>,
    /// Ad-hoc context: a daemon WebSocket URL (`ws://host:port`).
    #[arg(long, env = "NITS_WS_URL")]
    ws: Option<String>,
    /// Ad-hoc local context: data dir (socket at `<data-dir>/nitsd.sock`).
    #[arg(long, env = "NITS_DATA_DIR")]
    data_dir: Option<PathBuf>,
    /// Fail instead of starting the daemon when it is not running.
    #[arg(long)]
    no_autostart: bool,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .with_writer(std::io::stderr)
        .init();
    let args = Args::parse();
    let context = if let Some(url) = args.ws {
        Context::Ws { url }
    } else if args.socket.is_some() || args.data_dir.is_some() {
        Context::Local {
            data_dir: args.data_dir,
            socket: args.socket,
        }
    } else {
        let path = match args.config {
            Some(p) => p,
            None => nits_config::Config::default_path()?,
        };
        nits_config::Config::load(&path)?
            .resolve(args.context.as_deref())?
            .1
    };
    let mut server = Server::new(
        Endpoint {
            context,
            autostart: !args.no_autostart,
        },
        AgentIdentity::from_env(),
        BuildInfo {
            name: "nits-mcp".into(),
            version: env!("CARGO_PKG_VERSION").into(),
        },
    );
    let mut lines = BufReader::new(tokio::io::stdin()).lines();
    let mut out = tokio::io::stdout();
    while let Some(line) = lines.next_line().await? {
        if line.trim().is_empty() {
            continue;
        }
        if let Some(reply) = server.handle_line(&line).await {
            let mut bytes = serde_json::to_vec(&reply)?;
            bytes.push(b'\n');
            out.write_all(&bytes).await?;
            out.flush().await?;
        }
    }
    Ok(())
}
