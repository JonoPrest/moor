//! `~/.config/moor/keys.toml` → keymap overrides (UI-DESIGN §bindings).
//!
//! One table per context (case-insensitive: `[global]`, `[diff]`, …), one
//! entry per command. The value is the key sequence to bind (`""` unbinds),
//! or a table `{ keys = "…", primary = true }` to also put it in the hint
//! bar. Commands and contexts are enums and chords are parsed: an unknown
//! name or an unparsable chord fails loudly at load — the file is rejected
//! whole, never half-applied.
//!
//! ```toml
//! [global]
//! ToggleLayout = "g x"          # rebind
//! ToggleWhitespace = ""         # unbind
//!
//! [diff]
//! NextHunk = { keys = "]", primary = true }
//! ```

use std::path::PathBuf;
use std::str::FromStr;

use moor_client_core::{Command, Context, KeySeq, Override, Overrides};
use strum::IntoEnumIterator;

/// Why the keys file was rejected. Everything names the offending entry.
#[derive(Debug, thiserror::Error)]
pub enum KeysFileError {
    #[error("keys.toml is not valid TOML: {0}")]
    Toml(#[from] toml::de::Error),
    #[error("[{name}] is not a context; expected one of {names}", name = .0, names = context_names())]
    UnknownContext(String),
    #[error("[{context}] must be a table of command = keys entries")]
    NotATable { context: String },
    #[error("[{context}] {0:?} is not a command", .command)]
    UnknownCommand { context: String, command: String },
    #[error("[{context}] {command}: {source}")]
    BadKeys {
        context: String,
        command: String,
        source: moor_client_core::KeyParseError,
    },
    #[error("[{context}] {command}: expected a key string or {{ keys, primary }}")]
    BadValue { context: String, command: String },
}

fn context_names() -> String {
    Context::iter()
        .map(|c| c.to_string().to_ascii_lowercase())
        .collect::<Vec<_>>()
        .join(", ")
}

/// `$XDG_CONFIG_HOME/moor/keys.toml` or `~/.config/moor/keys.toml`.
#[must_use]
pub fn default_keys_path() -> Option<PathBuf> {
    if let Ok(x) = std::env::var("XDG_CONFIG_HOME") {
        return Some(PathBuf::from(x).join("moor").join("keys.toml"));
    }
    std::env::var("HOME")
        .ok()
        .map(|home| PathBuf::from(home).join(".config/moor/keys.toml"))
}

fn context_of(name: &str) -> Option<Context> {
    Context::iter().find(|c| c.to_string().eq_ignore_ascii_case(name))
}

fn command_of(name: &str) -> Option<Command> {
    Command::iter().find(|c| c.to_string().eq_ignore_ascii_case(name))
}

/// Parse the whole file into the override list the core consumes.
pub fn parse(text: &str) -> Result<Overrides, KeysFileError> {
    let value: toml::Value = toml::from_str(text)?;
    let Some(table) = value.as_table() else {
        return Ok(Overrides::default());
    };
    let mut bindings = Vec::new();
    for (context_name, entries) in table {
        let context = context_of(context_name)
            .ok_or_else(|| KeysFileError::UnknownContext(context_name.clone()))?;
        let entries = entries.as_table().ok_or_else(|| KeysFileError::NotATable {
            context: context_name.clone(),
        })?;
        for (command_name, spec) in entries {
            let command =
                command_of(command_name).ok_or_else(|| KeysFileError::UnknownCommand {
                    context: context_name.clone(),
                    command: command_name.clone(),
                })?;
            let (keys_text, primary) = match spec {
                toml::Value::String(s) => (s.clone(), false),
                toml::Value::Table(t) => {
                    let keys = t.get("keys").and_then(|k| k.as_str());
                    let primary = t.get("primary").and_then(toml::Value::as_bool);
                    let known = t.keys().all(|k| k == "keys" || k == "primary");
                    match (keys, known) {
                        (Some(k), true) => (k.to_owned(), primary.unwrap_or(false)),
                        (None | Some(_), _) => {
                            return Err(KeysFileError::BadValue {
                                context: context_name.clone(),
                                command: command_name.clone(),
                            });
                        }
                    }
                }
                toml::Value::Integer(_)
                | toml::Value::Float(_)
                | toml::Value::Boolean(_)
                | toml::Value::Datetime(_)
                | toml::Value::Array(_) => {
                    return Err(KeysFileError::BadValue {
                        context: context_name.clone(),
                        command: command_name.clone(),
                    });
                }
            };
            let keys = if keys_text.is_empty() {
                None
            } else {
                Some(
                    KeySeq::from_str(&keys_text).map_err(|source| KeysFileError::BadKeys {
                        context: context_name.clone(),
                        command: command_name.clone(),
                        source,
                    })?,
                )
            };
            bindings.push(Override {
                context,
                command,
                keys,
                primary,
            });
        }
    }
    Ok(Overrides { bindings })
}

#[cfg(test)]
mod tests {
    use super::*;
    use moor_client_core::Keymap;

