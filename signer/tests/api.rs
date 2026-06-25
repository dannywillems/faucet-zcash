//! End-to-end tests for the signer HTTP service. These drive the real router
//! (auth, address re-validation, routing, error mapping) and the offline wallet
//! engine (account derivation), without a live node.

use std::sync::Arc;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use faucet_core::Network;
use faucet_signer::wallet::Wallet;
use faucet_signer::{build_app, AppState};
use tower::util::ServiceExt;
use zeroize::Zeroizing;

const SECRET: &str = "test-shared-secret";
// Real testnet transparent address (from the zcash_address vectors).
const TESTNET_T: &str = "tm9iMLAuYMzJ6jtFLcA7rzUmfreGuKvr7Ma";
// A valid 32-byte (64 hex char) test seed.
const SEED_HEX: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";

fn app() -> axum::Router {
    // In-memory wallet DB so each request gets a fresh, side-effect-free engine.
    let wallet = Wallet::new(
        Network::Testnet,
        "http://127.0.0.1:8137".to_string(),
        ":memory:".to_string(),
        None,
        Zeroizing::new(SEED_HEX.to_string()),
    );
    let state = Arc::new(AppState::new(
        wallet,
        Zeroizing::new(SECRET.to_string()),
        Network::Testnet,
    ));
    build_app(state)
}

fn send_request(auth: Option<&str>, body: &str) -> Request<Body> {
    let mut builder = Request::builder()
        .method("POST")
        .uri("/send")
        .header("Content-Type", "application/json");
    if let Some(token) = auth {
        builder = builder.header("Authorization", format!("Bearer {token}"));
    }
    builder.body(Body::from(body.to_string())).unwrap()
}

#[tokio::test]
async fn health_ok() {
    let response = app()
        .oneshot(
            Request::builder()
                .uri("/health")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn send_without_auth_is_unauthorized() {
    let body =
        format!(r#"{{"address":"{TESTNET_T}","amount_zat":100000000,"pool":"transparent"}}"#);
    let response = app().oneshot(send_request(None, &body)).await.unwrap();
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn send_with_wrong_secret_is_unauthorized() {
    let body =
        format!(r#"{{"address":"{TESTNET_T}","amount_zat":100000000,"pool":"transparent"}}"#);
    let response = app()
        .oneshot(send_request(Some("wrong"), &body))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn send_with_invalid_address_is_bad_request() {
    let body = r#"{"address":"not-a-zcash-address","amount_zat":100000000,"pool":"transparent"}"#;
    let response = app()
        .oneshot(send_request(Some(SECRET), body))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn send_with_mainnet_address_is_bad_request() {
    // A mainnet transparent address must be rejected on the testnet faucet.
    let body =
        r#"{"address":"t1Hsc1LR8yKnbbe3twRp88p6vFfC5t7DLbs","amount_zat":1,"pool":"transparent"}"#;
    let response = app()
        .oneshot(send_request(Some(SECRET), body))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[test]
fn ensure_account_creates_and_persists() {
    // Use a temp file DB so the account persists across two opens (idempotent).
    let dir = std::env::temp_dir();
    let path = dir.join(format!("faucet-signer-test-{}.sqlite", std::process::id()));
    let _ = std::fs::remove_file(&path);
    let wallet = Wallet::new(
        Network::Testnet,
        "http://127.0.0.1:8137".to_string(),
        path.to_string_lossy().to_string(),
        None,
        Zeroizing::new(SEED_HEX.to_string()),
    );

    let first = wallet.ensure_account().expect("create account");
    let second = wallet.ensure_account().expect("reopen account");
    assert_eq!(first, second, "the faucet account should persist");

    let _ = std::fs::remove_file(&path);
}

// Note: a fully authorized + valid /send now drives the real engine (sync +
// prove + broadcast against zaino), which requires the live local node and a
// funded wallet, so it is verified manually rather than in CI.
