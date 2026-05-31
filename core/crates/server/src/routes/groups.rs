//! Public group-related endpoints.
//!
//! Stage 5 adds only one endpoint so far: publishing the homeserver's
//! zkgroup public params. Clients fetch this once and cache it; the bytes
//! are stable per `ServerSecretParams` generation (rotation is deferred).
//!
//! Unauthenticated by design — the public params are exactly that.

use axum::{extract::State, routing::get, Json, Router};
use base64::Engine as _;
use serde::Serialize;

use crate::db;
use crate::state::AppState;

pub fn routes() -> Router<AppState> {
    Router::new().route("/v1/groups/server-params", get(get_server_params))
}

#[derive(Serialize)]
struct ServerParamsResponse {
    /// Schema version of the params blob — matches the row pinned in the DB.
    version: i32,
    /// Base64-encoded `ServerPublicParams` bytes.
    params: String,
}

async fn get_server_params(
    State(state): State<AppState>,
) -> Json<ServerParamsResponse> {
    let bytes = state.zkgroup_secret.public_params().to_bytes();
    Json(ServerParamsResponse {
        version: db::zkgroup_params::CURRENT_VERSION,
        params: base64::engine::general_purpose::STANDARD.encode(&bytes),
    })
}
