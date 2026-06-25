//! Faucet wallet engine.
//!
//! This is the component that holds the seed and uses the Rust Zcash stack to
//! build, prove, and broadcast transactions. The intended implementation, using
//! `zcash_client_backend` 0.23 + `zcash_client_sqlite`, is:
//!
//! 1. Open a `WalletDb` (SQLite) and, on first run, create the faucet account
//!    from the seed with a birthday height (`WalletDb::create_account`).
//! 2. Sync against the configured lightwalletd via the tonic
//!    `CompactTxStreamerClient` using `zcash_client_backend::sync::run`, which
//!    scans Orchard notes and transparent UTXOs and maintains the note
//!    commitment tree (`incrementalmerkletree`/`shardtree`).
//! 3. For a drip: `propose_transfer` to a single recipient for `amount_zat`,
//!    then `create_proposed_transactions` with the Orchard proving key (built
//!    once at startup) and the transparent signing keys. Sapling is never used.
//! 4. Broadcast the resulting transaction with the lightwalletd
//!    `send_transaction` RPC and return the txid.
//!
//! That pipeline requires a funded testnet wallet and a live lightwalletd to
//! verify end to end, so it is wired separately; until then `send` reports that
//! the engine is not ready rather than returning a fabricated txid.

use faucet_core::{Network, Pool};
use zeroize::Zeroizing;

use crate::error::SignerError;

pub struct Wallet {
    network: Network,
    lightwalletd_url: String,
    // Held in scrubbed memory; consumed by the wallet engine when wired.
    seed: Zeroizing<String>,
}

impl Wallet {
    pub fn new(network: Network, lightwalletd_url: String, seed: Zeroizing<String>) -> Self {
        Self {
            network,
            lightwalletd_url,
            seed,
        }
    }

    /// Build, prove, and broadcast a transaction sending `amount_zat` to
    /// `address` in the given `pool`, returning the broadcast txid.
    pub async fn send(
        &self,
        address: &str,
        amount_zat: u64,
        pool: Pool,
    ) -> Result<String, SignerError> {
        // Reference the configured fields so the wallet is fully constructed and
        // ready for the engine; the seed length is logged, never its contents.
        tracing::warn!(
            network = ?self.network,
            lightwalletd = %self.lightwalletd_url,
            seed_len = self.seed.len(),
            %address,
            amount_zat,
            pool = ?pool,
            "drip requested, but the zcash_client_backend engine is not yet wired"
        );
        Err(SignerError::NotReady(
            "transaction construction is not yet enabled on this signer".to_string(),
        ))
    }
}
