//! Signer configuration, loaded from the environment. Secret material (the
//! shared auth token and the faucet seed) is wrapped in `Zeroizing` so it is
//! scrubbed from memory on drop.

use faucet_core::Network;
use zeroize::Zeroizing;

pub struct Config {
    /// Address the HTTP service binds to (default 127.0.0.1:8080; reached by
    /// the Worker over a Cloudflare Tunnel in production).
    pub bind: String,
    /// Bearer token the Worker must present on `/send`.
    pub shared_secret: Zeroizing<String>,
    /// lightwalletd-protocol gRPC endpoint used to sync the faucet wallet and
    /// broadcast. Defaults to a local zaino (backed by zebrad).
    pub lightwalletd_url: String,
    /// Path to the wallet SQLite database (created on first run).
    pub db_path: String,
    /// Network the faucet operates on (testnet).
    pub network: Network,
    /// The faucet seed (hex), held only in scrubbed memory.
    pub seed: Zeroizing<String>,
}

fn required(key: &str) -> Result<String, String> {
    std::env::var(key).map_err(|_| format!("missing required env var {key}"))
}

impl Config {
    pub fn from_env() -> Result<Self, String> {
        let bind = std::env::var("SIGNER_BIND").unwrap_or_else(|_| "127.0.0.1:8080".to_string());
        let shared_secret = Zeroizing::new(required("SIGNER_SHARED_SECRET")?);
        let lightwalletd_url = std::env::var("LIGHTWALLETD_URL")
            .unwrap_or_else(|_| "http://127.0.0.1:8137".to_string());
        let db_path =
            std::env::var("SIGNER_DB_PATH").unwrap_or_else(|_| "faucet-wallet.db".to_string());
        let network = match std::env::var("FAUCET_NETWORK").as_deref() {
            Ok("mainnet") => Network::Mainnet,
            _ => Network::Testnet,
        };
        let seed = Zeroizing::new(required("SIGNER_SEED")?);
        Ok(Self {
            bind,
            shared_secret,
            lightwalletd_url,
            db_path,
            network,
            seed,
        })
    }
}
