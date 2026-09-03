# nits-client-tauri

Desktop wrapper (`nits-desktop`) around `nits-client-host`.

- Run the built UI: `pnpm --dir ui build && cargo run -p nits-client-tauri`.
- Hot reload (Vite HMR for the UI, cargo rebuild for Rust): `cargo install
  tauri-cli --version '^2'` once, then from this directory
  `cargo tauri dev --config tauri.dev.json`. It starts `pnpm dev` for you;
  also run `pnpm --dir ui res:watch` in another terminal so `.res` edits
  recompile.
- `nits-desktop [context]` picks any named `nits-config` context (`Local`,
  `Ssh`, or `Ws`); `--socket`/`--data-dir` and `--ws` preserve the ad-hoc
  selections accepted by `nits --ui desktop`, and `--start-policy` preserves
  lifecycle behavior. SSH uses the configured remote `nits daemon stdio`
  proxy; WebSocket contexts connect directly.
- `capabilities/default.json` grants the main window `core:default` (events).
  Without it Tauri 2 silently denies `listen`/`invoke` and the UI stays
  "disconnected"; webview errors are forwarded to the Rust log via the
  `client_error` command (`RUST_LOG=nits_client_tauri=debug`).
