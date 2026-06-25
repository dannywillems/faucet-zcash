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
- `faucet-worker`: Cloudflare Worker faucet API on D1 (HTTP Basic Auth gate,
  Resend email OTP, sessions, per email/address/IP cooldown, signer call), with
  the D1 schema migration and `wrangler.toml`.
- Frontend: SvelteKit + Tailwind static SPA with dark/light theme, the
  email/OTP/drip flow, in-browser wasm address validation, the Pages Basic Auth
  gate, and Playwright e2e. CI gains a frontend job.
- `faucet-signer`: native axum service skeleton (bearer-authenticated `/send`,
  `/health`, zeroized seed + secret, server-side address re-validation). The
  zcash_client_backend transaction engine is documented but not yet wired
  (`/send` returns 503 until then).
- Deploy: Cloudflare `deploy-pages` and `deploy-worker` workflows, signer
  Dockerfile + docker-compose (with cloudflared tunnel), and a deploy runbook.

### Changed

- All crate dependencies are centralized in `[workspace.dependencies]` and
  referenced from member crates with `{ workspace = true }`.

### Infrastructure

- GitHub Actions CI (Rust fmt/clippy/test, wasm builds, `cargo-deny`) and the
  changelog, PR-hygiene, and shellcheck workflows.
- CI drives all checks through Makefile targets and runs a stable + beta Rust
  matrix.
