//! Shared types and validation for the Zcash testnet faucet.
//!
//! This crate is compiled both natively (the signer and the Worker logic) and
//! to `wasm32` (the Worker runtime and, via `faucet-addr-wasm`, the browser).
//! Keep its dependency surface small and wasm-friendly.
//!
//! Address validation against the real `zcash_address` crate is added once
//! dependency approval is recorded; for now this module pins down the shared
//! vocabulary (network, supported pools, validation outcome) used everywhere.

#![forbid(unsafe_code)]

/// Zcash network the faucet operates on. This faucet is testnet only.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Network {
    Testnet,
    Mainnet,
}

/// Value pool a destination address can receive into. Sapling is intentionally
/// excluded: the faucet does not build or accept Sapling outputs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Pool {
    Transparent,
    Orchard,
}

/// Why a destination address was rejected.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AddressRejection {
    /// Not a parseable Zcash address.
    Unparseable,
    /// Parsed, but for the wrong network (e.g. a mainnet address on testnet).
    WrongNetwork,
    /// A Sapling-only address, which this faucet does not support.
    SaplingUnsupported,
    /// A Unified Address that exposes neither a transparent nor an Orchard
    /// receiver.
    NoSupportedReceiver,
}

/// Result of validating a destination address for a faucet drip.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidAddress {
    /// The pool the faucet will send into for this address.
    pub pool: Pool,
}
