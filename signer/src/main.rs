//! Faucet signer service.
//!
//! Native HTTP service that runs on the node host next to zcashd + lightwalletd.
//! It holds the faucet seed and (once the wallet engine is wired, see
//! [`wallet`]) builds, proves, and broadcasts transparent and Orchard
//! transactions. The Cloudflare Worker calls `/send` over a Cloudflare Tunnel,
//! authenticated with a shared bearer secret.

mod config;
mod error;
mod wallet;

use std::sync::Arc;

use axum::extract::State;
use axum::http::HeaderMap;
use axum::routing::{get, post};
use axum::{Json, Router};
use faucet_core::{validate_destination, Network, SignerSendRequest, SignerSendResponse};
use zeroize::Zeroizing;

use crate::config::Config;
use crate::error::SignerError;
use crate::wallet::Wallet;

struct AppState {
    wallet: Wallet,
    shared_secret: Zeroizing<String>,
    network: Network,
}

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
    let state = Arc::new(AppState {
        wallet: Wallet::new(config.network, config.lightwalletd_url, config.seed),
        shared_secret: config.shared_secret,
        network: config.network,
    });

    let app = Router::new()
        .route("/health", get(|| async { "ok" }))
        .route("/send", post(handle_send))
        .with_state(state);

    let listener = match tokio::net::TcpListener::bind(&bind).await {
        Ok(l) => l,
        Err(e) => {
            tracing::error!("failed to bind {bind}: {e}");
            std::process::exit(1);
        }
    };
    tracing::info!("faucet-signer listening on {bind}");
    if let Err(e) = axum::serve(listener, app).await {
        tracing::error!("server error: {e}");
        std::process::exit(1);
    }
}

async fn handle_send(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(req): Json<SignerSendRequest>,
) -> Result<Json<SignerSendResponse>, SignerError> {
    authorize(&headers, &state.shared_secret)?;

    // Re-validate the destination on the signer too (defense in depth).
    validate_destination(&req.address, state.network)
        .map_err(|rejection| SignerError::BadRequest(rejection.message().to_string()))?;

    let txid = state
        .wallet
        .send(&req.address, req.amount_zat, req.pool)
        .await?;
    Ok(Json(SignerSendResponse { txid }))
}

/// Verify the `Authorization: Bearer <secret>` header against the shared secret.
fn authorize(headers: &HeaderMap, expected: &str) -> Result<(), SignerError> {
    let provided = headers
        .get("Authorization")
        .and_then(|h| h.to_str().ok())
        .and_then(|h| h.strip_prefix("Bearer "))
        .ok_or(SignerError::Unauthorized)?;
    if ct_eq(provided.as_bytes(), expected.as_bytes()) {
        Ok(())
    } else {
        Err(SignerError::Unauthorized)
    }
}

/// Constant-time byte comparison.
fn ct_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut diff = 0u8;
    for (x, y) in a.iter().zip(b.iter()) {
        diff |= x ^ y;
    }
    diff == 0
}
