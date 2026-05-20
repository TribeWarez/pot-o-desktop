# Contributing

Contributions are welcome. This repository is maintained by [TribeWarez](https://github.com/TribeWarez).

## How to contribute

1. Fork the repository and create a branch from `main`.
2. Make your changes. The app is built with Tauri 2.0 (Rust backend, JS frontend); ensure it compiles on all platforms.
3. Test locally:
   ```bash
   cd pot-o-desktop
   npm run build          # frontend
   cargo build --release  # backend
   ```
4. Run `cargo clippy` and fix any warnings before opening a PR.
5. Open a pull request with a clear description of the change.

## Code of conduct

Be respectful and constructive. We follow a code of conduct aligned with the TribeWarez ecosystem.

## License

By contributing, you agree that your contributions will be licensed under the MIT License.
