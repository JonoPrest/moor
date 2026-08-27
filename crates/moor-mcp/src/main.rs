//! `moor-mcp`: MCP stdio server proxying to a `moord` daemon.

use std::path::PathBuf;

use clap::Parser;
use moor_mcp::server::AgentIdentity;
use moor_mcp::{Endpoint, Server};
use moor_protocol::BuildInfo;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

#[derive(Debug, Parser)]
#[command(name = "moor-mcp", version, about)]
struct Args {
    /// Daemon unix socket. Default: `<data-dir>/moord.sock`.
    #[arg(long, env = "MOOR_SOCKET")]
    socket: Option<PathBuf>,
    /// Daemon WebSocket URL (`ws://host:port`); overrides `--socket`.
    #[arg(long, env = "MOOR_WS_URL")]
    ws: Option<String>,
    /// Where state lives, used only to find the default socket.
    #[arg(long, env = "MOOR_DATA_DIR")]
    data_dir: Option<PathBuf>,
}

fn default_data_dir() -> anyhow::Result<PathBuf> {
    if let Ok(x) = std::env::var("XDG_DATA_HOME") {
        return Ok(PathBuf::from(x).join("moor"));
    }
    let home = std::env::var("HOME").map_err(|_| anyhow::anyhow!("HOME is not set"))?;
    Ok(PathBuf::from(home).join(".local/share/moor"))
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .with_writer(std::io::stderr)
        .init();
    let args = Args::parse();
    let endpoint = match args.ws {
        Some(url) => Endpoint::Ws(url),
        None => Endpoint::Unix(match args.socket {
            Some(s) => s,
            None => args
                .data_dir
                .map_or_else(default_data_dir, Ok)?
                .join("moord.sock"),
        }),
    };
    let mut server = Server::new(
        endpoint,
        AgentIdentity::from_env(),
        BuildInfo {
            name: "moor-mcp".into(),
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
