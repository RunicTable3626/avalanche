//! Invite code endpoints: create, inspect, and redeem invite codes.
//!
//! - `POST /v1/invites` — create a new invite code (authenticated).
//! - `GET /v1/invites/:code` — inspect an invite code (unauthenticated).
//! - `POST /v1/invites/:code/redeem` — redeem an invite code (authenticated).

use axum::http::StatusCode;
use axum::{
    extract::{Path, State},
    routing::{get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use sqlx::Row;
use time::OffsetDateTime;

use crate::{db, error::ServerError, middleware::auth::AuthDevice, state::AppState};

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/v1/invites", post(create_invite))
        .route("/v1/invites/{code}", get(get_invite))
        .route("/v1/invites/{code}/redeem", post(redeem_invite))
}

// ── POST /v1/invites ─────────────────────────────────────────────────────────

#[derive(Deserialize)]
struct CreateInviteRequest {
    target_type: String,
    target_id: String,
    expires_at: Option<String>,
}

#[derive(Serialize)]
struct InviteResponse {
    code: String,
    expires_at: Option<String>,
}

async fn create_invite(
    State(state): State<AppState>,
    auth: AuthDevice,
    Json(req): Json<CreateInviteRequest>,
) -> Result<(StatusCode, Json<InviteResponse>), ServerError> {
    let mut conn = state.db.acquire().await?;

    let device = db::devices::find_by_pk(&mut conn, auth.device_pk)
        .await?
        .ok_or_else(|| ServerError::Internal("authenticated device not found".into()))?;

    let expires_at = match req.expires_at {
        Some(s) => Some(
            OffsetDateTime::parse(&s, &time::format_description::well_known::Rfc3339)
                .map_err(|e| ServerError::BadRequest(e.to_string()))?,
        ),
        None => None,
    };

    let code = {
        use base64::prelude::*;
        use rand::Rng;
        let bytes: [u8; 16] = rand::rng().random();
        BASE64_URL_SAFE_NO_PAD.encode(bytes)
    };

    db::invites::create(
        &mut conn,
        &code,
        device.account_id,
        &req.target_type,
        &req.target_id,
        expires_at,
    )
    .await?;

    Ok((
        StatusCode::CREATED,
        Json(InviteResponse {
            code,
            expires_at: expires_at.map(|dt| dt.to_string()),
        }),
    ))
}

// ── GET /v1/invites/:code ────────────────────────────────────────────────────

#[derive(Serialize)]
struct InviteDetailResponse {
    code: String,
    created_by: String,
    target_type: String,
    target_id: String,
    expires_at: Option<String>,
    used_by: Option<i64>,
    used_at: Option<String>,
}

async fn get_invite(
    State(state): State<AppState>,
    Path(code): Path<String>,
) -> Result<Json<InviteDetailResponse>, ServerError> {
    let mut conn = state.db.acquire().await?;

    let invite = db::invites::find_by_code(&mut conn, &code)
        .await?
        .ok_or(ServerError::NotFound)?;

    let created_by: String = sqlx::query("SELECT did FROM accounts WHERE id = $1")
        .bind(invite.created_by_account_id)
        .fetch_optional(&mut *conn)
        .await?
        .map(|row| row.get("did"))
        .unwrap_or_default();

    Ok(Json(InviteDetailResponse {
        code: invite.code,
        created_by,
        target_type: invite.target_type,
        target_id: invite.target_id,
        expires_at: invite.expires_at.map(|dt| dt.to_string()),
        used_by: invite.used_by_account_id,
        used_at: invite.used_at.map(|dt| dt.to_string()),
    }))
}

// ── POST /v1/invites/:code/redeem ────────────────────────────────────────────

#[derive(Serialize)]
struct RedeemInviteResponse {
    code: String,
    target_type: String,
    target_id: String,
}

async fn redeem_invite(
    State(state): State<AppState>,
    auth: AuthDevice,
    Path(code): Path<String>,
) -> Result<Json<RedeemInviteResponse>, ServerError> {
    let mut conn = state.db.acquire().await?;

    let device = db::devices::find_by_pk(&mut conn, auth.device_pk)
        .await?
        .ok_or_else(|| ServerError::Internal("authenticated device not found".into()))?;

    let redeemed = db::invites::redeem(&mut conn, &code, device.account_id).await?;

    if !redeemed {
        let exists = db::invites::find_by_code(&mut conn, &code).await?;
        if exists.is_none() {
            return Err(ServerError::NotFound);
        }
        return Err(ServerError::BadRequest("already redeemed".into()));
    }

    let invite = db::invites::find_by_code(&mut conn, &code)
        .await?
        .ok_or(ServerError::NotFound)?;

    Ok(Json(RedeemInviteResponse {
        code: invite.code,
        target_type: invite.target_type,
        target_id: invite.target_id,
    }))
}
