//! DID document resolution: `GET /.well-known/did/{did}`.
//!
//! Public, unauthenticated endpoint. Returns the DID document for a locally
//! hosted account. This is a stub for the full `did:plc` resolution flow
//! that will involve the PLC directory in Stage 9 (Federation).
//!
//! # Security note
//!
//! DID documents are intentionally public. They contain only the account's
//! public verification keys and service endpoints — no private information.

use axum::{
    extract::{Path, State},
    routing::get,
    Json, Router,
};

use crate::{db, error::ServerError, state::AppState};

pub fn routes() -> Router<AppState> {
    Router::new().route("/.well-known/did/{did}", get(resolve))
}

async fn resolve(
    State(state): State<AppState>,
    Path(did): Path<String>,
) -> Result<Json<serde_json::Value>, ServerError> {
    let mut conn = state.db.acquire().await?;
    let doc = db::did::find_by_did(&mut conn, &did)
        .await?
        .ok_or(ServerError::NotFound)?;
    Ok(Json(doc))
}
