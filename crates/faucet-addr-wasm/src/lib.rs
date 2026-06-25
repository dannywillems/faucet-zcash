//! Thin `wasm-bindgen` wrapper exposing faucet address validation to the
//! SvelteKit frontend, so the browser reuses the Rust `zcash_address` logic in
//! [`faucet_core`] instead of reimplementing address parsing in TypeScript.
//!
//! This is deliberately NOT WebZjs: it only validates addresses (no wallet, no
//! note scanning, no proving, no parameters), so it stays tiny and compiles
//! cleanly to `wasm32-unknown-unknown` without threads or `SharedArrayBuffer`.

use faucet_core::{Network, Pool, validate_destination};
use wasm_bindgen::prelude::*;

/// Result of validating a destination address, shaped for easy consumption in
/// JavaScript: `{ valid, pool, error }`.
#[wasm_bindgen]
pub struct ValidationResult {
    valid: bool,
    pool: Option<String>,
    error: Option<String>,
}

#[wasm_bindgen]
impl ValidationResult {
    /// Whether the address is a valid testnet destination for the faucet.
    #[wasm_bindgen(getter)]
    #[must_use]
    pub fn valid(&self) -> bool {
        self.valid
    }

    /// The pool the faucet would send into (`"transparent"` or `"orchard"`),
    /// or `undefined` when the address is invalid.
    #[wasm_bindgen(getter)]
    #[must_use]
    pub fn pool(&self) -> Option<String> {
        self.pool.clone()
    }

    /// A user-facing rejection message, or `undefined` when the address is
    /// valid.
    #[wasm_bindgen(getter)]
    #[must_use]
    pub fn error(&self) -> Option<String> {
        self.error.clone()
    }
}

fn pool_str(pool: Pool) -> String {
    match pool {
        Pool::Transparent => "transparent",
        Pool::Orchard => "orchard",
    }
    .to_owned()
}

/// Validate a destination address against Zcash **testnet** rules: accepts
/// transparent and Orchard-bearing unified addresses, rejects Sapling, Sprout,
/// mainnet, and unparseable input.
#[wasm_bindgen]
#[must_use]
pub fn validate_testnet_address(addr: &str) -> ValidationResult {
    match validate_destination(addr, Network::Testnet) {
        Ok(valid) => ValidationResult {
            valid: true,
            pool: Some(pool_str(valid.pool)),
            error: None,
        },
        Err(rejection) => ValidationResult {
            valid: false,
            pool: None,
            error: Some(rejection.message().to_owned()),
        },
    }
}
