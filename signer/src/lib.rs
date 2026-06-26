//! Faucet signer library: the HTTP app (router, state, handlers) so it can be
//! driven both by the binary (`main.rs`) and by integration tests.

pub mod blockcache;
pub mod config;
pub mod dblock;
pub mod error;
pub mod wallet;

use std::sync::Arc;

use axum::extract::{Query, State};
use axum::http::HeaderMap;
use axum::routing::{get, post};
use axum::{Json, Router};
use faucet_core::{
    FaucetBalanceResponse, Network, SignerSendRequest, SignerSendResponse, validate_destination,
};
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
        .route("/info", get(handle_info))
        .route("/balance", get(handle_balance))
        .route("/shield", post(handle_shield))
        .route("/send", post(handle_send))
        .with_state(state)
}

/// Authenticated diagnostics: confirms the faucet account exists (offline) and
/// reports the chain tip from the configured zaino (live).
async fn handle_info(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
) -> Result<Json<serde_json::Value>, SignerError> {
    authorize(&headers, &state.shared_secret)?;
    let account = state.wallet.ensure_account()?;
    let chain_height = state.wallet.chain_height().await?;
    Ok(Json(serde_json::json!({
        "network": match state.network {
            Network::Testnet => "testnet",
            Network::Mainnet => "mainnet",
        },
        "account": format!("{account:?}"),
        "chain_height": chain_height,
    })))
}

/// Authenticated balance/diagnostics: syncs the faucet wallet and reports its
/// receiving address and per-pool balances. Useful for ops (faucet reserves)
/// and for diagnosing funding (compare `unified_address` to what was funded).
async fn handle_balance(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Query(params): Query<std::collections::HashMap<String, String>>,
) -> Result<Json<FaucetBalanceResponse>, SignerError> {
    authorize(&headers, &state.shared_secret)?;
    // `?sync=true` forces a chain sync first (slow); default is a fast read.
    let sync = matches!(params.get("sync").map(String::as_str), Some("true" | "1"));
    let s = state.wallet.summary(sync).await?;
    Ok(Json(FaucetBalanceResponse {
        unified_address: s.unified_address,
        chain_tip: s.chain_tip,
        fully_scanned: s.fully_scanned,
        transparent_total_zat: s.transparent_total_zat,
        orchard_spendable_zat: s.orchard_spendable_zat,
        orchard_total_zat: s.orchard_total_zat,
    }))
}

/// Authenticated maintenance: shield the faucet's transparent funds into
/// Orchard so they become spendable by `/send`. Returns the shielding txid.
async fn handle_shield(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
) -> Result<Json<serde_json::Value>, SignerError> {
    authorize(&headers, &state.shared_secret)?;
    let txid = state.wallet.shield().await?;
    Ok(Json(serde_json::json!({ "txid": txid })))
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
        .send(&req.address, req.amount_zat, req.pool, req.memo)
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
