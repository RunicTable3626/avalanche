//! End-to-end-encrypted attachment endpoints (docs/35-attachments.md):
//!
//! - `POST   /v1/attachments`      — allocate an upload slot (size cap + quota).
//! - `PUT    /v1/attachments/{id}` — upload the ciphertext (owner only).
//! - `GET    /v1/attachments/{id}` — download the ciphertext (UNauthenticated).
//! - `DELETE /v1/attachments/{id}` — best-effort delete (owner only; TTL GC is
//!   authoritative).
//!
//! The server stores only opaque ciphertext addressed by an opaque
//! `attachment_id`; it never sees the key or plaintext. **Download is
//! unauthenticated**: the unguessable, opaque id — which only exists inside an
//! E2E message — *is* the capability, exactly like Signal's CDN and an S3
//! presigned URL. (Auth would add almost nothing: the id is a random UUID, not
//! a guessable DID, so it can't probe membership; and the bytes are E2E
//! ciphertext regardless. Leaving it open also lets a cross-server recipient
//! fetch from the sender's homeserver without holding credentials there, and
//! lets a future S3 backend put the presigned object URL straight in the
//! pointer with no homeserver hop.) Allocate and upload stay authenticated —
//! those need an owning account for the size cap, the per-account byte quota,
//! and the upload owner-check.

