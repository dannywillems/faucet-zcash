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
- Signer `/balance` (faucet unified address + per-pool balances) and `/shield`
  (shields transparent funds into Orchard so they can be dripped); shielding
  requires 100 confirmations so only mature mining coinbase is selected.
- Worker `GET /api/faucet/balance` and a frontend "Faucet reserves" panel
  (per pool) plus a "How it works" section describing the backend flow,
  including automatic shielding of transparent coinbase into Orchard.
- Server-side logout (`/api/auth/logout`) and a session-aware frontend that
  restores the signed-in state on load and offers a log-out control.
- Frontend `/api/*` is proxied to the Worker by a Pages Function (same origin),
  and the Worker restricts OTP recipients to an email-domain allowlist.
- Chain-liveness heartbeat as a Cloudflare Worker Cron Trigger
  (`*/5 * * * *`): the Worker's scheduled handler has the faucet self-send a
  tiny Orchard amount to its own unified address every 5 minutes, exercising
  the full sync/build/prove/broadcast path on a schedule. Deployed on
  Cloudflare by CI (`deploy-worker`); no host process. Tunable via the
  `HEARTBEAT_AMOUNT_ZAT` Worker var; a no-op until `SIGNER_URL` is set.
- Single-writer wallet-DB lock in the signer (`signer/src/dblock.rs`): an
  exclusive `flock` on `<db>.lock` taken at startup. A second instance logs the
  contention and exits non-zero, so an orchestrator restarts it until the holder
  releases the lock; the lock is released automatically when the process exits.
  The wallet DB now lives in the repo `data/` directory (gitignored); the signer
  docker service bind-mounts it (`../data:/data`, `SIGNER_DB_PATH=/data/wallet.db`)
  and uses `restart: on-failure`.
- `deploy/faucet-orchard-generator.sh`: host-side Orchard activity generator
  (behind the tunnel) that fires a burst (default 10) of small Orchard
  self-sends per run, intended for a per-minute cron, to keep the Orchard pool
  continuously active. Tolerates per-transaction failures and uses a
  single-flight lock so bursts cannot overlap.
- `docs/INFRASTRUCTURE.md`: how the Worker reaches the host-run signer over a
  Cloudflare Tunnel (outbound-only, no inbound ports), the background-job data
  flow, and the security model; cross-linked from the READMEs.
- Background-services status card on the frontend, backed by a new
  `GET /api/faucet/services` endpoint: reports the Worker API, heartbeat job
  (both on Cloudflare), and the signer service, Zcash node (zebra + zaino), and
  maintenance job (on the faucet host, reached over the tunnel) with a coarse
  status (operational/degraded/down), a one-line live detail, a description of
  what each service is, and a link to its source. The Worker API entry lists
  its actual HTTP endpoints. The heartbeat result is persisted in D1
  (`heartbeat` table) by the scheduled handler; maintenance liveness is
  inferred from the reserves-snapshot freshness.

### Fixed

- OTP send no longer returns a raw 500, and no longer locks an address out,
  when email delivery fails. The Worker now sends the email before recording the
  code, so a provider rejection (e.g. an unverified sender domain) leaves no row
  to trip the resend throttle, and it returns a clear message ("try again in a
  couple of minutes, contact the administrator if it persists") instead of a
  500. The provider error is logged server-side, never surfaced. The resend
  throttle message is clearer too.

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
