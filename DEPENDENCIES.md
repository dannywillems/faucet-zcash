# Dependencies

This file records the dependencies approved for this project and the rationale,
per the dependency-approval policy. Exact versions are pinned in `Cargo.lock`
(Rust) and `package-lock.json` (frontend). `cargo audit` and `cargo deny` run in
CI to catch advisories and license drift.

## Approved Rust crates

| Crate | Resolved | Where | Purpose |
| --- | --- | --- | --- |
| `zcash_address` | 0.12.0 | faucet-core | Parse/validate Zcash addresses (native + wasm32). |
| `zcash_protocol` | 0.9.0 | faucet-core | `NetworkType`, `PoolType` for validation. |
| `serde` | 1.0.228 | faucet-core | DTO (de)serialization across layers. |
| `wasm-bindgen` | 0.2.126 | faucet-addr-wasm | Browser bindings for address validation. |
| `worker` (workers-rs) | 0.8.5 (feature `d1`) | worker | Cloudflare Worker runtime + D1 binding. |
| `getrandom` | 0.3 (feature `wasm_js`) | worker | CSPRNG for OTP codes and session tokens. |
| `sha2`, `hex` | 0.11 / 0.4 | worker | Hash OTP codes and session tokens at rest. |
| `serde_json` | 1.0 | worker | JSON request/response bodies. |
| `axum` | 0.8.9 | signer | HTTP service. |
| `tokio` | 1.52 (rt-multi-thread, macros, net) | signer | Async runtime. |
| `tracing`, `tracing-subscriber` | 0.1 / 0.3 | signer | Structured logging. |
| `thiserror` | 2.0 | signer | Error types. |
| `zeroize` | 1.9 | signer | Scrub the faucet seed + auth secret in memory. |
| `zcash_client_backend`, `zcash_client_sqlite`, `zcash_primitives`, `zcash_protocol` | librustzcash git `main` @ `8a0ae65` | signer | Wallet engine (the latest crates.io pair does not compile together; see deny.toml `allow-git`). |
| `orchard` | 0.14 | signer (via backend feature) | Orchard (halo2) proving. |
| `secrecy`, `rand`, `hex` | 0.8 / 0.8 / 0.4 | signer | Seed handling (SecretVec), OsRng, hex decode. |
| `incrementalmerkletree`, `shardtree` | via client_backend | signer | Note commitment tree + witnesses. |
| `k256` | pending | signer | secp256k1 signing (transparent inputs), added with the broadcast step. |

## Notes

- `zcash_address` + `zcash_protocol` compile cleanly to
  `wasm32-unknown-unknown` with no `getrandom` "js"-feature problem (verified);
  the browser validator wasm is ~72 KB.
- `secp256k1`/`secp256k1-sys` is intentionally avoided: it fails to build for
  `wasm32-unknown-unknown`. `k256` is used instead.
- No Sapling crates (`sapling-crypto`) are pulled in: Sapling is not supported.
- Crates marked "pending" are approved in principle and added as each component
  is implemented; this table is updated with the resolved version at that time.
