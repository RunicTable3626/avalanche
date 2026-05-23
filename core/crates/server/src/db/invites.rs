//! Invite code management: create, find, and redeem.
//!
//! Invite codes are short opaque strings that let an existing user invite
//! another user to a project or conversation. Codes may optionally expire.

use sqlx::{PgConnection, Row};
use time::OffsetDateTime;

pub struct InviteCode {
    pub code: String,
    pub created_by_account_id: i64,
    pub target_type: String,
    pub target_id: String,
    pub expires_at: Option<OffsetDateTime>,
    pub used_by_account_id: Option<i64>,
    pub used_at: Option<OffsetDateTime>,
    pub created_at: OffsetDateTime,
}

/// Create a new invite code.
pub async fn create(
    conn: &mut PgConnection,
    code: &str,
    created_by_account_id: i64,
    target_type: &str,
    target_id: &str,
    expires_at: Option<OffsetDateTime>,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO invite_codes (code, created_by_account_id, target_type, target_id, expires_at)
         VALUES ($1, $2, $3, $4, $5)",
    )
    .bind(code)
    .bind(created_by_account_id)
    .bind(target_type)
    .bind(target_id)
    .bind(expires_at)
    .execute(&mut *conn)
    .await?;
    Ok(())
}

/// Look up an invite code by its string value.
/// Returns `None` if the code does not exist or has expired.
pub async fn find_by_code(
    conn: &mut PgConnection,
    code: &str,
) -> Result<Option<InviteCode>, sqlx::Error> {
    let row = sqlx::query(
        "SELECT code, created_by_account_id, target_type, target_id, expires_at,
                used_by_account_id, used_at, created_at
         FROM invite_codes WHERE code = $1",
    )
    .bind(code)
    .fetch_optional(&mut *conn)
    .await?;

    match row {
        Some(row) => {
            let expires_at: Option<OffsetDateTime> = row.get("expires_at");
            if let Some(expires) = expires_at {
                if expires < OffsetDateTime::now_utc() {
                    return Ok(None);
                }
            }
            Ok(Some(InviteCode {
                code: row.get("code"),
                created_by_account_id: row.get("created_by_account_id"),
                target_type: row.get("target_type"),
                target_id: row.get("target_id"),
                expires_at,
                used_by_account_id: row.get("used_by_account_id"),
                used_at: row.get("used_at"),
                created_at: row.get("created_at"),
            }))
        }
        None => Ok(None),
    }
}

/// Redeem an invite code for the given account.
/// Returns `true` if the code was successfully redeemed, `false` if it was
/// already used or does not exist.
pub async fn redeem(
    conn: &mut PgConnection,
    code: &str,
    used_by_account_id: i64,
) -> Result<bool, sqlx::Error> {
    let result = sqlx::query(
        "UPDATE invite_codes
         SET used_by_account_id = $1, used_at = now()
         WHERE code = $2
           AND used_by_account_id IS NULL
           AND (expires_at IS NULL OR expires_at > now())",
    )
    .bind(used_by_account_id)
    .bind(code)
    .execute(&mut *conn)
    .await?;
    Ok(result.rows_affected() == 1)
}
