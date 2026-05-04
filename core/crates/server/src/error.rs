//! Server error type with HTTP status mapping.
//!
//! [`ServerError`] is the single error type returned by all route handlers.
//! Its [`IntoResponse`] impl maps each variant to an HTTP status code.
//!
//! # Security note
//!
//! Error responses intentionally return generic messages ("not found",
//! "unauthorized") rather than leaking internal details. Database errors and
//! internal failures are logged server-side via `tracing::error` but the
//! client only sees "internal error".

use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};

#[derive(Debug, thiserror::Error)]
pub enum ServerError {
    #[error("database error: {0}")]
    Db(#[from] sqlx::Error),

    #[error("not found")]
    NotFound,

    #[error("unauthorized")]
    Unauthorized,

    #[error("bad request: {0}")]
    BadRequest(String),

    #[error("rate limited")]
    RateLimited,

    #[error("internal error: {0}")]
    Internal(String),
}

impl IntoResponse for ServerError {
    fn into_response(self) -> Response {
        let (status, message) = match &self {
            ServerError::Db(_) => (StatusCode::INTERNAL_SERVER_ERROR, "internal error"),
            ServerError::NotFound => (StatusCode::NOT_FOUND, "not found"),
            ServerError::Unauthorized => (StatusCode::UNAUTHORIZED, "unauthorized"),
            ServerError::BadRequest(_) => (StatusCode::BAD_REQUEST, "bad request"),
            ServerError::RateLimited => (StatusCode::TOO_MANY_REQUESTS, "rate limited"),
            ServerError::Internal(_) => (StatusCode::INTERNAL_SERVER_ERROR, "internal error"),
        };

        if matches!(self, ServerError::Db(_) | ServerError::Internal(_)) {
            tracing::error!(%self);
        }

        (status, message).into_response()
    }
}
