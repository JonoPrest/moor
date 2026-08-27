//! Client configuration shared by the CLI, MCP shim and desktop app: a set
//! of named **contexts** (where a daemon is and how to reach it) and which
//! one is current, kubectl-style. Stored as TOML at
//! `$XDG_CONFIG_HOME/moor/config.toml` (default `~/.config/moor/config.toml`).
//!
//! ```toml
//! current = "laptop"
//!
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
//! demand (locally, or via `ssh host moord --stdio` which does it remotely);
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
        /// Default: `$XDG_DATA_HOME/moor` or `~/.local/share/moor`.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        data_dir: Option<PathBuf>,
        /// Default: `<data_dir>/moord.sock`.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        socket: Option<PathBuf>,
    },
    /// A daemon on another machine, reached by `ssh <host> moord --stdio`,
    /// which starts it there if needed. Auth, jumps and ports come from
    /// `~/.ssh/config`.
    Ssh {
        host: String,
        /// The remote `moord` binary. Default: `moord` on the remote PATH.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        moord: Option<String>,
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

/// The whole config file.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    /// Name of the context used when none is given. Absent means
    /// [`DEFAULT_CONTEXT`], an implicit local context.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub current: Option<String>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub contexts: BTreeMap<String, Context>,
}

impl Config {
    /// `$XDG_CONFIG_HOME/moor/config.toml` or `~/.config/moor/config.toml`.
    pub fn default_path() -> Result<PathBuf, ConfigError> {
        if let Ok(x) = std::env::var("XDG_CONFIG_HOME") {
            return Ok(PathBuf::from(x).join("moor").join("config.toml"));
        }
        let home = std::env::var("HOME").map_err(|_| ConfigError::NoHome)?;
        Ok(PathBuf::from(home).join(".config/moor/config.toml"))
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

    /// The current context's name.
    #[must_use]
    pub fn current_name(&self) -> &str {
        self.current.as_deref().unwrap_or(DEFAULT_CONTEXT)
    }

    /// Resolve `name` (or the current one). An unconfigured
    /// [`DEFAULT_CONTEXT`] is an implicit `Local` with defaults, so a fresh
    /// install works with no config file.
    pub fn resolve(&self, name: Option<&str>) -> Result<(String, Context), ConfigError> {
        let name = name.unwrap_or_else(|| self.current_name());
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

    /// Make `name` current; it must exist (or be the implicit default).
    pub fn use_context(&mut self, name: &str) -> Result<(), ConfigError> {
        self.resolve(Some(name))?;
        self.current = Some(name.to_string());
        Ok(())
    }

    pub fn remove(&mut self, name: &str) -> Result<Context, ConfigError> {
        let c = self
            .contexts
            .remove(name)
            .ok_or_else(|| ConfigError::NoSuchContext(name.to_string()))?;
        if self.current.as_deref() == Some(name) {
            self.current = None;
        }
        Ok(c)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_and_resolves() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("moor/config.toml");
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
                moord: None,
                args: vec!["--data-dir".into(), "/srv/moor".into()],
                ssh: None,
            },
        );
        cfg.contexts.insert(
            "shared".into(),
            Context::Ws {
                url: "ws://h:7677".into(),
            },
        );
        cfg.use_context("box").unwrap();
        cfg.save(&path).unwrap();
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(text.contains("current = \"box\""), "{text}");
        assert!(text.contains("type = \"Ssh\""), "{text}");
        let back = Config::load(&path).unwrap();
        assert_eq!(back, cfg);
        assert_eq!(back.resolve(None).unwrap().0, "box");

        let mut back = back;
        back.remove("box").unwrap();
        assert_eq!(back.current, None);
        assert_eq!(back.resolve(None).unwrap().0, "local");
    }
}
