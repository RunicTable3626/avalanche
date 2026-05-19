//! Push pseudonym queries.

use sqlx::PgConnection;

/// Register or update a push pseudonym for a device.
pub async fn register(
    conn: &mut PgConnection,
    pseudonym: &str,
    device_pk: i64,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO push_pseudonyms (pseudonym, device_pk)
         VALUES ($1, $2)
         ON CONFLICT (pseudonym) DO UPDATE SET device_pk = $2, registered_at = now()",
    )
    .bind(pseudonym)
    .bind(device_pk)
    .execute(conn)
    .await?;
    Ok(())
}

/// Remove a push pseudonym (e.g. on rotation or logout).
pub async fn unregister(
    conn: &mut PgConnection,
    pseudonym: &str,
) -> Result<bool, sqlx::Error> {
    let result = sqlx::query("DELETE FROM push_pseudonyms WHERE pseudonym = $1")
        .bind(pseudonym)
        .execute(conn)
        .await?;
    Ok(result.rows_affected() > 0)
}

/// Look up the push pseudonym for a device. Returns None if the device
/// has no registered pseudonym.
pub async fn pseudonym_for_device(
    conn: &mut PgConnection,
    device_pk: i64,
) -> Result<Option<String>, sqlx::Error> {
    let row = sqlx::query_scalar::<_, String>(
        "SELECT pseudonym FROM push_pseudonyms WHERE device_pk = $1 ORDER BY registered_at DESC LIMIT 1",
    )
    .bind(device_pk)
    .fetch_optional(conn)
    .await?;
    Ok(row)
}
