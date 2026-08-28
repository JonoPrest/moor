# moor-client-tauri

Desktop wrapper (`moor-desktop`) around `moor-client-host`.

- Run the built UI: `pnpm --dir ui build && cargo run -p moor-client-tauri`.
- Hot reload (Vite HMR for the UI, cargo rebuild for Rust): `cargo install
  tauri-cli --version '^2'` once, then from this directory
  `cargo tauri dev --config tauri.dev.json`. It starts `pnpm dev` for you;
  also run `pnpm --dir ui res:watch` in another terminal so `.res` edits
  recompile.
- `moor-desktop [context]` picks a `moor-config` context; only `Local`
  contexts work so far.
