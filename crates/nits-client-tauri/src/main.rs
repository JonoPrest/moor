#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::path::PathBuf;

use clap::{Parser, ValueEnum};
use nits_client_tauri::{EndpointOptions, EndpointSource};
use nitsd::contexts::StartPolicy;

#[derive(Debug, Parser)]
#[command(name = "nits-desktop", version, about)]
struct Args {
    /// Named context from the config file; the default context when absent.
    #[arg(value_name = "CONTEXT", conflicts_with_all = ["socket", "ws", "data_dir"])]
    context: Option<String>,
    #[arg(
        long,
        env = "NITS_CONFIG",
        conflicts_with_all = ["socket", "ws", "data_dir"]
    )]
    config: Option<PathBuf>,
    #[arg(long, conflicts_with = "ws")]
    socket: Option<PathBuf>,
    #[arg(long, conflicts_with_all = ["socket", "data_dir"])]
    ws: Option<String>,
    #[arg(long, conflicts_with = "ws")]
    data_dir: Option<PathBuf>,
    #[arg(long, value_enum, default_value_t = StartPolicyArg::StartIfNeeded)]
    start_policy: StartPolicyArg,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum StartPolicyArg {
    StartIfNeeded,
    RequireRunning,
}

impl From<StartPolicyArg> for StartPolicy {
    fn from(value: StartPolicyArg) -> Self {
        match value {
            StartPolicyArg::StartIfNeeded => Self::StartIfNeeded,
            StartPolicyArg::RequireRunning => Self::RequireRunning,
        }
    }
}

impl Args {
    fn endpoint_options(self) -> Result<EndpointOptions, &'static str> {
        let source = match (self.ws, self.socket, self.data_dir) {
            (Some(url), None, None) => EndpointSource::WebSocket { url },
            (None, Some(socket), data_dir) => EndpointSource::Local {
                data_dir,
                socket: Some(socket),
            },
            (None, None, Some(data_dir)) => EndpointSource::Local {
                data_dir: Some(data_dir),
                socket: None,
            },
            (None, None, None) => EndpointSource::Named {
                context: self.context,
                config: self.config,
            },
            (Some(_), Some(_), _) | (Some(_), _, Some(_)) => {
                return Err("WebSocket, socket, and data-dir selections conflict");
            }
        };
        Ok(EndpointOptions {
            source,
            start: self.start_policy.into(),
        })
    }
}

fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();
    let options = match Args::parse().endpoint_options() {
        Ok(options) => options,
        Err(error) => {
            eprintln!("nits-desktop: {error}");
            std::process::exit(2);
        }
    };
    if let Err(e) = nits_client_tauri::run(options) {
        eprintln!("nits-desktop: {e}");
        std::process::exit(1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    static ENV: std::sync::Mutex<()> = std::sync::Mutex::new(());

    #[test]
    fn config_path_is_parsed_from_the_environment() {
        let _guard = ENV.lock().unwrap();
        let previous = std::env::var_os("NITS_CONFIG");
        // SAFETY: this is the only test in this test binary that reads or
        // writes NITS_CONFIG, and the mutex keeps future copies serialized.
        unsafe { std::env::set_var("NITS_CONFIG", "/tmp/team-nits.toml") };
        let options = Args::try_parse_from(["nits-desktop"])
            .unwrap()
            .endpoint_options()
            .unwrap();
        match previous {
            // SAFETY: same serialized environment access as above.
            Some(value) => unsafe { std::env::set_var("NITS_CONFIG", value) },
            // SAFETY: same serialized environment access as above.
            None => unsafe { std::env::remove_var("NITS_CONFIG") },
        }
        assert_eq!(
            options.source,
            EndpointSource::Named {
                context: None,
                config: Some(PathBuf::from("/tmp/team-nits.toml"))
            }
        );
    }
}
