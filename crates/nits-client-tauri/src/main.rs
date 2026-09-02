#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();
    // `nits-desktop [context]`; no clap for one optional positional.
    let context = std::env::args().nth(1);
    if let Err(e) = nits_client_tauri::run(context) {
        eprintln!("nits-desktop: {e}");
        std::process::exit(1);
    }
}
