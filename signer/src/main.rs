//! Faucet signer service entry point.
//!
//! Native HTTP service that runs on the node host next to zcashd + lightwalletd.
//! It holds the faucet seed and (once the wallet engine is wired, see
//! [`faucet_signer::wallet`]) builds, proves, and broadcasts transparent and
//! Orchard transactions. The Cloudflare Worker calls `/send` over a Cloudflare
//! Tunnel, authenticated with a shared bearer secret.

use std::sync::Arc;

use faucet_signer::config::Config;
use faucet_signer::wallet::Wallet;
use faucet_signer::{build_app, AppState};

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

    let bind = config.bind.clone();
    let state = Arc::new(AppState::new(
        Wallet::new(config.network, config.lightwalletd_url, config.seed),
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
