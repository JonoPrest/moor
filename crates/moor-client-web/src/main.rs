//! `moor-web [context] [--port 9777]`: WebSocket bridge for the browser
//! UI. Run `pnpm --dir ui dev`, open the Vite URL, and the page connects
//! to this bridge (default `ws://127.0.0.1:9777`, override with `?ws=`).

use std::net::{Ipv4Addr, SocketAddr};
use std::path::Path;

use clap::Parser;
use moor_client_core::IdSeed;
use moor_client_host::{Identity, KvConfig, local_config};
use moor_config::{Config, Context};
use moor_protocol_shim::{Author, BuildInfo, ClientId};

// The wire types come through moor-client-core's re-export so the bin
// needs no direct moor-protocol dependency.
mod moor_protocol_shim {
    pub use moor_client_core::protocol::{Author, BuildInfo, ClientId};
}

#[derive(Debug, Parser)]
#[command(about = "WebSocket bridge around moor-client-host for the browser UI")]
struct Args {
    /// Named context from the config file. Default: `local`.
    context: Option<String>,
    /// Port to listen on (loopback only).
    #[arg(long, default_value_t = 9777)]
    port: u16,
}

fn identity() -> Identity {
    let machine = gethostname::gethostname().to_string_lossy().into_owned();
    let name = std::env::var("USER").unwrap_or_else(|_| "anonymous".into());
    let (ts, r) = moord::ids::fresh_parts();
    Identity {
        client_id: ClientId::from_parts(ts, r),
        client: BuildInfo {
            name: "moor-web".into(),
            version: env!("CARGO_PKG_VERSION").into(),
        },
        author: Author::Human { name, machine },
    }
}

#[tokio::main(flavor = "multi_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();
    let args = Args::parse();
    let cfg = Config::load(&Config::default_path()?)?;
    let (name, ctx) = cfg.resolve(args.context.as_deref())?;
    let socket = match ctx {
        Context::Local { data_dir, socket } => {
            let spec = moord::contexts::local_spec(data_dir.as_ref(), socket.as_ref())?;
            moord::launch::ensure_daemon(&spec).await?;
            spec.socket
        }
        Context::Ssh { .. } | Context::Ws { .. } => {
            return Err(format!(
                "context `{name}` is not local; moor-web only speaks unix sockets yet"
            )
            .into());
        }
    };
    // Dev tool: memory KV is enough (prefs reset per run).
    let config = local_config(
        &socket,
        identity(),
        IdSeed(fastrand::u128(..)),
        KvConfig::Memory,
    );
    let addr = SocketAddr::from((Ipv4Addr::LOCALHOST, args.port));
    let server = moor_client_web::serve(addr, config).await?;
    let _ = Path::new("/");
    eprintln!(
        "moor-web: ws://{} (daemon {})",
        server.addr(),
        socket.display()
    );
    tokio::signal::ctrl_c().await?;
    server.stop();
    Ok(())
}
