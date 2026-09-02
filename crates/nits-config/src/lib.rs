//! Client configuration shared by the CLI, MCP shim and desktop app: a set
//! of named **contexts** (where a daemon is and how to reach it). Stored as
//! TOML at `$XDG_CONFIG_HOME/nits/config.toml` (default
//! `~/.config/nits/config.toml`).
//!
//! Deliberately **no "current" context**: the file holds definitions only.
//! Each process picks its context explicitly (`--context`, `NITS_CONTEXT`)
//! or gets the implicit [`DEFAULT_CONTEXT`], so a CLI or MCP session for one
//! project can never redirect another, and the desktop app keeps its own
//! selection in its own state.
//!
//! ```toml
//! [contexts.laptop]
//! type = "Local"
//!
//! [contexts.build-box]
//! type = "Ssh"
//! host = "build-box"
//!
//! [contexts.shared]
//! type = "Ws"
//! url = "ws://reviews.internal:7677"
//! ```
//!
//! Daemon lifecycle per kind: `Local` and `Ssh` contexts start the daemon on
//! demand (locally, or via `ssh host nits daemon stdio` which does it remotely);
//! a `Ws` context is somebody else's daemon and is only connected to.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

/// Name of the context used when none is configured.
pub const DEFAULT_CONTEXT: &str = "local";

#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("parse {path}: {source}")]
    Parse {
        path: PathBuf,
        #[source]
        source: toml::de::Error,
    },
    #[error("serialize: {0}")]
    Serialize(#[from] toml::ser::Error),
    #[error("no context named {0}")]
    NoSuchContext(String),
    #[error("HOME is not set")]
    NoHome,
}

/// Where a daemon is and how to reach it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", deny_unknown_fields)]
pub enum Context {
    /// A daemon on this machine, started on demand.
    Local {
        /// Default: `$XDG_DATA_HOME/nits` or `~/.local/share/nits`.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        data_dir: Option<PathBuf>,
        /// Default: `<data_dir>/nitsd.sock`.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        socket: Option<PathBuf>,
    },
    /// A daemon on another machine, reached by `ssh <host> nits daemon
    /// stdio`, which starts it there if needed. Auth, jumps and ports come
    /// from `~/.ssh/config`.
    Ssh {
        host: String,
        /// The remote `nits` binary. Default: `nits` on the remote PATH.
        /// The `nitsd` alias reads configs written when the daemon was a
        /// second executable.
        #[serde(default, alias = "nitsd", skip_serializing_if = "Option::is_none")]
        bin: Option<String>,
        /// Extra arguments for the remote daemon, e.g. `--data-dir`.
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        args: Vec<String>,
        /// The ssh client to run. Default: `ssh`.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        ssh: Option<String>,
    },
    /// A daemon already listening for WebSocket clients.
    Ws { url: String },
}

impl Context {
    /// One-line description for listings.
    #[must_use]
    pub fn describe(&self) -> String {
        match self {
            Context::Local { data_dir, socket } => match (data_dir, socket) {
                (None, None) => "local".into(),
                (d, s) => format!(
                    "local{}{}",
                    d.as_ref()
                        .map(|d| format!(" data_dir={}", d.display()))
                        .unwrap_or_default(),
                    s.as_ref()
                        .map(|s| format!(" socket={}", s.display()))
                        .unwrap_or_default()
                ),
            },
            Context::Ssh { host, .. } => format!("ssh {host}"),
            Context::Ws { url } => format!("ws {url}"),
        }
    }
}

/// The whole config file. Unknown top-level keys are ignored (not denied)
/// so a file written by a newer or older client still loads; contexts
/// themselves stay strict.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct Config {
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub contexts: BTreeMap<String, Context>,
}

impl Config {
    /// `$XDG_CONFIG_HOME/nits/config.toml` or `~/.config/nits/config.toml`.
    pub fn default_path() -> Result<PathBuf, ConfigError> {
        if let Ok(x) = std::env::var("XDG_CONFIG_HOME") {
            return Ok(PathBuf::from(x).join("nits").join("config.toml"));
        }
        let home = std::env::var("HOME").map_err(|_| ConfigError::NoHome)?;
        Ok(PathBuf::from(home).join(".config/nits/config.toml"))
    }

    /// Read `path`; a missing file is an empty config.
    pub fn load(path: &Path) -> Result<Self, ConfigError> {
        match std::fs::read_to_string(path) {
            Ok(text) => toml::from_str(&text).map_err(|source| ConfigError::Parse {
                path: path.to_path_buf(),
                source,
            }),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Self::default()),
            Err(e) => Err(e.into()),
        }
    }

    /// Write `path`, creating parent directories.
    pub fn save(&self, path: &Path) -> Result<(), ConfigError> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(path, toml::to_string_pretty(self)?)?;
        Ok(())
    }

    /// Resolve `name` (or [`DEFAULT_CONTEXT`]). An unconfigured default is
    /// an implicit `Local` with defaults, so a fresh install works with no
    /// config file.
    pub fn resolve(&self, name: Option<&str>) -> Result<(String, Context), ConfigError> {
        let name = name.unwrap_or(DEFAULT_CONTEXT);
        if let Some(c) = self.contexts.get(name) {
            return Ok((name.to_string(), c.clone()));
        }
        if name == DEFAULT_CONTEXT {
            return Ok((
                name.to_string(),
                Context::Local {
                    data_dir: None,
                    socket: None,
                },
            ));
        }
        Err(ConfigError::NoSuchContext(name.to_string()))
    }

    pub fn remove(&mut self, name: &str) -> Result<Context, ConfigError> {
        self.contexts
            .remove(name)
            .ok_or_else(|| ConfigError::NoSuchContext(name.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_and_resolves() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("nits/config.toml");
        let empty = Config::load(&path).unwrap();
        assert_eq!(empty, Config::default());
        let (name, ctx) = empty.resolve(None).unwrap();
        assert_eq!(name, "local");
        assert!(matches!(ctx, Context::Local { .. }));
        assert!(empty.resolve(Some("nope")).is_err());

        let mut cfg = Config::default();
        cfg.contexts.insert(
            "box".into(),
            Context::Ssh {
                host: "build-box".into(),
                bin: None,
                args: vec!["--data-dir".into(), "/srv/nits".into()],
                ssh: None,
            },
        );
        cfg.contexts.insert(
            "shared".into(),
            Context::Ws {
                url: "ws://h:7677".into(),
            },
        );
        cfg.save(&path).unwrap();
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(text.contains("type = \"Ssh\""), "{text}");
        let back = Config::load(&path).unwrap();
        assert_eq!(back, cfg);
        assert_eq!(back.resolve(Some("box")).unwrap().0, "box");
        // No "current": resolving nothing is always the implicit default.
        assert_eq!(back.resolve(None).unwrap().0, "local");

        let mut back = back;
        back.remove("box").unwrap();
        assert!(back.resolve(Some("box")).is_err());
        assert!(Config::load(&path).is_ok());
    }
}
