//! Faucet wallet engine.
//!
//! Holds the seed and uses the Rust Zcash stack (librustzcash pinned to git
//! main, Orchard 0.14) to build, prove, and broadcast transactions.
//!
//! What is implemented and verifiable offline:
//! - [`Wallet::ensure_account`] opens the `WalletDb` (SQLite), runs migrations,
//!   and derives/creates the faucet account from the seed. This is exercised by
//!   the integration tests against an in-memory database.
//!
//! What remains (requires the live local zebra + zaino and a funded faucet
//! wallet to verify end to end, so it is implemented separately):
//! - sync the account via `zcash_client_backend::sync::run` against zaino's
//!   lightwalletd gRPC (`CompactTxStreamerClient`),
//! - `propose_standard_transfer_to_address` for `amount_zat` to the recipient,
//! - `create_proposed_transactions` with the Orchard proving key + transparent
//!   keys (Sapling never used),
//! - broadcast with `CompactTxStreamerClient::send_transaction` and return txid.

use faucet_core::{Network, Pool};
use rand::rngs::OsRng;
use secrecy::SecretVec;
use zcash_client_backend::data_api::chain::ChainState;
use zcash_client_backend::data_api::{AccountBirthday, WalletRead, WalletWrite};
use zcash_client_sqlite::util::SystemClock;
use zcash_client_sqlite::{AccountUuid, WalletDb};
use zcash_primitives::block::BlockHash;
use zcash_protocol::consensus::{Network as ZNetwork, NetworkUpgrade, Parameters};
use zeroize::Zeroizing;

use crate::error::SignerError;

pub struct Wallet {
    network: Network,
    lightwalletd_url: String,
    db_path: String,
    /// Faucet seed (hex), held only in scrubbed memory.
    seed: Zeroizing<String>,
}

impl Wallet {
    pub fn new(
        network: Network,
        lightwalletd_url: String,
        db_path: String,
        seed: Zeroizing<String>,
    ) -> Self {
        Self {
            network,
            lightwalletd_url,
            db_path,
            seed,
        }
    }

    fn zcash_network(&self) -> ZNetwork {
        match self.network {
            Network::Testnet => ZNetwork::TestNetwork,
            Network::Mainnet => ZNetwork::MainNetwork,
        }
    }

    fn seed_secret(&self) -> Result<SecretVec<u8>, SignerError> {
        let bytes = hex::decode(self.seed.trim())
            .map_err(|_| SignerError::Internal("seed is not valid hex".to_string()))?;
        if !(32..=252).contains(&bytes.len()) {
            return Err(SignerError::Internal(
                "seed must decode to 32..=252 bytes".to_string(),
            ));
        }
        Ok(SecretVec::new(bytes))
    }

    /// Open the wallet database (creating and migrating it on first run) and
    /// ensure the faucet account exists, returning its id. Fully offline.
    pub fn ensure_account(&self) -> Result<AccountUuid, SignerError> {
        let params = self.zcash_network();
        let mut db = WalletDb::for_path(&self.db_path, params, SystemClock, OsRng)
            .map_err(|e| SignerError::Internal(format!("open wallet db: {e}")))?;
        zcash_client_sqlite::wallet::init::init_wallet_db(&mut db, None)
            .map_err(|e| SignerError::Internal(format!("init wallet db: {e}")))?;

        let ids = db
            .get_account_ids()
            .map_err(|e| SignerError::Internal(format!("list accounts: {e}")))?;
        if let Some(id) = ids.first() {
            return Ok(*id);
        }

        // First run: create the faucet account from the seed, with a birthday
        // at NU5 (Orchard) activation (the public equivalent of the test-only
        // `from_activation` helper).
        let activation = params
            .activation_height(NetworkUpgrade::Nu5)
            .ok_or_else(|| SignerError::Internal("NU5 activation height is not set".to_string()))?;
        let birthday = AccountBirthday::from_parts(
            ChainState::empty(activation - 1, BlockHash([0u8; 32])),
            None,
        );
        let seed = self.seed_secret()?;
        let (account_id, _usk) = db
            .create_account("faucet", &seed, &birthday, None)
            .map_err(|e| SignerError::Internal(format!("create account: {e}")))?;
        Ok(account_id)
    }

    /// Build, prove, and broadcast a transaction sending `amount_zat` to
    /// `address` in the given `pool`, returning the broadcast txid.
    pub async fn send(
        &self,
        address: &str,
        amount_zat: u64,
        pool: Pool,
    ) -> Result<String, SignerError> {
        // Offline: ensure the wallet DB + faucet account exist.
        let account = self.ensure_account()?;
        tracing::info!(
            ?account,
            lightwalletd = %self.lightwalletd_url,
            %address,
            amount_zat,
            ?pool,
            "faucet account ready; live sync + broadcast against zebra + zaino pending",
        );
        Err(SignerError::NotReady(
            "live sync and broadcast against zebra + zaino is not yet enabled".to_string(),
        ))
    }
}
