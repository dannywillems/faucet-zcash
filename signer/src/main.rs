//! Faucet signer service entry point.
//!
//! Runs natively on the node host (next to zcashd + lightwalletd). Holds the
//! faucet seed, builds and proves transparent and Orchard transactions via the
//! Rust Zcash stack, and broadcasts them. Exposes an authenticated `/send`
//! endpoint that the Cloudflare Worker calls over a Cloudflare Tunnel.
//!
//! Implementation lands in the signer task; this stub keeps the binary
//! building while the dependency set is approved.

fn main() {
    println!("faucet-signer: not yet implemented");
}
