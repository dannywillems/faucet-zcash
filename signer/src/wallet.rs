//! Faucet wallet engine.
//!
//! Holds the seed and uses the Rust Zcash stack (librustzcash pinned to git
//! main, Orchard 0.14) to derive the faucet account, sync it from the local
//! zaino (lightwalletd gRPC, backed by zebrad), and build, prove, and broadcast
//! transparent + Orchard transactions. Sapling is never used as an output, but
//! `create_proposed_transactions` still requires a Sapling prover argument, so
//! the host must have the Sapling parameters (`~/.zcash-params`).
//!
//! The implementation mirrors the official `zcash-devtool` send flow.

use rand::rngs::OsRng;
use secrecy::{ExposeSecret, SecretVec};
use tonic::transport::Channel;
use zcash_client_backend::data_api::chain::ChainState;
use zcash_client_backend::data_api::wallet::input_selection::GreedyInputSelectorError;
use zcash_client_backend::data_api::wallet::{
    ConfirmationsPolicy, SpendingKeys, create_proposed_transactions,
    propose_standard_transfer_to_address,
};
use zcash_client_backend::data_api::{AccountBirthday, WalletRead, WalletWrite};
use zcash_client_backend::fees::StandardFeeRule;
use zcash_client_backend::proto::service::{
    self, Empty, compact_tx_streamer_client::CompactTxStreamerClient,
};
use zcash_client_backend::wallet::OvkPolicy;
use zcash_client_sqlite::util::SystemClock;
use zcash_client_sqlite::wallet::commitment_tree;
use zcash_client_sqlite::{AccountUuid, WalletDb};
use zcash_keys::address::Address;
use zcash_keys::keys::UnifiedSpendingKey;
use zcash_primitives::block::BlockHash;
use zcash_proofs::prover::LocalTxProver;
use zcash_protocol::ShieldedProtocol;
use zcash_protocol::consensus::{BlockHeight, Network as ZNetwork, NetworkUpgrade, Parameters};
use zcash_protocol::value::Zatoshis;
use zeroize::Zeroizing;

use faucet_core::{Network, Pool};

use crate::error::SignerError;

/// Wallet DB concrete type (file or `:memory:` SQLite, system clock, OS RNG).
type Db = WalletDb<rusqlite::Connection, ZNetwork, SystemClock, OsRng>;

pub struct Wallet {
    network: Network,
    lightwalletd_url: String,
    db_path: String,
    birthday_height: Option<u32>,
    /// Faucet seed (BIP39 mnemonic or hex), held only in scrubbed memory.
    seed: Zeroizing<String>,
}

