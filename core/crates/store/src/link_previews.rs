//! Local persistence of message link previews (docs/35 "Link previews").
//!
//! When a message carrying link-preview cards is saved to history, the decrypted
//! preview metadata (url/title/description/date) and the og:image pointer are
//! persisted here so the card re-renders after restart. The image pointer is
//! enough to re-fetch the og:image on demand via the normal attachment path; we
//! deliberately do NOT persist the image's local download state (preview images
//! are small and re-fetched within the blob TTL — keeps this table simple).

use crate::{db::IdentityStore, error::StoreError};

/// A decrypted link preview plus its og:image pointer (if any).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LinkPreviewRow {
    /// Local row UUID.
    pub id: String,
    /// `message_history.id` this preview belongs to.
    pub message_id: String,
    /// Position within the message.
    pub ordinal: i64,
    pub url: String,
    pub title: String,
    pub description: String,
    /// Article published date, unix millis; 0 = unknown.
    pub date_ms: i64,
    // og:image pointer — all `None` when the preview has no image.
    pub image_url: Option<String>,
    pub image_content_type: Option<String>,
    pub image_key: Option<Vec<u8>>,
    pub image_digest: Option<Vec<u8>>,
    pub image_size_bytes: Option<i64>,
    pub image_width: Option<i64>,
    pub image_height: Option<i64>,
}

impl IdentityStore {
    /// Replace the link-preview rows for a message. Idempotent: clears existing
    /// rows for `message_id` first so a re-save doesn't duplicate.
    pub async fn save_link_previews(
        &self,
        message_id: &str,
        previews: &[LinkPreviewRow],
    ) -> Result<(), StoreError> {
        let message_id = message_id.to_string();
        let rows = previews.to_vec();
        self.conn
            .call(move |conn| {
                let tx = conn.transaction()?;
                tx.execute(
                    "DELETE FROM message_link_previews WHERE message_id = ?1",
                    [&message_id],
                )?;
                for p in &rows {
                    tx.execute(
                        "INSERT INTO message_link_previews
                         (id, message_id, ordinal, url, title, description, date_ms,
                          image_url, image_content_type, image_key, image_digest,
                          image_size_bytes, image_width, image_height)
                         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)",
                        rusqlite::params![
                            p.id,
                            p.message_id,
                            p.ordinal,
                            p.url,
                            p.title,
                            p.description,
                            p.date_ms,
                            p.image_url,
                            p.image_content_type,
                            p.image_key,
                            p.image_digest,
                            p.image_size_bytes,
                            p.image_width,
                            p.image_height,
                        ],
                    )?;
                }
                tx.commit()?;
                Ok(())
            })
            .await
            .map_err(StoreError::Db)
    }

    /// Load every link preview for messages in a conversation, ordered by
    /// `(message_id, ordinal)`. The caller groups by `message_id`.
    pub async fn load_link_previews_for_conversation(
        &self,
        conversation_id: &str,
    ) -> Result<Vec<LinkPreviewRow>, StoreError> {
        let conversation_id = conversation_id.to_string();
        self.conn
            .call(move |conn| {
                let mut stmt = conn.prepare(
                    "SELECT p.id, p.message_id, p.ordinal, p.url, p.title, p.description,
                            p.date_ms, p.image_url, p.image_content_type, p.image_key,
                            p.image_digest, p.image_size_bytes, p.image_width, p.image_height
                     FROM message_link_previews p
                     JOIN message_history m ON m.id = p.message_id
                     WHERE m.conversation_id = ?1
                     ORDER BY p.message_id ASC, p.ordinal ASC",
                )?;
                let rows = stmt.query_map([&conversation_id], |row| {
                    Ok(LinkPreviewRow {
                        id: row.get(0)?,
                        message_id: row.get(1)?,
                        ordinal: row.get(2)?,
                        url: row.get(3)?,
                        title: row.get(4)?,
                        description: row.get(5)?,
                        date_ms: row.get(6)?,
                        image_url: row.get(7)?,
                        image_content_type: row.get(8)?,
                        image_key: row.get(9)?,
                        image_digest: row.get(10)?,
                        image_size_bytes: row.get(11)?,
                        image_width: row.get(12)?,
                        image_height: row.get(13)?,
                    })
                })?;
                rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
            })
            .await
            .map_err(StoreError::Db)
    }
}
