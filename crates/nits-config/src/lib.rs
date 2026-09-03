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

/// Which binary an ssh context runs on the remote, parsed from the two
/// wire spellings once, here. The rest of the code never sees "both keys"
/// or has to decide which wins: the states a config can express are the
/// variants, and nothing else.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum RemoteBin {
    /// No key: `nits` from the remote PATH.
    #[default]
    Default,
    /// `bin = "..."`: where `nits` is on that host.
    Nits(String),
    /// `nitsd = "..."`, written before the daemon became `nits daemon
    /// serve`. Its own variant because it must never be *run* — the value
    /// names an executable that does not understand `daemon stdio` — only
    /// refused, with the edit to make.
    Legacy(String),
}

impl RemoteBin {
    /// The remote `nits` to run, for a context that may be run at all.
    #[must_use]
    pub fn nits(&self) -> Option<&str> {
        match self {
            RemoteBin::Default => Some("nits"),
            RemoteBin::Nits(bin) => Some(bin),
            RemoteBin::Legacy(_) => None,
        }
    }
}

/// Where a daemon is and how to reach it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "ContextWire", into = "ContextWire")]
pub enum Context {
    /// A daemon on this machine, started on demand.
    Local {
        /// Default: `$XDG_DATA_HOME/nits` or `~/.local/share/nits`.
        data_dir: Option<PathBuf>,
        /// Default: `<data_dir>/nitsd.sock`.
        socket: Option<PathBuf>,
    },
    /// A daemon on another machine, reached by `ssh <host> nits daemon
    /// stdio`, which starts it there if needed. Auth, jumps and ports come
    /// from `~/.ssh/config`.
    Ssh {
        host: String,
        bin: RemoteBin,
        /// Extra arguments for the remote daemon, e.g. `--data-dir`.
        args: Vec<String>,
        /// The ssh client to run. Default: `ssh`.
        ssh: Option<String>,
    },
    /// A daemon already listening for WebSocket clients.
    Ws { url: String },
}

/// The file's spelling of a context. Only [`Context`] is used past the
/// boundary; this exists so the two ways of naming a remote binary are
/// turned into one domain value — or rejected — exactly once.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", deny_unknown_fields)]
enum ContextWire {
    Local {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        data_dir: Option<PathBuf>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        socket: Option<PathBuf>,
    },
    Ssh {
        host: String,
        /// The remote `nits` binary. Default: `nits` on the remote PATH.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        bin: Option<String>,
        /// The pre-one-binary key, recognised so a config written before
        /// the daemon became `nits daemon serve` can be reported rather
        /// than silently mis-run.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        nitsd: Option<String>,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        args: Vec<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        ssh: Option<String>,
    },
    Ws {
        url: String,
    },
}

/// Why a context in the file is not a context.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ContextParseError {
    #[error(
        "context for {host} sets both `bin` and `nitsd`; {}",
        Context::LEGACY_NITSD_HELP
    )]
    BothBinAndNitsd { host: String },
}

impl TryFrom<ContextWire> for Context {
    type Error = ContextParseError;

    fn try_from(w: ContextWire) -> Result<Self, Self::Error> {
        Ok(match w {
            ContextWire::Local { data_dir, socket } => Context::Local { data_dir, socket },
            ContextWire::Ssh {
                host,
                bin,
                nitsd,
                args,
                ssh,
            } => {
                let bin = match (bin, nitsd) {
                    (Some(_), Some(_)) => {
                        return Err(ContextParseError::BothBinAndNitsd { host });
                    }
                    (Some(bin), None) => RemoteBin::Nits(bin),
                    (None, Some(nitsd)) => RemoteBin::Legacy(nitsd),
                    (None, None) => RemoteBin::Default,
                };
                Context::Ssh {
                    host,
                    bin,
                    args,
                    ssh,
                }
            }
            ContextWire::Ws { url } => Context::Ws { url },
        })
    }
}

