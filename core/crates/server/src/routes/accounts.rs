//! Account info: `GET /v1/accounts/{did}`.
//!
//! Returns the public metadata for an account — display name and bot flag.
//! Requires authentication so the endpoint cannot be used for unauthenticated
//! account enumeration.
//!
//! **Note:** `display_name` is only populated for bot accounts. Human display
//! names are exchanged via encrypted profile bundles (client-to-client) and
//! are never stored on the server. Clients should use this endpoint to look up
//! bot names, not human names.

use axum::{extract::{Path, State}, routing::{get, put}, Json, Router};
use base64::prelude::*;
use serde::{Deserialize, Serialize};

use crate::{db, error::ServerError, middleware::auth::AuthDevice, state::AppState};

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/v1/accounts/{did}", get(get_account_info))
        .route("/v1/accounts/me/profile", put(put_profile_blob))
}

#[derive(Serialize)]
struct AccountInfoResponse {
    did: String,
    display_name: Option<String>,
    is_bot: bool,
    profile_blob: Option<String>,
}

#[derive(Deserialize)]
struct ProfileUpdateRequest {
    profile_blob: String,
}

async fn get_account_info(
    State(state): State<AppState>,
    _auth: AuthDevice,
    Path(did): Path<String>,
) -> Result<Json<AccountInfoResponse>, ServerError> {
    let mut conn = state.db.acquire().await?;
    let account = db::accounts::find_by_did(&mut conn, &did)
        .await?
        .ok_or(ServerError::NotFound)?;
    Ok(Json(AccountInfoResponse {
        did: account.did,
        display_name: account.display_name,
        is_bot: account.is_bot,
        profile_blob: account.profile_blob.map(|b| BASE64_STANDARD.encode(b)),
    }))
}

async fn put_profile_blob(
    State(state): State<AppState>,
    auth: AuthDevice,
    Json(req): Json<ProfileUpdateRequest>,
) -> Result<Json<serde_json::Value>, ServerError> {
    let mut conn = state.db.acquire().await?;

    let device = db::devices::find_by_pk(&mut conn, auth.device_pk)
        .await?
        .ok_or_else(|| ServerError::Internal("authenticated device not found".into()))?;

    let blob = BASE64_STANDARD
        .decode(&req.profile_blob)
        .map_err(|e| ServerError::BadRequest(e.to_string()))?;

    db::accounts::update_profile_blob(&mut conn, device.account_id, &blob).await?;

    Ok(Json(serde_json::json!({})))
}
