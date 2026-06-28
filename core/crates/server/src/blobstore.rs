//! Opaque attachment blob storage (docs/35-attachments.md).
//!
//! The server stores attachment ciphertext as opaque bytes addressed by an
//! opaque `attachment_id`; it never sees the key or plaintext. This module
//! defines the [`BlobStore`] trait and its first-cut [`LocalFs`] backend. An
//! S3-compatible backend (presigned URLs) is a later increment that slots in
//! behind the same trait with no protocol change.

use std::io::ErrorKind;
use std::path::{Path, PathBuf};

use async_trait::async_trait;
use tokio::fs;
use uuid::Uuid;

use crate::error::ServerError;

/// A dumb, untrusted blob store: put bytes by id, get them back, delete them.
#[async_trait]
pub trait BlobStore: Send + Sync {
    /// Store `blob` under `attachment_id`, overwriting any existing bytes.
    async fn put(&self, attachment_id: &str, blob: &[u8]) -> Result<(), ServerError>;
    /// Fetch the blob, or `None` if absent.
    async fn get(&self, attachment_id: &str) -> Result<Option<Vec<u8>>, ServerError>;
    /// Delete the blob (idempotent — absent is success).
    async fn delete(&self, attachment_id: &str) -> Result<(), ServerError>;
}

/// Local-filesystem blob store: one file per attachment under a base directory,
/// named by the opaque `attachment_id`. The default for a fresh self-host.
pub struct LocalFs {
    base_dir: PathBuf,
}

impl LocalFs {
    pub fn new(base_dir: impl Into<PathBuf>) -> Self {
        Self { base_dir: base_dir.into() }
    }

    /// Resolve the on-disk path for an id. Attachment ids are always
    /// server-minted UUIDs (`allocate` uses `Uuid::new_v4()`), so we require the
    /// id to parse as a UUID — the tightest possible guard, and the only one
    /// between a client-supplied id and the filesystem (download is
    /// unauthenticated and takes the id straight from the URL). A valid UUID is
    /// only hex digits and hyphens, so it cannot contain a path separator or
    /// `..` and the join can never escape `base_dir`.
    fn path_for(&self, attachment_id: &str) -> Result<PathBuf, ServerError> {
        if Uuid::parse_str(attachment_id).is_err() {
            return Err(ServerError::BadRequest("invalid attachment id".into()));
        }
        Ok(self.base_dir.join(attachment_id))
    }
}

#[async_trait]
impl BlobStore for LocalFs {
    async fn put(&self, attachment_id: &str, blob: &[u8]) -> Result<(), ServerError> {
        let path = self.path_for(attachment_id)?;
        ensure_dir(&self.base_dir).await?;
        fs::write(&path, blob)
            .await
            .map_err(|e| ServerError::Internal(format!("blob write failed: {e}")))?;
        Ok(())
    }

    async fn get(&self, attachment_id: &str) -> Result<Option<Vec<u8>>, ServerError> {
        let path = self.path_for(attachment_id)?;
        match fs::read(&path).await {
            Ok(bytes) => Ok(Some(bytes)),
            Err(e) if e.kind() == ErrorKind::NotFound => Ok(None),
            Err(e) => Err(ServerError::Internal(format!("blob read failed: {e}"))),
        }
    }

    async fn delete(&self, attachment_id: &str) -> Result<(), ServerError> {
        let path = self.path_for(attachment_id)?;
        match fs::remove_file(&path).await {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == ErrorKind::NotFound => Ok(()),
            Err(e) => Err(ServerError::Internal(format!("blob delete failed: {e}"))),
        }
    }
}

async fn ensure_dir(dir: &Path) -> Result<(), ServerError> {
    fs::create_dir_all(dir)
        .await
        .map_err(|e| ServerError::Internal(format!("blob dir create failed: {e}")))
}

#[cfg(test)]
mod tests {
    use super::*;

    // Attachment ids are UUIDs; the store enforces that.
    const ID_A: &str = "3f2504e0-4f89-41d3-9a0c-0305e82c3301";
    const ID_B: &str = "0e37df36-f698-11e6-8dd4-cb9ced3df976";

    #[tokio::test]
    async fn local_fs_round_trip_and_delete() {
        let dir = std::env::temp_dir().join(format!("av-blob-test-{}", std::process::id()));
        let store = LocalFs::new(&dir);

        assert!(store.get(ID_B).await.unwrap().is_none());

        store.put(ID_A, b"ciphertext").await.unwrap();
        assert_eq!(store.get(ID_A).await.unwrap().as_deref(), Some(&b"ciphertext"[..]));

        store.delete(ID_A).await.unwrap();
        assert!(store.get(ID_A).await.unwrap().is_none());
        // Delete is idempotent.
        store.delete(ID_A).await.unwrap();

        let _ = tokio::fs::remove_dir_all(&dir).await;
    }

    #[tokio::test]
    async fn rejects_non_uuid_ids() {
        let store = LocalFs::new(std::env::temp_dir().join("av-blob-traversal"));
        // Only well-formed UUIDs are accepted, so a crafted id can never contain
        // a separator or `..` and can never escape the base dir. This rejects
        // traversal attempts and any other non-UUID id outright.
        for bad in [
            "",
            "..",
            "../escape",
            "../../etc/passwd",
            "a/b",
            "/etc/passwd",
            "/",
            "sub/dir",
            "not-a-uuid",
            // a UUID with a traversal suffix is still not a valid UUID
            "3f2504e0-4f89-41d3-9a0c-0305e82c3301/../x",
        ] {
            assert!(store.put(bad, b"x").await.is_err(), "put({bad:?}) must be rejected");
            assert!(store.get(bad).await.is_err(), "get({bad:?}) must be rejected");
            assert!(store.delete(bad).await.is_err(), "delete({bad:?}) must be rejected");
        }
        // A well-formed UUID is accepted.
        assert!(store.get(ID_A).await.is_ok());
    }
}
