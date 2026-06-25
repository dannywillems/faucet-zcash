//! Faucet signer library: the HTTP app (router, state, handlers) so it can be
//! driven both by the binary (`main.rs`) and by integration tests.

pub mod config;
pub mod error;
pub mod wallet;

use std::sync::Arc;

use axum::extract::State;
use axum::http::HeaderMap;
use axum::routing::{get, post};
use axum::{Json, Router};
use faucet_core::{validate_destination, Network, SignerSendRequest, SignerSendResponse};
use zeroize::Zeroizing;

use crate::error::SignerError;
use crate::wallet::Wallet;

/// Shared application state.
pub struct AppState {
    pub wallet: Wallet,
    pub shared_secret: Zeroizing<String>,
    pub network: Network,
}

impl AppState {
    pub fn new(wallet: Wallet, shared_secret: Zeroizing<String>, network: Network) -> Self {
        Self {
            wallet,
            shared_secret,
            network,
        }
    }
}

/// Build the axum router with `/health` and the authenticated `/send`.
pub fn build_app(state: Arc<AppState>) -> Router {
    Router::new()
        .route("/health", get(|| async { "ok" }))
        .route("/send", post(handle_send))
        .with_state(state)
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
pub fn authorize(headers: &HeaderMap, expected: &str) -> Result<(), SignerError> {
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
