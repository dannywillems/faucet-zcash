# faucet-zcash

A faucet for **Zcash testnet** (TAZ). It drips small amounts of testnet funds to
authenticated users, gated behind an HTTP Basic Auth bot filter and per-user
email OTP. Transactions are built and proved with the official Rust Zcash stack
(librustzcash, `orchard`, `incrementalmerkletree`); none of the cryptography is
reimplemented in TypeScript.

## Supported pools

- Transparent (`tm...`) addresses.
- Orchard, via Unified Addresses (`utest1...`) that expose an Orchard receiver.
- Sapling is **not** supported. Sapling-only destination addresses are rejected.

## Architecture

The system is three deployables plus shared Rust crates in one workspace.

```
+-------------------+      +--------------------------+      +-----------------------------+
|  Frontend (Pages) | ---> |  Worker API (workers-rs) | ---> |  Signer (native Rust, axum) |
|  SvelteKit +      | HTTP |  + D1 (users/OTP/drips)  | HTTP |  holds seed, builds/proves  |
|  Tailwind + WASM  |      |  Resend OTP, Basic Auth, |      |  via librustzcash, then     |
|  addr validation  |      |  rate limit, cooldown    |      |  broadcasts to zcashd       |
+-------------------+      +--------------------------+      +-----------------------------+
                                                                   |            ^
                                                                   v            |
                                                          zcashd (testnet) + lightwalletd
```

Why the split: Orchard (halo2) proving cannot run inside a Cloudflare Worker
(single-threaded, no `SharedArrayBuffer`/Web Workers for rayon, tight CPU). The
seed-holding signer therefore runs natively on the node host; the Worker stays a
thin authenticated API that owns D1 and calls the signer over an authenticated
channel (Cloudflare Tunnel).

## Repository layout

| Path                       | What                                                        |
| -------------------------- | ----------------------------------------------------------- |
| `crates/faucet-core`       | Shared DTOs + `zcash_address` validation (native + wasm32)  |
| `crates/faucet-addr-wasm`  | `wasm-bindgen` address validator for the frontend           |
| `worker`                   | Cloudflare Worker (Rust) + D1 migrations                    |
| `signer`                   | Native axum service: seed, build/prove/sign, broadcast      |
| `frontend`                 | SvelteKit + Tailwind UI                                      |
| `deploy`                   | docker-compose: zcashd + lightwalletd + signer + cloudflared |

## Development

See the `Makefile` (`make help`) for all commands. Quick start is documented per
component in its own directory once implemented.

## Secrets (never committed)

- `CLOUDFLARE_API_TOKEN`, `CLOUDFLARE_ACCOUNT_ID` (deploys)
- `RESEND_API_TOKEN` (OTP email)
- `BASIC_AUTH_USER` / `BASIC_AUTH_PASS` (edge bot gate)
- `SIGNER_SHARED_SECRET` (Worker to signer auth)
- faucet seed (signer host only)

## License

MIT. See [LICENSE](LICENSE).