    #[test]
    fn parses_rebinding_unbinding_and_primary() {
        let overrides = parse(
            r#"
            [global]
            ToggleLayout = "g x"
            ToggleWhitespace = ""

            [diff]
            NextHunk = { keys = "]", primary = true }
            "#,
        )
        .unwrap();
        assert_eq!(overrides.bindings.len(), 3);
        let map = Keymap::with_overrides(&overrides);
        let rebound: Vec<_> = map
            .bindings()
            .iter()
            .filter(|b| b.command == Command::ToggleLayout && b.context == Context::Global)
            .collect();
        assert_eq!(rebound.len(), 1);
        assert_eq!(rebound[0].keys.to_string(), "g x");
        assert!(
            !map.bindings()
                .iter()
                .any(|b| b.command == Command::ToggleWhitespace && b.context == Context::Global),
            "empty keys unbind"
        );
        let hunk = map
            .bindings()
            .iter()
            .find(|b| b.command == Command::NextHunk && b.context == Context::Diff)
            .unwrap();
        assert_eq!(hunk.keys.to_string(), "]");
        assert!(hunk.primary);
    }

    #[test]
    fn rejects_unknown_names_and_bad_chords_loudly() {
        let err = parse("[nowhere]\nOpen = \"o\"\n").unwrap_err();
        assert!(matches!(err, KeysFileError::UnknownContext(c) if c == "nowhere"));
        let err = parse("[diff]\nFrobnicate = \"o\"\n").unwrap_err();
        assert!(
            matches!(err, KeysFileError::UnknownCommand { command, .. } if command == "Frobnicate")
        );
        let err = parse("[diff]\nOpen = \"ctrl+bogus+x\"\n").unwrap_err();
        assert!(matches!(err, KeysFileError::BadKeys { .. }));
        let err = parse("[diff]\nOpen = 3\n").unwrap_err();
        assert!(matches!(err, KeysFileError::BadValue { .. }));
        let err = parse("[diff]\nOpen = { keys = \"o\", typo = true }\n").unwrap_err();
        assert!(matches!(err, KeysFileError::BadValue { .. }));
        assert!(
            parse("[[diff]]\n").is_err(),
            "arrays of tables are not contexts"
        );
    }

    #[test]
    fn context_and_command_names_are_case_insensitive() {
        let overrides = parse("[REVIEWLIST]\nrefresh = \"F5\"\n");
        // F5 is not a chord we parse — the error names the entry.
        assert!(matches!(
            overrides.unwrap_err(),
            KeysFileError::BadKeys { command, .. } if command == "refresh"
        ));
        let overrides = parse("[reviewlist]\nrefresh = \"r\"\n").unwrap();
        assert_eq!(overrides.bindings[0].context, Context::ReviewList);
        assert_eq!(overrides.bindings[0].command, Command::Refresh);
    }
}
