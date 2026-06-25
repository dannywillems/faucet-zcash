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
- Signer split into lib + bin with end-to-end service tests (auth, address
  validation, routing, error mapping).
- Signer wallet engine on the latest librustzcash (git-pinned `main`, Orchard
  0.14): opens the wallet DB, migrates, and derives the faucet account from the
  seed (runtime-tested). Config and deploy target a local zaino + zebrad.
- Signer connects to zaino (lightwalletd gRPC) and exposes an authenticated
  `/info` endpoint (network, account, chain height); the seed may be a BIP39
  mnemonic or hex. Verified live against local zebra + zaino.
- Signer full send pipeline: syncs the account (with bounded back-off retry on
  zebra 429 rate-limits), builds + proves (Orchard/transparent) and broadcasts
  via `send_transaction`, returning the txid; `SIGNER_BIRTHDAY_HEIGHT` bounds
  the initial scan.
- Signer supports TLS lightwalletd endpoints and defaults to
  `https://testnet.zec.rocks:443` (overridable to a local zaino).
- Server-side logout (`/api/auth/logout`) and a session-aware frontend that
  restores the signed-in state on load and offers a log-out control.
- Frontend `/api/*` is proxied to the Worker by a Pages Function (same origin),
  and the Worker restricts OTP recipients to an email-domain allowlist.

### Changed

- All crate dependencies are centralized in `[workspace.dependencies]` and
  referenced from member crates with `{ workspace = true }`.

### Security

- Pin transitive `cookie` to `>=0.7.2` via an npm override to clear a
  low-severity advisory pulled in by `@sveltejs/kit`.

### Infrastructure

- GitHub Actions CI (Rust fmt/clippy/test, wasm builds, `cargo-deny`) and the
  changelog, PR-hygiene, and shellcheck workflows.
- Dependabot for cargo, npm, and github-actions (weekly, grouped). The
  changelog-entry check is skipped for Dependabot PRs; `rand`/`secrecy` majors
  are ignored (pinned by the librustzcash API types).
- Worker `wrangler.toml` carries the real D1 `database_id` and pins
  `worker-build` to 0.8.5 (matching workers-rs); the Worker, D1 schema, Pages
  site, and Resend/auth secrets are deployed.

### Changed

- Workspace moved to Rust edition 2024; `[workspace.dependencies]` grouped by
  role (Zcash together) and wrapped at 80 columns.
- CI drives all checks through Makefile targets and runs a stable + beta Rust
  matrix.
