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
    /// Account birthday height (the block the faucet wallet was funded near).
    /// Defaults to NU5 activation, which forces a full scan; set this to avoid
    /// scanning the whole chain.
    pub birthday_height: Option<u32>,
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
        let birthday_height = std::env::var("SIGNER_BIRTHDAY_HEIGHT")
            .ok()
            .and_then(|s| s.parse::<u32>().ok());
        let network = match std::env::var("FAUCET_NETWORK").as_deref() {
            Ok("mainnet") => Network::Mainnet,
            _ => Network::Testnet,
        };
        // Accept SIGNER_SEED, or fall back to SEED (the project .env name).
        let seed = std::env::var("SIGNER_SEED")
            .or_else(|_| std::env::var("SEED"))
            .map(Zeroizing::new)
            .map_err(|_| "missing required env var SIGNER_SEED (or SEED)".to_string())?;
        Ok(Self {
            bind,
            shared_secret,
            lightwalletd_url,
            db_path,
            birthday_height,
            network,
            seed,
        })
    }
}