impl Wallet {
    pub fn new(
        network: Network,
        lightwalletd_url: String,
        db_path: String,
        birthday_height: Option<u32>,
        seed: Zeroizing<String>,
    ) -> Self {
        Self {
            network,
            lightwalletd_url,
            db_path,
            birthday_height,
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
        let s = self.seed.trim();
        // Accept either a BIP39 mnemonic (the project .env form) or raw hex.
        let bytes = if s.split_whitespace().count() > 1 {
            let mnemonic = <bip0039::Mnemonic<bip0039::English>>::from_phrase(s)
                .map_err(|_| SignerError::Internal("invalid BIP39 mnemonic".to_string()))?;
            mnemonic.to_seed("").to_vec()
        } else {
            hex::decode(s)
                .map_err(|_| SignerError::Internal("seed is not valid hex".to_string()))?
        };
        if !(32..=252).contains(&bytes.len()) {
            return Err(SignerError::Internal(
                "seed must decode to 32..=252 bytes".to_string(),
            ));
        }
        Ok(SecretVec::new(bytes))
    }

    fn birthday(&self, params: &ZNetwork) -> Result<AccountBirthday, SignerError> {
        let height = match self.birthday_height {
            Some(h) => BlockHeight::from_u32(h),
            None => params
                .activation_height(NetworkUpgrade::Nu5)
                .ok_or_else(|| {
                    SignerError::Internal("NU5 activation height is not set".to_string())
                })?,
        };
        Ok(AccountBirthday::from_parts(
            ChainState::empty(height - 1, BlockHash([0u8; 32])),
            None,
        ))
    }

    /// Open the wallet DB and run migrations.
    fn open_db(&self) -> Result<Db, SignerError> {
        let mut db = WalletDb::for_path(&self.db_path, self.zcash_network(), SystemClock, OsRng)
            .map_err(|e| SignerError::Internal(format!("open wallet db: {e}")))?;
        zcash_client_sqlite::wallet::init::init_wallet_db(&mut db, None)
            .map_err(|e| SignerError::Internal(format!("init wallet db: {e}")))?;
        Ok(db)
    }

    /// Ensure the faucet account exists in `db`, returning its id. Offline.
    fn ensure_account_in(&self, db: &mut Db) -> Result<AccountUuid, SignerError> {
        let ids = db
            .get_account_ids()
            .map_err(|e| SignerError::Internal(format!("list accounts: {e}")))?;
        if let Some(id) = ids.first() {
            return Ok(*id);
        }
        let birthday = self.birthday(&self.zcash_network())?;
        let seed = self.seed_secret()?;
        let (account_id, _usk) = db
            .create_account("faucet", &seed, &birthday, None)
            .map_err(|e| SignerError::Internal(format!("create account: {e}")))?;
        Ok(account_id)
    }

    /// Ensure the faucet account exists (opens its own DB). Offline.
    pub fn ensure_account(&self) -> Result<AccountUuid, SignerError> {
        let mut db = self.open_db()?;
        self.ensure_account_in(&mut db)
    }

    /// Connect to the configured lightwalletd gRPC (local zaino, or a public
    /// server such as testnet.zec.rocks). `https://` endpoints use TLS.
    async fn connect(&self) -> Result<CompactTxStreamerClient<Channel>, SignerError> {
        let url = self.lightwalletd_url.clone();
        let mut endpoint = Channel::from_shared(url.clone())
            .map_err(|e| SignerError::Internal(format!("bad lightwalletd url: {e}")))?;
        if url.starts_with("https://") {
            let host = url
                .trim_start_matches("https://")
                .split(['/', ':'])
                .next()
                .unwrap_or_default()
                .to_string();
            let tls = tonic::transport::ClientTlsConfig::new()
                .domain_name(host)
                .with_webpki_roots();
            endpoint = endpoint
                .tls_config(tls)
                .map_err(|e| SignerError::Internal(format!("tls config: {e}")))?;
        }
        let channel = endpoint
            .connect()
            .await
            .map_err(|e| SignerError::Internal(format!("connect to lightwalletd: {e}")))?;
        Ok(CompactTxStreamerClient::new(channel))
    }

    /// Current chain tip height reported by zaino. Verifies connectivity.
    pub async fn chain_height(&self) -> Result<u64, SignerError> {
        let mut client = self.connect().await?;
        let info = client
            .get_lightd_info(Empty {})
            .await
            .map_err(|e| SignerError::Internal(format!("get_lightd_info: {e}")))?
            .into_inner();
        Ok(info.block_height)
    }

    /// Build, prove, and broadcast a transaction sending `amount_zat` to
    /// `address`, returning the broadcast txid. Syncs the wallet first.
    pub async fn send(
        &self,
        address: &str,
        amount_zat: u64,
        _pool: Pool,
    ) -> Result<String, SignerError> {
        let params = self.zcash_network();
        let mut db = self.open_db()?;
        let account_id = self.ensure_account_in(&mut db)?;

        // Sync the wallet from zaino into an in-memory compact-block cache.
        let mut client = self.connect().await?;
        let db_cache = crate::blockcache::MemBlockCache::new();
        // Small batches keep the per-request load on zaino/zebra low. `sync::run`
        // resumes from the wallet's stored progress, so on a transient
        // rate-limit (zebra 429) we back off and retry, making incremental
        // headway rather than failing the whole drip.
        let mut attempts = 0u32;
        loop {
            match zcash_client_backend::sync::run(&mut client, &params, &db_cache, &mut db, 1_000)
                .await
            {
                Ok(()) => break,
                Err(e) => {
                    let msg = format!("{e}");
                    attempts += 1;
                    if attempts > 60 || !msg.contains("429") {
                        return Err(SignerError::Internal(format!("sync: {msg}")));
                    }
                    tracing::warn!(attempts, "sync rate-limited (429); backing off");
                    tokio::time::sleep(std::time::Duration::from_secs(2)).await;
                }
            }
        }

        // Build and prove the transfer.
        let prover = LocalTxProver::with_default_location().ok_or_else(|| {
            SignerError::Internal(
                "Sapling params not found in ~/.zcash-params (run fetch-params)".to_string(),
            )
        })?;
        let seed = self.seed_secret()?;
        let usk =
            UnifiedSpendingKey::from_seed(&params, seed.expose_secret(), zip32::AccountId::ZERO)
                .map_err(|e| SignerError::Internal(format!("derive spending key: {e}")))?;
        let to = Address::decode(&params, address)
            .ok_or_else(|| SignerError::BadRequest("unparseable address".to_string()))?;
        let amount = Zatoshis::from_u64(amount_zat)
            .map_err(|_| SignerError::Internal("bad amount".into()))?;

        let proposal = propose_standard_transfer_to_address::<_, _, commitment_tree::Error>(
            &mut db,
            &params,
            StandardFeeRule::Zip317,
            account_id,
            ConfirmationsPolicy::default(),
            &to,
            amount,
            None,
            None,
            ShieldedProtocol::Orchard,
            None,
        )
        .map_err(|e| SignerError::Internal(format!("propose: {e}")))?;

        let txids = create_proposed_transactions::<
            _,
            _,
            GreedyInputSelectorError,
            _,
            zcash_primitives::transaction::fees::zip317::FeeError,
            _,
        >(
            &mut db,
            &params,
            &prover,
            &prover,
            &SpendingKeys::from_unified_spending_key(usk),
            OvkPolicy::Sender,
            &proposal,
            None,
        )
        .map_err(|e| SignerError::Internal(format!("create transaction: {e}")))?;

        // Broadcast.
        let mut result_txid = None;
        for txid in txids.iter() {
            let tx = db
                .get_transaction(*txid)
                .map_err(|e| SignerError::Internal(format!("load transaction: {e}")))?
                .ok_or_else(|| SignerError::Internal("transaction missing after build".into()))?;
            let mut raw = service::RawTransaction::default();
            tx.write(&mut raw.data)
                .map_err(|e| SignerError::Internal(format!("serialize transaction: {e}")))?;
            let resp = client
                .send_transaction(raw)
                .await
                .map_err(|e| SignerError::Internal(format!("broadcast: {e}")))?
                .into_inner();
            if resp.error_code != 0 {
                return Err(SignerError::Internal(format!(
                    "broadcast rejected ({}): {}",
                    resp.error_code, resp.error_message
                )));
            }
            result_txid = Some(tx.txid());
        }
        result_txid
            .map(|t| t.to_string())
            .ok_or_else(|| SignerError::Internal("no transaction produced".to_string()))
    }
}
