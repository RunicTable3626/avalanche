//! Encrypted message queue: store-and-forward.
//!
//! When a client sends a message, the server enqueues the ciphertext for each
//! recipient device. If the device has a live WebSocket the message is pushed
//! immediately; otherwise it waits in the queue until the device reconnects
//! and drains via `GET /v1/messages`.
//!
//! # Security notes
//!
//! - The `ciphertext` column is opaque `bytea` — the server cannot read it.
//! - `acknowledge()` is scoped to the authenticated device's PK so a client
//!   cannot delete another device's messages.
//! - Messages have a server-enforced expiry (`expires_at`). A background task
//!   deletes expired rows. This is a defense-in-depth complement to the
//!   client-side message expiry enforced in encrypted group state.
//! - `sender_account_id` and `sender_device_pk` are nullable to support
//!   future sealed-sender mode where the server does not know who sent a message.

use sqlx::{PgConnection, Row};
use time::OffsetDateTime;

pub struct QueuedMessage {
    pub id: i64,
    pub ciphertext: Vec<u8>,
    pub message_kind: i16,
    pub enqueued_at: OffsetDateTime,
    pub sender_did: Option<String>,
    pub sender_device_id: Option<i32>,
}

/// Enqueue an encrypted message for a recipient device.
pub async fn enqueue(
    conn: &mut PgConnection,
    recipient_device_pk: i64,
    sender_account_id: Option<i64>,
    sender_device_pk: Option<i64>,
    ciphertext: &[u8],
    message_kind: i16,
    expiry_secs: i64,
) -> Result<i64, sqlx::Error> {
    let row = sqlx::query(
        "INSERT INTO message_queue
         (recipient_device_pk, sender_account_id, sender_device_pk, ciphertext, message_kind, expires_at)
         VALUES ($1, $2, $3, $4, $5, now() + make_interval(secs => $6))
         RETURNING id",
    )
    .bind(recipient_device_pk)
    .bind(sender_account_id)
    .bind(sender_device_pk)
    .bind(ciphertext)
    .bind(message_kind)
    .bind(expiry_secs as f64)
    .fetch_one(&mut *conn)
    .await?;
    Ok(row.get("id"))
}

/// Fetch all queued messages for a device, oldest first.
/// Joins to resolve sender DID and device_id from internal PKs.
pub async fn fetch_for_device(
    conn: &mut PgConnection,
    device_pk: i64,
) -> Result<Vec<QueuedMessage>, sqlx::Error> {
    let rows = sqlx::query(
        "SELECT mq.id, mq.ciphertext, mq.message_kind, mq.enqueued_at,
                a.did AS sender_did, d.device_id AS sender_device_id
         FROM message_queue mq
         LEFT JOIN devices d ON d.id = mq.sender_device_pk
         LEFT JOIN accounts a ON a.id = mq.sender_account_id
         WHERE mq.recipient_device_pk = $1
         ORDER BY mq.enqueued_at ASC, mq.id ASC",
    )
    .bind(device_pk)
    .fetch_all(&mut *conn)
    .await?;

    Ok(rows
        .into_iter()
        .map(|r| QueuedMessage {
            id: r.get("id"),
            ciphertext: r.get("ciphertext"),
            message_kind: r.get("message_kind"),
            enqueued_at: r.get("enqueued_at"),
            sender_did: r.get("sender_did"),
            sender_device_id: r.get("sender_device_id"),
        })
        .collect())
}

/// Delete messages by ID, scoped to a specific device.
pub async fn acknowledge(
    conn: &mut PgConnection,
    device_pk: i64,
    message_ids: &[i64],
) -> Result<u64, sqlx::Error> {
    let result = sqlx::query(
        "DELETE FROM message_queue
         WHERE recipient_device_pk = $1 AND id = ANY($2)",
    )
    .bind(device_pk)
    .bind(message_ids)
    .execute(&mut *conn)
    .await?;
    Ok(result.rows_affected())
}

/// Delete expired messages.
pub async fn delete_expired(conn: &mut PgConnection) -> Result<u64, sqlx::Error> {
    let result = sqlx::query("DELETE FROM message_queue WHERE expires_at < now()")
        .execute(&mut *conn)
        .await?;
    Ok(result.rows_affected())
}