use axum::{
    body::{Body, Bytes},
    extract::{Path, State},
    http::{header, HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

use crate::{db, error::ServerError, middleware::auth::AuthDevice, state::AppState};

pub fn routes() -> Router<AppState> {
    // The upload body is the raw ciphertext, far larger than a JSON request, so
    // the PUT route gets its own generous body limit (the per-attachment cap is
    // also enforced in the handler against the declared size). 110 MB leaves
    // headroom over the 100 MB default cap.
    Router::new()
        .route("/v1/attachments", post(allocate))
        .route(
            "/v1/attachments/{attachment_id}",
            get(download).put(upload).delete(delete_attachment).layer(
                axum::extract::DefaultBodyLimit::max(110 * 1024 * 1024),
            ),
        )
}

async fn account_id_for(state: &AppState, auth: &AuthDevice) -> Result<i64, ServerError> {
    let mut conn = state.db.acquire().await?;
    let device = db::devices::find_by_pk(&mut conn, auth.device_pk)
        .await?
        .ok_or(ServerError::Internal("device not found for session".into()))?;
    Ok(device.account_id)
}

// ── POST /v1/attachments ─────────────────────────────────────────────────────

#[derive(Deserialize)]
struct AllocateRequest {
    /// Ciphertext size in bytes — checked against the cap and the rolling quota.
    size_bytes: i64,
}

/// Where and how the client should PUT the ciphertext. The client replays this
/// verbatim and stays backend-blind: for LocalFs `url` is this homeserver's own
/// route and `headers` carry the bearer; a future S3 backend returns a presigned
/// PUT URL + signed headers here instead, with no client change (docs/35).
#[derive(Serialize)]
struct UploadDescriptor {
    url: String,
    method: String,
    headers: Vec<(String, String)>,
}

#[derive(Serialize)]
struct AllocateResponse {
    attachment_id: String,
    /// Where/how to upload the ciphertext.
    upload: UploadDescriptor,
    /// Absolute, stable URL for the E2E pointer; recipients GET it
    /// (unauthenticated). For LocalFs this is the same route as `upload.url`
    /// (different method); a future S3 backend points it at the homeserver's
    /// redirect route (not a short-lived presigned URL, which couldn't survive
    /// the blob's multi-week TTL).
    download_url: String,
    /// Unix-millis blob TTL deadline.
    expires_at_ms: i64,
}

async fn allocate(
    State(state): State<AppState>,
    auth: AuthDevice,
    headers: HeaderMap,
    Json(req): Json<AllocateRequest>,
) -> Result<(StatusCode, Json<AllocateResponse>), ServerError> {
    if req.size_bytes <= 0 {
        return Err(ServerError::BadRequest("size_bytes must be positive".into()));
    }
    if req.size_bytes > state.config.attachment_max_size_bytes {
        return Err(ServerError::BadRequest("attachment too large".into()));
    }

    let account_id = account_id_for(&state, &auth).await?;
    let mut conn = state.db.acquire().await?;

    // Request-rate limit on allocation.
    if !db::rate_limits::check_and_increment(
        &mut conn,
        account_id,
        crate::middleware::rate_limit::ACTION_ATTACHMENT_ALLOCATE,
        crate::middleware::rate_limit::LIMIT_ATTACHMENT_ALLOCATE,
        crate::middleware::rate_limit::WINDOW_ATTACHMENT_ALLOCATE,
    )
    .await?
    {
        return Err(ServerError::RateLimited);
    }

    // Rolling bytes-per-hour quota: a single account can't fill the disk.
    let used = db::attachments::bytes_uploaded_since(&mut conn, account_id, 3600).await?;
    if used + req.size_bytes > state.config.attachment_bytes_per_hour {
        return Err(ServerError::RateLimited);
    }

    let attachment_id = uuid::Uuid::new_v4().to_string();
    let expires_at = OffsetDateTime::now_utc()
        + time::Duration::seconds(state.config.attachment_blob_ttl_secs);
    db::attachments::insert(&mut conn, &attachment_id, account_id, req.size_bytes, expires_at)
        .await?;

    let base = state.config.server_url.trim_end_matches('/');
    let url = format!("{base}/v1/attachments/{attachment_id}");
    // Echo the caller's bearer so the client replays it verbatim on the PUT
    // (LocalFs upload is authenticated by the session token, same as before —
    // it just arrives via the descriptor rather than the client adding it). The
    // client already holds this token; the redundancy goes away when the upload
    // path later moves to a scoped/presigned token, a server-only change.
    let mut upload_headers = vec![(
        "content-type".to_string(),
        "application/octet-stream".to_string(),
    )];
    if let Some(authz) = headers
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
    {
        upload_headers.push(("authorization".to_string(), authz.to_string()));
    }

    Ok((
        StatusCode::CREATED,
        Json(AllocateResponse {
            attachment_id,
            upload: UploadDescriptor {
                url: url.clone(),
                method: "PUT".to_string(),
                headers: upload_headers,
            },
            download_url: url,
            expires_at_ms: expires_at.unix_timestamp() * 1000,
        }),
    ))
}

// ── PUT /v1/attachments/{id} ─────────────────────────────────────────────────

async fn upload(
    State(state): State<AppState>,
    auth: AuthDevice,
    Path(attachment_id): Path<String>,
    body: Bytes,
) -> Result<StatusCode, ServerError> {
    let account_id = account_id_for(&state, &auth).await?;
    let mut conn = state.db.acquire().await?;

    let att = db::attachments::get(&mut conn, &attachment_id)
        .await?
        .ok_or(ServerError::NotFound)?;
    // Only the allocator may upload to a slot.
    if att.account_id != account_id {
        return Err(ServerError::Unauthorized);
    }
    // Uploaded bytes must not exceed the declared (and already quota-checked)
    // size, nor the cap.
    if (body.len() as i64) > att.size_bytes
        || (body.len() as i64) > state.config.attachment_max_size_bytes
    {
        return Err(ServerError::BadRequest("uploaded blob exceeds declared size".into()));
    }

    state.blob_store.put(&attachment_id, &body).await?;
    Ok(StatusCode::NO_CONTENT)
}

// ── GET /v1/attachments/{id} ─────────────────────────────────────────────────

async fn download(
    State(state): State<AppState>,
    Path(attachment_id): Path<String>,
    headers: HeaderMap,
) -> Result<Response, ServerError> {
    let blob = state
        .blob_store
        .get(&attachment_id)
        .await?
        .ok_or(ServerError::NotFound)?;

    // Minimal single-range support so large media can stream / resume.
    if let Some(range) = headers.get(header::RANGE).and_then(|v| v.to_str().ok()) {
        if let Some((start, end)) = parse_range(range, blob.len()) {
            let slice = blob[start..=end].to_vec();
            let content_range = format!("bytes {start}-{end}/{}", blob.len());
            return Ok((
                StatusCode::PARTIAL_CONTENT,
                [
                    (header::CONTENT_TYPE, "application/octet-stream".to_string()),
                    (header::CONTENT_RANGE, content_range),
                    (header::ACCEPT_RANGES, "bytes".to_string()),
                ],
                Body::from(slice),
            )
                .into_response());
        }
    }

    Ok((
        StatusCode::OK,
        [
            (header::CONTENT_TYPE, "application/octet-stream".to_string()),
            (header::ACCEPT_RANGES, "bytes".to_string()),
        ],
        Body::from(blob),
    )
        .into_response())
}

/// Parse a single `bytes=start-end` range against `len`. Returns inclusive
/// `(start, end)` byte indices, or `None` for unsupported / malformed ranges
/// (caller falls back to a full 200 response).
fn parse_range(header: &str, len: usize) -> Option<(usize, usize)> {
    if len == 0 {
        return None;
    }
    let spec = header.strip_prefix("bytes=")?;
    // Only a single range is supported.
    if spec.contains(',') {
        return None;
    }
    let (start_s, end_s) = spec.split_once('-')?;
    let last = len - 1;
    match (start_s.trim(), end_s.trim()) {
        ("", "") => None,
        // suffix range: last N bytes
        ("", n) => {
            let n: usize = n.parse().ok()?;
            if n == 0 {
                return None;
            }
            Some((len.saturating_sub(n), last))
        }
        (s, "") => {
            let start: usize = s.parse().ok()?;
            if start > last {
                return None;
            }
            Some((start, last))
        }
        (s, e) => {
            let start: usize = s.parse().ok()?;
            let end: usize = e.parse().ok()?;
            if start > end || start > last {
                return None;
            }
            Some((start, end.min(last)))
        }
    }
}

// ── DELETE /v1/attachments/{id} ──────────────────────────────────────────────

async fn delete_attachment(
    State(state): State<AppState>,
    auth: AuthDevice,
    Path(attachment_id): Path<String>,
) -> Result<StatusCode, ServerError> {
    let account_id = account_id_for(&state, &auth).await?;
    let mut conn = state.db.acquire().await?;

    // Deleting is best-effort and owner-gated; a missing row is success
    // (idempotent — the TTL GC may have already removed it).
    if let Some(att) = db::attachments::get(&mut conn, &attachment_id).await? {
        if att.account_id != account_id {
            return Err(ServerError::Unauthorized);
        }
        db::attachments::delete(&mut conn, &attachment_id).await?;
        state.blob_store.delete(&attachment_id).await?;
    }
    Ok(StatusCode::NO_CONTENT)
}

#[cfg(test)]
mod tests {
    use super::parse_range;

    #[test]
    fn range_parsing() {
        assert_eq!(parse_range("bytes=0-99", 1000), Some((0, 99)));
        assert_eq!(parse_range("bytes=100-", 1000), Some((100, 999)));
        assert_eq!(parse_range("bytes=-50", 1000), Some((950, 999)));
        // end past EOF clamps to last byte
        assert_eq!(parse_range("bytes=0-100000", 1000), Some((0, 999)));
        // malformed / unsupported → None (full response)
        assert_eq!(parse_range("bytes=abc", 1000), None);
        assert_eq!(parse_range("bytes=0-1,2-3", 1000), None);
        assert_eq!(parse_range("bytes=2000-3000", 1000), None);
        assert_eq!(parse_range("bytes=5-2", 1000), None);
    }
}
