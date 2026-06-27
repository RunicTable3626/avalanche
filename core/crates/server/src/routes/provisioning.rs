//! Device-linking provisioning mailbox: `POST /v1/provisioning/sessions`,
//! `PUT /v1/provisioning/{id}/{slot}`, `GET /v1/provisioning/{id}/{slot}`
//! (docs/04-multi-device.md §4).
//!
//! A short-lived, ciphertext-only rendezvous between an existing device and a
//! new device joining the same identity. The server is pure transport: it
//! stores opaque blobs in two named slots (`handshake`, `bundle`) and forwards
//! them, never learning the shared key (derived from ephemeral X25519 keypairs
//! exchanged out-of-band via the pairing code). All endpoints are
//! **unauthenticated** — the new device has no account yet — and rate-limited
//! per IP. Sessions expire after a few minutes (nothing durable lives here).

use axum::{
    extract::{Path, State},
    routing::{post, put},
    Json, Router,
};
use base64::prelude::*;
use serde::{Deserialize, Serialize};

use crate::{db, error::ServerError, middleware::client_ip::ClientIp, state::AppState};

/// Lifetime of a provisioning session. A link should complete in seconds; this
/// leaves comfortable headroom for the user to scan/paste and approve.
const SESSION_LIFETIME_SECS: i64 = 300;

/// Maximum bytes stored in any one slot. The sealed bundle is a small keyring
/// (identity key + rotation key + storage key + routing); 16 KiB is far above
/// that and stops a slot from becoming a file.
const MAX_SLOT_BYTES: usize = 16 * 1024;

/// The only valid slot names.
const SLOT_HANDSHAKE: &str = "handshake";
const SLOT_BUNDLE: &str = "bundle";

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/v1/provisioning/sessions", post(create_session))
        .route("/v1/provisioning/{id}/{slot}", put(put_slot).get(get_slot))
}

fn valid_slot(slot: &str) -> bool {
    slot == SLOT_HANDSHAKE || slot == SLOT_BUNDLE
}

// ── POST /v1/provisioning/sessions ──────────────────────────────────────────

#[derive(Serialize)]
struct CreateSessionResponse {
    session_id: String,
    expires_at: String,
}

async fn create_session(
    State(state): State<AppState>,
    ClientIp(ip): ClientIp,
) -> Result<Json<CreateSessionResponse>, ServerError> {
    let mut conn = state.db.acquire().await?;

    if !db::ip_rate_limits::check_and_increment(
        &mut conn,
        &ip,
        crate::middleware::rate_limit::ACTION_PROVISIONING_CREATE,
        crate::middleware::rate_limit::LIMIT_PROVISIONING_CREATE,
        crate::middleware::rate_limit::WINDOW_PROVISIONING_CREATE,
    )
    .await?
    {
        return Err(ServerError::RateLimited);
    }

    let session_id = {
        use rand::Rng;
        let bytes: [u8; 32] = rand::rng().random();
        BASE64_URL_SAFE_NO_PAD.encode(bytes)
    };
    let expires_at =
        db::provisioning::create_session(&mut conn, &session_id, SESSION_LIFETIME_SECS).await?;

    Ok(Json(CreateSessionResponse {
        session_id,
        expires_at: expires_at.to_string(),
    }))
}

// ── PUT /v1/provisioning/{id}/{slot} ────────────────────────────────────────

#[derive(Deserialize)]
struct PutSlotRequest {
    ciphertext: String, // base64
}

async fn put_slot(
    State(state): State<AppState>,
    ClientIp(ip): ClientIp,
    Path((session_id, slot)): Path<(String, String)>,
    Json(req): Json<PutSlotRequest>,
) -> Result<axum::http::StatusCode, ServerError> {
    if !valid_slot(&slot) {
        return Err(ServerError::BadRequest("unknown provisioning slot".into()));
    }

    let mut conn = state.db.acquire().await?;

    if !db::ip_rate_limits::check_and_increment(
        &mut conn,
        &ip,
        crate::middleware::rate_limit::ACTION_PROVISIONING_PUT,
        crate::middleware::rate_limit::LIMIT_PROVISIONING_PUT,
        crate::middleware::rate_limit::WINDOW_PROVISIONING_PUT,
    )
    .await?
    {
        return Err(ServerError::RateLimited);
    }

    let ciphertext = BASE64_STANDARD
        .decode(&req.ciphertext)
        .map_err(|_| ServerError::BadRequest("invalid base64 ciphertext".into()))?;
    if ciphertext.len() > MAX_SLOT_BYTES {
        return Err(ServerError::BadRequest("provisioning slot too large".into()));
    }

    let stored = db::provisioning::put_slot(&mut conn, &session_id, &slot, &ciphertext).await?;
    if !stored {
        // Session missing or expired.
        return Err(ServerError::NotFound);
    }
    Ok(axum::http::StatusCode::NO_CONTENT)
}

// ── GET /v1/provisioning/{id}/{slot} ────────────────────────────────────────

#[derive(Serialize)]
struct GetSlotResponse {
    ciphertext: String, // base64
}

async fn get_slot(
    State(state): State<AppState>,
    ClientIp(ip): ClientIp,
    Path((session_id, slot)): Path<(String, String)>,
) -> Result<Json<GetSlotResponse>, ServerError> {
    if !valid_slot(&slot) {
        return Err(ServerError::BadRequest("unknown provisioning slot".into()));
    }

    let mut conn = state.db.acquire().await?;

    if !db::ip_rate_limits::check_and_increment(
        &mut conn,
        &ip,
        crate::middleware::rate_limit::ACTION_PROVISIONING_GET,
        crate::middleware::rate_limit::LIMIT_PROVISIONING_GET,
        crate::middleware::rate_limit::WINDOW_PROVISIONING_GET,
    )
    .await?
    {
        return Err(ServerError::RateLimited);
    }

    let ciphertext = db::provisioning::get_slot(&mut conn, &session_id, &slot)
        .await?
        .ok_or(ServerError::NotFound)?;
    Ok(Json(GetSlotResponse {
        ciphertext: BASE64_STANDARD.encode(ciphertext),
    }))
}
