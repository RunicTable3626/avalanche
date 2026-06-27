//! Device-linking provisioning mailbox (docs/04-multi-device.md §4).
//!
//! A short-lived, ciphertext-only rendezvous between an existing device and a
//! new device joining the same identity. The server is pure transport: it
//! stores opaque blobs in named slots and forwards them, never learning the
//! shared key. Sessions are unauthenticated and expire after a few minutes.

use sqlx::{PgConnection, Row};
use time::OffsetDateTime;

/// Create a provisioning session with the given opaque id, expiring after
/// `lifetime_secs`. Returns the assigned `expires_at`.
pub async fn create_session(
    conn: &mut PgConnection,
    id: &str,
    lifetime_secs: i64,
) -> Result<OffsetDateTime, sqlx::Error> {
    let row = sqlx::query(
        "INSERT INTO provisioning_sessions (id, expires_at)
         VALUES ($1, now() + make_interval(secs => $2))
         RETURNING expires_at",
    )
    .bind(id)
    .bind(lifetime_secs as f64)
    .fetch_one(&mut *conn)
    .await?;
    Ok(row.get("expires_at"))
}

/// Upsert a slot's ciphertext for a live (non-expired) session. Returns `false`
/// if the session does not exist or has expired (caller maps to 404).
pub async fn put_slot(
    conn: &mut PgConnection,
    session_id: &str,
    slot: &str,
    ciphertext: &[u8],
) -> Result<bool, sqlx::Error> {
    // Guard the write on a live session in a single statement so an expired or
    // missing session can never receive a slot.
    let result = sqlx::query(
        "INSERT INTO provisioning_slots (session_id, slot, ciphertext, byte_len, updated_at)
         SELECT $1, $2, $3, $4, now()
         FROM provisioning_sessions
         WHERE id = $1 AND expires_at > now()
         ON CONFLICT (session_id, slot)
         DO UPDATE SET ciphertext = EXCLUDED.ciphertext,
                       byte_len   = EXCLUDED.byte_len,
                       updated_at = now()",
    )
    .bind(session_id)
    .bind(slot)
    .bind(ciphertext)
    .bind(ciphertext.len() as i32)
    .execute(&mut *conn)
    .await?;
    Ok(result.rows_affected() > 0)
}

/// Fetch a slot's ciphertext, but only while its session is live. Returns
/// `None` if the session is missing/expired or the slot has not been written.
pub async fn get_slot(
    conn: &mut PgConnection,
    session_id: &str,
    slot: &str,
) -> Result<Option<Vec<u8>>, sqlx::Error> {
    let row = sqlx::query(
        "SELECT s.ciphertext
         FROM provisioning_slots s
         JOIN provisioning_sessions ps ON ps.id = s.session_id
         WHERE s.session_id = $1 AND s.slot = $2 AND ps.expires_at > now()",
    )
    .bind(session_id)
    .bind(slot)
    .fetch_optional(&mut *conn)
    .await?;
    Ok(row.map(|r| r.get::<Vec<u8>, _>("ciphertext")))
}

/// Delete expired provisioning sessions (slots cascade). Housekeeping.
pub async fn delete_expired(conn: &mut PgConnection) -> Result<u64, sqlx::Error> {
    let result = sqlx::query("DELETE FROM provisioning_sessions WHERE expires_at < now()")
        .execute(&mut *conn)
        .await?;
    Ok(result.rows_affected())
}
