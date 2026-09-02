# nits-client-tauri

Desktop wrapper (`nits-desktop`) around `nits-client-host`.

- Run the built UI: `pnpm --dir ui build && cargo run -p nits-client-tauri`.
- Hot reload (Vite HMR for the UI, cargo rebuild for Rust): `cargo install
  tauri-cli --version '^2'` once, then from this directory
  `cargo tauri dev --config tauri.dev.json`. It starts `pnpm dev` for you;
  also run `pnpm --dir ui res:watch` in another terminal so `.res` edits
  recompile.
- `nits-desktop [context]` picks a `nits-config` context; only `Local`
  contexts work so far.
- `capabilities/default.json` grants the main window `core:default` (events).
  Without it Tauri 2 silently denies `listen`/`invoke` and the UI stays
  "disconnected"; webview errors are forwarded to the Rust log via the
  `client_error` command (`RUST_LOG=nits_client_tauri=debug`).
