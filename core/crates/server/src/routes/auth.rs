//! Device authentication: `POST /v1/auth/token`.
//!
//! Issues a new session token for an existing device. The client provides
//! its DID and device_id; the server looks up the device and returns a
//! time-limited bearer token.
//!
//! # Security notes
//!
//! - **Identity key signature verification is not yet implemented.** In the
//!   full protocol, the client would sign a nonce with its identity key's
//!   private half, and the server would verify against the stored public key.
//!   Without this, anyone who knows a DID and device_id can obtain a token.
//!   This is a Stage 3 TODO — acceptable for local development but must be
//!   implemented before any real deployment.
//! - Tokens are 256-bit random strings (not JWTs), revocable by deletion.

use axum::{extract::State, routing::post, Json, Router};
use serde::{Deserialize, Serialize};

use crate::{db, error::ServerError, state::AppState};

pub fn routes() -> Router<AppState> {
    Router::new().route("/v1/auth/token", post(issue_token))
}

#[derive(Deserialize)]
struct TokenRequest {
    did: String,
    device_id: i32,
    // In a full implementation this would include a signature over a nonce
    // to prove possession of the identity key. For now we trust the caller
    // can identify the correct (did, device_id) pair.
    // TODO(stage3): add identity_key_signature verification.
}

#[derive(Serialize)]
struct TokenResponse {
    session_token: String,
    expires_at: String,
}

async fn issue_token(
    State(state): State<AppState>,
    Json(req): Json<TokenRequest>,
) -> Result<Json<TokenResponse>, ServerError> {
    let mut conn = state.db.acquire().await?;

    let account = db::accounts::find_by_did(&mut conn, &req.did)
        .await?
        .ok_or(ServerError::NotFound)?;

    let device = db::devices::find(&mut conn, account.id, req.device_id)
        .await?
        .ok_or(ServerError::NotFound)?;

    let token = {
        use base64::prelude::*;
        use rand::Rng;
        let bytes: [u8; 32] = rand::rng().random();
        BASE64_URL_SAFE_NO_PAD.encode(bytes)
    };

    let expires_at =
        db::sessions::create(&mut conn, &token, device.id, state.config.token_lifetime_secs)
            .await?;

    Ok(Json(TokenResponse {
        session_token: token,
        expires_at: expires_at.to_string(),
    }))
}
