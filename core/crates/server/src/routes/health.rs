//! Liveness/readiness probe: `GET /healthz`.
//!
//! Public, unauthenticated. Returns 200 with body `ok` if the server can
//! acquire a database connection, 503 otherwise. Used by deploy scripts,
//! load balancers, and the post-update verification step in
//! `avalanche-update`.

use axum::{extract::State, http::StatusCode, routing::get, Router};

use crate::state::AppState;

pub fn routes() -> Router<AppState> {
    Router::new().route("/healthz", get(healthz))
}

async fn healthz(State(state): State<AppState>) -> (StatusCode, &'static str) {
    match state.db.acquire().await {
        Ok(_) => (StatusCode::OK, "ok"),
        Err(err) => {
            tracing::warn!(error = %err, "healthz: database unreachable");
            (StatusCode::SERVICE_UNAVAILABLE, "db unavailable")
        }
    }
}
