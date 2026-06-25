//! Signer error type and its HTTP mapping. Error messages never include the
//! seed, the bearer token, or raw node responses.

use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;

#[derive(thiserror::Error, Debug)]
pub enum SignerError {
    #[error("unauthorized")]
    Unauthorized,
    #[error("{0}")]
    BadRequest(String),
    #[error("wallet engine not ready: {0}")]
    NotReady(String),
}

impl IntoResponse for SignerError {
    fn into_response(self) -> Response {
        let status = match self {
            SignerError::Unauthorized => StatusCode::UNAUTHORIZED,
            SignerError::BadRequest(_) => StatusCode::BAD_REQUEST,
            SignerError::NotReady(_) => StatusCode::SERVICE_UNAVAILABLE,
        };
        (
            status,
            Json(serde_json::json!({ "error": self.to_string() })),
        )
            .into_response()
    }
}
