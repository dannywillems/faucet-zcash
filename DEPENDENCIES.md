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
| `worker` (workers-rs) | pending | worker | Cloudflare Worker runtime + D1 binding. |
| `zcash_client_backend` | pending | signer | Wallet engine: note scanning, proposals, proving. |
| `zcash_client_sqlite` | pending | signer | Wallet data store (notes, witnesses, tree). |
| `zcash_primitives` | pending | signer | Transparent tx building, primitives. |
| `orchard` | pending | signer | Orchard (halo2) note/proof construction. |
| `incrementalmerkletree`, `shardtree` | pending | signer | Note commitment tree + witnesses. |
| `k256` | pending | signer | secp256k1 signing (wasm-friendly, pure Rust). |
| `zeroize` | pending | signer | Scrub the faucet seed in memory. |
| `axum`, `tokio` | pending | signer | HTTP service + async runtime (already approved upstream). |

## Notes

- `zcash_address` + `zcash_protocol` compile cleanly to
  `wasm32-unknown-unknown` with no `getrandom` "js"-feature problem (verified);
  the browser validator wasm is ~72 KB.
- `secp256k1`/`secp256k1-sys` is intentionally avoided: it fails to build for
  `wasm32-unknown-unknown`. `k256` is used instead.
- No Sapling crates (`sapling-crypto`) are pulled in: Sapling is not supported.
- Crates marked "pending" are approved in principle and added as each component
  is implemented; this table is updated with the resolved version at that time.
