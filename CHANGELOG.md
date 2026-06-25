# Changelog

All notable changes to this project are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).

## [Unreleased]

### Added

- Initial repository scaffold: cargo workspace, crate stubs (`faucet-core`,
  `faucet-addr-wasm`, `worker`, `signer`), tooling configs, and project docs.
- `faucet-core`: destination address validation via `zcash_address` (accepts
  transparent and Orchard unified addresses, rejects Sapling/Sprout/mainnet),
  plus the shared DTOs for the frontend, Worker, and signer.
- `faucet-addr-wasm`: `wasm-bindgen` validator (`validate_testnet_address`) so
  the frontend reuses the Rust address logic; ~72 KB wasm bundle.
- `DEPENDENCIES.md` documenting approved crates and resolved versions.

### Infrastructure

- GitHub Actions CI (Rust fmt/clippy/test, wasm builds, `cargo-deny`) and the
  changelog, PR-hygiene, and shellcheck workflows.
- CI drives all checks through Makefile targets and runs a stable + beta Rust
  matrix.
