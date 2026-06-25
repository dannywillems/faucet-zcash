//! Thin `wasm-bindgen` wrapper that exposes faucet address validation to the
//! SvelteKit frontend, so the browser reuses the Rust Zcash address logic
//! instead of reimplementing it in TypeScript.
//!
//! This is deliberately NOT WebZjs: it only validates addresses (no wallet, no
//! proving, no parameters), so it stays tiny and compiles cleanly to wasm32.
//!
//! The `wasm_bindgen` export is wired up once dependency approval is recorded.

#![forbid(unsafe_code)]

// Placeholder so the crate compiles before wasm-bindgen is added. Replaced by
// the real exported `validate_address` function in the implementation step.
pub use faucet_core::{Pool, ValidAddress};