impl From<Context> for ContextWire {
    fn from(c: Context) -> Self {
        match c {
            Context::Local { data_dir, socket } => ContextWire::Local { data_dir, socket },
            Context::Ssh {
                host,
                bin,
                args,
                ssh,
            } => {
                let (bin, nitsd) = match bin {
                    RemoteBin::Default => (None, None),
                    RemoteBin::Nits(bin) => (Some(bin), None),
                    RemoteBin::Legacy(nitsd) => (None, Some(nitsd)),
                };
                ContextWire::Ssh {
                    host,
                    bin,
                    nitsd,
                    args,
                    ssh,
                }
            }
            Context::Ws { url } => ContextWire::Ws { url },
        }
    }
}

impl Context {
    /// What to tell someone whose config still has the old key.
    pub const LEGACY_NITSD_HELP: &'static str = "the daemon is now `nits daemon serve`. Replace `nitsd = \"...\"` with \
         `bin = \"nits\"` (or the path to `nits` on that host), or drop the line \
         to use `nits` from the remote PATH";

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
                bin: RemoteBin::Default,
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

    /// A config written before the daemon became `nits daemon serve` still
    /// loads — it has to, or the CLI could not even tell the user what is
    /// wrong — but the old `nitsd` key stays distinguishable from `bin` so
    /// callers refuse it instead of running a binary that cannot serve.
    #[test]
    fn the_pre_one_binary_nitsd_key_is_recognised_not_silently_reused() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        std::fs::write(
            &path,
            "[contexts.box]\ntype = \"Ssh\"\nhost = \"build-box\"\nnitsd = \"/opt/bin/nitsd\"\n",
        )
        .unwrap();

        let cfg = Config::load(&path).expect("an old config still loads");
        let (_, ctx) = cfg.resolve(Some("box")).unwrap();
        // Recognised as the legacy key, and crucially not adopted as the
        // binary to run: `/opt/bin/nitsd daemon stdio` is not a command
        // the old daemon understands. There is no state in which a caller
        // has to decide between two spellings — the parse already did.
        assert!(matches!(
            &ctx,
            Context::Ssh { bin: RemoteBin::Legacy(n), .. } if n == "/opt/bin/nitsd"
        ));
        assert_eq!(
            match &ctx {
                Context::Ssh { bin, .. } => bin.nits(),
                Context::Local { .. } | Context::Ws { .. } => None,
            },
            None,
            "a legacy context names nothing runnable"
        );
        assert!(Context::LEGACY_NITSD_HELP.contains("bin = "));
        // It round-trips back to the old spelling rather than being
        // rewritten into something the old client would not understand.
        let round: Context = toml::from_str(&toml::to_string(&ctx).unwrap()).unwrap();
        assert_eq!(round, ctx);

        // Both keys at once is not a context: the file is refused, with
        // the edit to make, rather than one of them silently winning.
        std::fs::write(
            &path,
            "[contexts.box]\ntype = \"Ssh\"\nhost = \"build-box\"\nbin = \"nits\"\nnitsd = \"/opt/bin/nitsd\"\n",
        )
        .unwrap();
        let err = Config::load(&path).expect_err("both keys is not a context");
        let text = err.to_string();
        assert!(text.contains("bin") && text.contains("nitsd"), "{text}");

        // A migrated config has no legacy key and names `nits`.
        std::fs::write(
            &path,
            "[contexts.box]\ntype = \"Ssh\"\nhost = \"build-box\"\nbin = \"nits\"\n",
        )
        .unwrap();
        let (_, ctx) = Config::load(&path).unwrap().resolve(Some("box")).unwrap();
        assert!(matches!(
            &ctx,
            Context::Ssh { bin: RemoteBin::Nits(b), .. } if b == "nits"
        ));
        // And one with neither key runs `nits` from the remote PATH.
        std::fs::write(
            &path,
            "[contexts.box]\ntype = \"Ssh\"\nhost = \"build-box\"\n",
        )
        .unwrap();
        let (_, ctx) = Config::load(&path).unwrap().resolve(Some("box")).unwrap();
        assert!(matches!(
            &ctx,
            Context::Ssh {
                bin: RemoteBin::Default,
                ..
            }
        ));
        assert_eq!(
            match &ctx {
                Context::Ssh { bin, .. } => bin.nits(),
                Context::Local { .. } | Context::Ws { .. } => None,
            },
            Some("nits")
        );
    }
}
