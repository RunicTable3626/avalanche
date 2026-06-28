-- Attachments (docs/35-attachments.md): metadata for end-to-end-encrypted
-- attachment blobs.
--
-- The ciphertext itself is stored by the `BlobStore` backend (local filesystem
-- in the first cut), keyed by `attachment_id`; this table holds only the
-- metadata the server legitimately needs: who uploaded it (quota accounting),
-- its size (cap + bytes/window quota), and when it expires (TTL GC). The server
-- never sees the key or the plaintext — confidentiality rides on the pointer
-- being inside the E2E message.
--
-- Reclamation is by TTL, not reference count: the server cannot see which
-- (encrypted) message references which blob, so a background task deletes rows
-- (and their on-disk blobs) past `expires_at`. The media TTL is deliberately
-- longer than the message-queue retention so an offline / newly-linked
-- recipient can still pull a blob.

CREATE TABLE attachments (
    attachment_id  TEXT        PRIMARY KEY,                 -- opaque, server-generated (UUID)
    account_id     BIGINT      NOT NULL REFERENCES accounts(id),
    size_bytes     BIGINT      NOT NULL,                    -- declared ciphertext size
    expires_at     TIMESTAMPTZ NOT NULL,                    -- TTL deadline
    created_at     TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- GC scan: DELETE ... WHERE expires_at < now().
CREATE INDEX idx_attachments_expires ON attachments (expires_at);

-- Bytes-per-window quota: SUM(size_bytes) for an account over a recent window.
CREATE INDEX idx_attachments_account_created ON attachments (account_id, created_at);
