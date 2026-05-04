//! Session token management: issue, validate, and expire.
//!
//! Session tokens are opaque 256-bit random strings stored in the database.
//! They are short-lived (default 24h) and tied to a specific device. Every
//! authenticated API call and WebSocket connection requires a valid token.
//!
//! # Security notes
//!
//! - Tokens are generated from `OsRng` (256 bits of entropy), making brute
//!   force infeasible.
//! - Tokens are compared via database lookup, not cryptographic verification
//!   (they are not JWTs). This means they are trivially revocable: delete
//!   the row.
//! - A background task periodically deletes expired tokens so the table
//!   doesn't grow unbounded.
//! - Token lifetime is configurable but defaults to 24 hours. Clients must
//!   refresh before expiry.

use sqlx::{PgConnection, Row};
use time::OffsetDateTime;

/// Issue a new session token for a device.
pub async fn create(
    conn: &mut PgConnection,
    token: &str,
    device_pk: i64,
    lifetime_secs: i64,
) -> Result<OffsetDateTime, sqlx::Error> {
    let row = sqlx::query(
        "INSERT INTO session_tokens (token, device_pk, expires_at)
         VALUES ($1, $2, now() + make_interval(secs => $3))
         RETURNING expires_at",
    )
    .bind(token)
    .bind(device_pk)
    .bind(lifetime_secs as f64)
    .fetch_one(&mut *conn)
    .await?;
    Ok(row.get("expires_at"))
}

/// Validate a session token. Returns the device PK if valid and not expired.
pub async fn validate(conn: &mut PgConnection, token: &str) -> Result<Option<i64>, sqlx::Error> {
    let row = sqlx::query(
        "SELECT device_pk FROM session_tokens
         WHERE token = $1 AND expires_at > now()",
    )
    .bind(token)
    .fetch_optional(&mut *conn)
    .await?;
    Ok(row.map(|r| r.get("device_pk")))
}

/// Delete expired session tokens.
pub async fn delete_expired(conn: &mut PgConnection) -> Result<u64, sqlx::Error> {
    let result = sqlx::query("DELETE FROM session_tokens WHERE expires_at < now()")
        .execute(&mut *conn)
        .await?;
    Ok(result.rows_affected())
}
