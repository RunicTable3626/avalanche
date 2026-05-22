//! Local account identity and registration state.
//!
//! This module handles the two pieces of persistent state that exist before any
//! messages are sent:
//!
//! - **Identity key pair** — the long-term Ed25519 key pair generated at
//!   account creation. Stored in the `identity_keypair` table alongside the
//!   libsignal registration ID. This is also the data that
//!   [`libsignal_protocol::IdentityKeyStore::get_identity_key_pair`] reads when
//!   building outgoing messages (that implementation lives in [`crate::session`];
//!   the storage layer is shared via the same database connection).
//!
//! - **Registration info** — the account DID and homeserver URL confirmed after
//!   the server accepts the registration request. Absent until registration
//!   completes; `app-core` checks for its presence to decide whether to show the
//!   onboarding flow.

use rusqlite::OptionalExtension as _;
use types::Timestamp;

use crate::{db::Store, error::StoreError};

/// The local account state saved after successful registration.
#[derive(Debug, Clone)]
pub struct RegistrationInfo {
    pub account_id: String,
    pub server_url: String,
    pub registered_at: Timestamp,
}

impl Store {
    /// Persist the local identity key pair and libsignal registration ID.
    /// Called once during account creation.
    pub async fn save_identity(
        &self,
        keypair: &crypto::IdentityKeyPair,
        registration_id: u32,
    ) -> Result<(), StoreError> {
        let bytes = keypair.serialize();
        self.conn
            .call(move |conn| {
                conn.execute(
                    "INSERT OR REPLACE INTO identity_keypair (id, keypair_bytes, registration_id)
                     VALUES (1, ?1, ?2)",
                    rusqlite::params![bytes, registration_id],
                )?;
                Ok(())
            })
            .await
            .map_err(StoreError::Db)
    }

    /// Load the local identity key pair. Returns `None` if not yet created.
    pub async fn load_identity(
        &self,
    ) -> Result<Option<crypto::IdentityKeyPair>, StoreError> {
        let result: Option<Vec<u8>> = self
            .conn
            .call(|conn| {
                conn.query_row(
                    "SELECT keypair_bytes FROM identity_keypair WHERE id = 1",
                    [],
                    |row| row.get(0),
                )
                .optional()
                .map_err(Into::into)
            })
            .await
            .map_err(StoreError::Db)?;

        match result {
            Some(bytes) => crypto::IdentityKeyPair::deserialize(&bytes)
                .map(Some)
                .map_err(|e| StoreError::Corrupt(e.to_string())),
            None => Ok(None),
        }
    }

    /// Persist registration details after the homeserver confirms the account.
    pub async fn save_registration(&self, info: &RegistrationInfo) -> Result<(), StoreError> {
        let account_id = info.account_id.clone();
        let server_url = info.server_url.clone();
        let registered_at = info.registered_at.as_millis();
        self.conn
            .call(move |conn| {
                conn.execute(
                    "INSERT OR REPLACE INTO account (id, account_id, server_url, registered_at)
                     VALUES (1, ?1, ?2, ?3)",
                    rusqlite::params![account_id, server_url, registered_at],
                )?;
                Ok(())
            })
            .await
            .map_err(StoreError::Db)
    }

    /// Load registration details. Returns `None` if not yet registered.
    pub async fn load_registration(&self) -> Result<Option<RegistrationInfo>, StoreError> {
        let result = self
            .conn
            .call(|conn| {
                conn.query_row(
                    "SELECT account_id, server_url, registered_at FROM account WHERE id = 1",
                    [],
                    |row| {
                        Ok((
                            row.get::<_, String>(0)?,
                            row.get::<_, String>(1)?,
                            row.get::<_, i64>(2)?,
                        ))
                    },
                )
                .optional()
                .map_err(Into::into)
            })
            .await
            .map_err(StoreError::Db)?;

        Ok(result.map(|(account_id, server_url, registered_at)| RegistrationInfo {
            account_id,
            server_url,
            registered_at: Timestamp(registered_at),
        }))
    }

    pub async fn has_recovery_key(&self) -> Result<bool, StoreError> {
        let count: i64 = self
            .conn
            .call(|conn| {
                conn.query_row(
                    "SELECT COUNT(*) FROM recovery_keys",
                    [],
                    |row| row.get(0),
                )
                .map_err(Into::into)
            })
            .await
            .map_err(StoreError::Db)?;
        Ok(count > 0)
    }

    pub async fn save_recovery_key(&self, key_material: &[u8]) -> Result<(), StoreError> {
        let key_material = key_material.to_vec();
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;
        self.conn
            .call(move |conn| {
                conn.execute(
                    "INSERT OR REPLACE INTO recovery_keys (id, key_material, created_at) VALUES (1, ?1, ?2)",
                    rusqlite::params![key_material, now],
                )?;
                Ok(())
            })
            .await
            .map_err(StoreError::Db)
    }
}
