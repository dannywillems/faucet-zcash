//! Cloudflare Worker faucet API (Rust / workers-rs).
//!
//! Thin authenticated edge API: HTTP Basic Auth bot gate, email OTP via Resend,
//! session management, rate limiting and cooldown backed by D1, and the
//! authenticated call to the signer for the actual transaction. It never holds
//! the seed and never builds transactions.
//!
//! The `#[event(fetch)]` handler and routes land in the Worker task; this stub
//! keeps the crate building while the dependency set is approved.

#![forbid(unsafe_code)]

// Re-export the shared vocabulary so the Worker and signer agree on types.
pub use faucet_core::{Network, Pool};
