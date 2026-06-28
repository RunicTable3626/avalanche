//! Attachment blob metadata (docs/35-attachments.md).
//!
//! Stores only what the server legitimately needs — owner, size, TTL — never
//! the key or plaintext. The ciphertext lives in the `BlobStore` backend keyed
//! by `attachment_id`.

use sqlx::{PgConnection, Row};
use time::OffsetDateTime;

/// An attachment's stored metadata.
pub struct Attachment {
    pub attachment_id: String,
    pub account_id: i64,
    pub size_bytes: i64,
}

/// Insert a freshly-allocated attachment slot.
pub async fn insert(
    conn: &mut PgConnection,
    attachment_id: &str,
    account_id: i64,
    size_bytes: i64,
    expires_at: OffsetDateTime,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO attachments (attachment_id, account_id, size_bytes, expires_at)
         VALUES ($1, $2, $3, $4)",
    )
    .bind(attachment_id)
    .bind(account_id)
    .bind(size_bytes)
    .bind(expires_at)
    .execute(&mut *conn)
    .await?;
    Ok(())
}

/// Look up an attachment by id (for upload owner-check / download existence).
pub async fn get(
    conn: &mut PgConnection,
    attachment_id: &str,
) -> Result<Option<Attachment>, sqlx::Error> {
    let row = sqlx::query(
        "SELECT attachment_id, account_id, size_bytes FROM attachments WHERE attachment_id = $1",
    )
    .bind(attachment_id)
    .fetch_optional(&mut *conn)
    .await?;
    Ok(row.map(|r| Attachment {
        attachment_id: r.get("attachment_id"),
        account_id: r.get("account_id"),
        size_bytes: r.get("size_bytes"),
    }))
}

/// Total bytes an account has uploaded within the last `window_secs` seconds —
/// the rolling bytes-per-window quota input.
pub async fn bytes_uploaded_since(
    conn: &mut PgConnection,
    account_id: i64,
    window_secs: i64,
) -> Result<i64, sqlx::Error> {
    let row = sqlx::query(
        // SUM over BIGINT yields NUMERIC in Postgres; cast back to BIGINT so it
        // decodes as i64.
        "SELECT COALESCE(SUM(size_bytes), 0)::BIGINT AS total
         FROM attachments
         WHERE account_id = $1
           AND created_at > now() - make_interval(secs => $2)",
    )
    .bind(account_id)
    .bind(window_secs as f64)
    .fetch_one(&mut *conn)
    .await?;
    Ok(row.get::<i64, _>("total"))
}

/// Delete a single attachment row (best-effort `DELETE /v1/attachments/:id`).
pub async fn delete(conn: &mut PgConnection, attachment_id: &str) -> Result<(), sqlx::Error> {
    sqlx::query("DELETE FROM attachments WHERE attachment_id = $1")
        .bind(attachment_id)
        .execute(&mut *conn)
        .await?;
    Ok(())
}

/// Delete every attachment past its TTL, returning the ids of the rows removed
/// so the caller can delete the corresponding on-disk blobs.
pub async fn delete_expired(conn: &mut PgConnection) -> Result<Vec<String>, sqlx::Error> {
    let rows = sqlx::query(
        "DELETE FROM attachments WHERE expires_at < now() RETURNING attachment_id",
    )
    .fetch_all(&mut *conn)
    .await?;
    Ok(rows.iter().map(|r| r.get::<String, _>("attachment_id")).collect())
}
