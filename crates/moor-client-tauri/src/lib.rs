//! Tauri wrapper around `moor-client-host` (PLAN 4.3, ARCHITECTURE §6.2).
//!
//! The webview calls three commands — `dispatch {action}`, `key {chord}`,
//! `attach {}` — which forward to [`moor_client_host::Handle`]; a task
//! drains the host's patch receiver into `app.emit("view", patches)`.
//! Nothing here knows about the review model: the host owns the core.

use std::path::{Path, PathBuf};

use moor_client_core::{Action, IdSeed, KeyChord, ViewPatch};
use moor_client_host::{Handle, HostConfig, Identity, KvConfig, local_config};
use moor_config::{Config, Context};
use moor_protocol::{Author, BuildInfo, ClientId};
use tauri::{AppHandle, Emitter, Manager, State};
use tokio_util::sync::CancellationToken;

/// The event the UI listens on; payload is `Vec<ViewPatch>`.
pub const VIEW_EVENT: &str = "view";

#[derive(Debug, thiserror::Error)]
pub enum SetupError {
    #[error("config: {0}")]
    Config(#[from] moor_config::ConfigError),
    #[error("{what}: {source}")]
    Io {
        what: &'static str,
        #[source]
        source: std::io::Error,
    },
    #[error("host: {0}")]
    Host(#[from] moor_client_host::HostError),
    #[error(
        "the `{0}` context is not a local daemon; the desktop app only speaks unix sockets yet"
    )]
    NotLocal(String),
    #[error("host task exited during setup")]
    HostGone,
}

/// Shared with every command: the host handle.
#[derive(Debug)]
pub struct Host {
    handle: Handle,
}

/// Error reported to the webview when the host task has ended.
#[derive(Debug, Clone, Copy, serde::Serialize)]
pub struct HostGone;

impl std::fmt::Display for HostGone {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("host task has exited")
    }
}

// `tauri::command` requires `State` by value.
#[allow(clippy::needless_pass_by_value)]
#[tauri::command]
fn dispatch(host: State<'_, Host>, action: Action) -> Result<(), HostGone> {
    host.handle.dispatch(action).then_some(()).ok_or(HostGone)
}

#[allow(clippy::needless_pass_by_value)]
#[tauri::command]
fn key(host: State<'_, Host>, chord: KeyChord) -> Result<(), HostGone> {
    host.handle.key(chord).then_some(()).ok_or(HostGone)
}

/// The webview reports adapter failures here so they reach the Rust log.
#[allow(clippy::needless_pass_by_value)]
#[tauri::command]
fn client_error(message: String) {
    tracing::warn!(%message, "webview");
}

#[allow(clippy::needless_pass_by_value)]
#[tauri::command]
fn attach(host: State<'_, Host>) -> Result<(), HostGone> {
    host.handle.attach().then_some(()).ok_or(HostGone)
}

/// Who this desktop client is: `$USER@hostname`, a fresh client id.
#[must_use]
pub fn identity() -> Identity {
    let machine = gethostname::gethostname().to_string_lossy().into_owned();
    let name = std::env::var("USER").unwrap_or_else(|_| "anonymous".into());
    let (ts, r) = moord::ids::fresh_parts();
    Identity {
        client_id: ClientId::from_parts(ts, r),
        client: BuildInfo {
            name: "moor-desktop".into(),
            version: env!("CARGO_PKG_VERSION").into(),
        },
        author: Author::Human { name, machine },
    }
}

/// Resolve the daemon socket for `context` (default context when `None`),
/// starting a local daemon if needed. Remote contexts are not wired yet
/// (PLAN 4.6).
pub async fn socket_for(context: Option<&str>) -> Result<PathBuf, SetupError> {
    let cfg = Config::load(&Config::default_path()?)?;
    let (name, ctx) = cfg.resolve(context)?;
    match ctx {
        Context::Local { data_dir, socket } => {
            let spec = moord::contexts::local_spec(data_dir.as_ref(), socket.as_ref()).map_err(
                |source| SetupError::Io {
                    what: "data dir",
                    source,
                },
            )?;
            moord::launch::ensure_daemon(&spec)
                .await
                .map_err(|source| SetupError::Io {
                    what: "starting the daemon",
                    source,
                })?;
            Ok(spec.socket)
        }
        Context::Ssh { .. } | Context::Ws { .. } => Err(SetupError::NotLocal(name)),
    }
}

/// Host config for the desktop: redb KV under `app_data_dir`, random seed.
#[must_use]
pub fn host_config(socket: &Path, app_data_dir: &Path) -> HostConfig {
    local_config(
        socket,
        identity(),
        IdSeed(fastrand::u128(..)),
        KvConfig::Redb(app_data_dir.join("kv.redb")),
    )
}

/// Start the host and forward its patches to the webview. Returns the
/// state the commands read.
pub fn start_host(app: &AppHandle, config: HostConfig) -> Result<Host, SetupError> {
    if let KvConfig::Redb(p) = &config.kv
        && let Some(dir) = p.parent()
    {
        std::fs::create_dir_all(dir).map_err(|source| SetupError::Io {
            what: "app data dir",
            source,
        })?;
    }
    let shutdown = CancellationToken::new();
    // `spawn` calls `tokio::spawn`; `setup` runs on the main thread, outside
    // Tauri's runtime, so enter it explicitly.
    let (handle, mut patches) = {
        let rt = tauri::async_runtime::handle();
        let _guard = rt.inner().enter();
        moor_client_host::spawn(config, shutdown.clone())?
    };
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        while let Some(batch) = patches.recv().await {
            tracing::debug!(patches = ?batch, "view");
            if let Err(e) = app.emit(VIEW_EVENT, &batch) {
                tracing::warn!(error = %e, "emit view patches");
            }
        }
        shutdown.cancel();
    });
    // The core only dials when asked (`Action::Connect`); the desktop
    // always wants to be connected.
    if !handle.dispatch(Action::Connect) {
        return Err(SetupError::HostGone);
    }
    Ok(Host { handle })
}

/// Build and run the app. `context` picks a `moor-config` context.
pub fn run(context: Option<String>) -> Result<(), Box<dyn std::error::Error>> {
    tauri::Builder::default()
        .setup(move |app| {
            let data_dir = app.path().app_data_dir()?;
            let socket = tauri::async_runtime::block_on(socket_for(context.as_deref()))?;
            let host = start_host(app.handle(), host_config(&socket, &data_dir))?;
            app.manage(host);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            dispatch,
            key,
            attach,
            client_error
        ])
        .run(tauri::generate_context!())?;
    Ok(())
}

/// Patches are the same type the host emits; kept public so a test can
/// assert the event payload shape.
pub type Patches = Vec<ViewPatch>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identity_is_a_human_on_this_machine() {
        let id = identity();
        assert_eq!(id.client.name, "moor-desktop");
        assert!(matches!(id.author, Author::Human { .. }));
    }

    #[test]
    fn host_config_puts_kv_under_app_data_dir() {
        let cfg = host_config(Path::new("/tmp/moord.sock"), Path::new("/data"));
        assert_eq!(cfg.socket, Path::new("/tmp/moord.sock"));
        assert!(matches!(&cfg.kv, KvConfig::Redb(p) if p == Path::new("/data/kv.redb")));
    }

    /// The `view` payload is exactly the array `CoreTauri.res` parses.
    #[test]
    fn view_payload_is_a_patch_array() {
        let patches: Patches = Vec::new();
        assert_eq!(serde_json::to_string(&patches).unwrap(), "[]");
    }
}
