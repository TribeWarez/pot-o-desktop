# Pot-O Desktop — AGENTS.md

## Stack

- **Tauri 2.0** — Rust backend (`src-tauri/`), vanilla JS frontend (`src/`), Vite bundler
- No framework, no TypeScript, no test suite. JS validation = `vite build` only.

## Key commands

```bash
npm run dev          # Vite frontend dev server on :1420
npm run build        # Vite build → dist/
npm run tauri        # npx tauri proxy (e.g. `npm run tauri build`)

npm run tauri build -- --no-bundle   # production binary (embeds dist/); use for local runs & CI
cargo clippy --manifest-path src-tauri/Cargo.toml   # required before PR
cargo tauri dev      # full Tauri dev with hot-reload
npx tauri build      # platform bundles (.deb/.AppImage/.dmg/.msi)

# Plain `cargo build --release` is NOT a shippable desktop build (WebView stays on :1420).
# CI runs: npm ci → npm run tauri build -- --no-bundle
```

## Architecture

- **Rust crate**: `pot_o_desktop_lib` (not the package name), 5 modules:
  - `lib.rs` — 13 Tauri commands, app state (config + engine + stats)
  - `config.rs` — TOML config at `~/.config/pot-o-desktop/config.toml`
  - `mining.rs` — PoT-O (tensor ops/MML/path search) and Hexchain (SHA-256 PoW)
  - `rpc.rs` — Async HTTP client (30s GET / 60s POST timeouts)
  - `keypair.rs` — Ed25519 gen/load, 64-byte Solana keypair detection
- **Frontend**: `index.html → src/main.js` (SPA, 3 tabs via `[data-tab]`), `src/styles.css`
- Two RPC targets: `rpc_url` (PoT-O validator commands) and `status_url` (gateway health)

## Quirks & conventions

- `Cargo.toml` lives in `src-tauri/`, not root — always pass `--manifest-path`
- Vite port 1420 (strict), HMR port 1421, ignores `src-tauri/**` in file watcher
- Config file: `~/.config/pot-o-desktop/config.toml` — mirrors CLI env vars from pot-o-miner-cli
- `miner_json_path` default: `~/pot-o-miner-cli/miner.json`
- CI auto-releases on push to `main` (tagged `v0.1.0+YYYYMMDD-commitsha`)
- No Rust tests or JS tests exist — do not add without explicit request
- Frontend is vanilla DOM manipulation — no template engine, no virtual DOM
