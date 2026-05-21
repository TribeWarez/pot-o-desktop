# Pot-O Desktop

[![CI](https://github.com/TribeWarez/pot-o-desktop/actions/workflows/ci.yml/badge.svg)](https://github.com/TribeWarez/pot-o-desktop/actions)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)

A cross-platform GUI wallet and miner for the TribeWarez ecosystem. Built with [Tauri 2.0](https://v2.tauri.app).

- **Settings** — all CLI flags/env vars from `pot-o-miner-cli`, saved as TOML
- **Dashboard** — live view of gateway services, PoT-O validator, network peers, pool, mining stats
- **Keys** — Ed25519 keypair generation/import, 64-byte Solana keypair detection

## Ecosystem

| Project | Description |
|---------|-------------|
| [pot-o-validator](https://github.com/TribeWarez/pot-o-validator) | PoT-O RPC validator — axum HTTP server, challenge generation, proof verification, Solana bridge |
| [pot-o-miner-cli](https://github.com/TribeWarez/pot-o-miner-cli) | Bash/Python CLI miner — tensor ops, MML compression, neural path search, hexchain PoW |
| [pot-o-desktop](https://github.com/TribeWarez/pot-o-desktop) | **You are here** — Tauri GUI replacement for the CLI/TUI |
| [defi.tribewarez.com](https://defi.tribewarez.com) | DeFi dashboard — swap quotes, pool stats, token analytics |

## Requirements

- **Rust** 1.75+ (install via [rustup](https://rustup.rs))
- **Node.js** 20+ (install via [nvm](https://github.com/nvm-sh/nvm) or [nodejs.org](https://nodejs.org))
- **System libs** — Linux: GTK3/WebKit2GTK, macOS: Xcode CLI tools, Windows: WebView2 (included in Win10 1803+)

## Build

```bash
# Install dependencies
npm install

# Production binary (frontend embedded; no installer)
npm run tauri build -- --no-bundle

# Full platform installer bundle (.deb/.AppImage/.dmg/.msi)
npx tauri build
```

Plain `cargo build --release` does not embed the frontend; use `tauri build` for runnable apps.

## Usage

```bash
# Development mode (hot-reload)
cargo tauri dev

# Run the production binary (after `tauri build --no-bundle`)
./src-tauri/target/release/pot-o-desktop
```

## Architecture

```
src-tauri/src/
├── config.rs    → TOML settings (mirrors 15 CLI env vars)
├── mining.rs    → Mining engine (tensor ops, MML, neural path, hexchain PoW)
├── rpc.rs       → Async HTTP client (validator RPC calls)
├── keypair.rs   → Ed25519 keypair gen/load (ed25519-dalek)
└── lib.rs       → 13 Tauri commands

src/
├── main.js      → Settings + Dashboard + Keys UI
└── styles.css   → Dark theme
```

## License

MIT — see [LICENSE](LICENSE).
