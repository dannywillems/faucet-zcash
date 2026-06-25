//! Signer error type and its HTTP mapping. Error messages never include the
//! seed, the bearer token, or raw node responses.

use axum::Json;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};

#[derive(thiserror::Error, Debug)]
pub enum SignerError {
    #[error("unauthorized")]
    Unauthorized,
    #[error("{0}")]
    BadRequest(String),
    #[error("wallet engine not ready: {0}")]
    NotReady(String),
    // The signer is called only by the Worker over an authenticated internal
    // channel, so surfacing the detail here aids debugging; the Worker decides
    // what (if anything) to expose publicly.
    #[error("internal error: {0}")]
    Internal(String),
}

impl IntoResponse for SignerError {
    fn into_response(self) -> Response {
        let status = match self {
            SignerError::Unauthorized => StatusCode::UNAUTHORIZED,
            SignerError::BadRequest(_) => StatusCode::BAD_REQUEST,
            SignerError::NotReady(_) => StatusCode::SERVICE_UNAVAILABLE,
            SignerError::Internal(_) => StatusCode::INTERNAL_SERVER_ERROR,
        };
        (
            status,
            Json(serde_json::json!({ "error": self.to_string() })),
        )
            .into_response()
    }
}
