//! DID document storage and resolution.
//!
//! Each account has a DID document stored as JSONB. The document contains the
//! account's verification methods (public keys) and service endpoints (this
//! homeserver's URL). It is served publicly at `GET /.well-known/did/:did`.
//!
//! This is a **local stub**: the DID is generated and stored on this server
//! only. Full `did:plc` directory integration — where DIDs are registered
//! with the global PLC directory and resolvable by any server — ships in
//! Stage 9 (Federation).

use sqlx::{PgConnection, Row};

/// Store a DID document for an account.
pub async fn upsert_document(
    conn: &mut PgConnection,
    account_id: i64,
    document: &serde_json::Value,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO did_documents (account_id, document)
         VALUES ($1, $2)
         ON CONFLICT (account_id) DO UPDATE SET document = $2, updated_at = now()",
    )
    .bind(account_id)
    .bind(document)
    .execute(&mut *conn)
    .await?;
    Ok(())
}

/// Fetch a DID document by DID string.
pub async fn find_by_did(
    conn: &mut PgConnection,
    did: &str,
) -> Result<Option<serde_json::Value>, sqlx::Error> {
    let row = sqlx::query(
        "SELECT dd.document
         FROM did_documents dd
         JOIN accounts a ON a.id = dd.account_id
         WHERE a.did = $1",
    )
    .bind(did)
    .fetch_optional(&mut *conn)
    .await?;
    Ok(row.map(|r| r.get("document")))
}
