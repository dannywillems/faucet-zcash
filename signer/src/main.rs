//! Faucet signer service entry point.
//!
//! Native HTTP service that runs on the node host next to zebra + zaino.
//! It holds the faucet seed and (once the wallet engine is wired, see
//! [`faucet_signer::wallet`]) builds, proves, and broadcasts transparent and
//! Orchard transactions. The Cloudflare Worker calls `/send` over a Cloudflare
//! Tunnel, authenticated with a shared bearer secret.

use std::sync::Arc;

use faucet_signer::config::Config;
use faucet_signer::wallet::Wallet;
use faucet_signer::{AppState, build_app};

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    let config = match Config::from_env() {
        Ok(c) => c,
        Err(e) => {
            tracing::error!("configuration error: {e}");
            std::process::exit(1);
        }
    };

    // Single-writer guard: hold an exclusive lock on the wallet DB so only one
    // signer ever touches it. If another process holds it, exit non-zero so the
    // orchestrator restarts us until the holder releases it. The lock is held
    // for the whole process lifetime and released on exit (see `dblock`).
    let _wallet_lock = match faucet_signer::dblock::acquire(&config.db_path) {
        Ok(lock) => {
            tracing::info!("acquired wallet lock ({})", lock.path());
            lock
        }
        Err(faucet_signer::dblock::LockError::Held(path)) => {
            tracing::error!("another signer holds the wallet lock ({path}); exiting to retry");
            std::process::exit(1);
        }
        Err(e) => {
            tracing::error!("{e}");
            std::process::exit(1);
        }
    };

    let bind = config.bind.clone();
    let state = Arc::new(AppState::new(
        Wallet::new(
            config.network,
            config.lightwalletd_url,
            config.db_path,
            config.birthday_height,
            config.seed,
        ),
        config.shared_secret,
        config.network,
    ));

    let listener = match tokio::net::TcpListener::bind(&bind).await {
        Ok(l) => l,
        Err(e) => {
            tracing::error!("failed to bind {bind}: {e}");
            std::process::exit(1);
        }
    };
    tracing::info!("faucet-signer listening on {bind}");
    if let Err(e) = axum::serve(listener, build_app(state)).await {
        tracing::error!("server error: {e}");
        std::process::exit(1);
    }
}
