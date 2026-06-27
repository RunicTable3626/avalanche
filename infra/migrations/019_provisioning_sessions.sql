-- Device-linking provisioning mailbox (docs/04-multi-device.md §4).
--
-- A short-lived, ciphertext-only rendezvous between an existing device and a
-- new device being linked to the same identity. The server is pure transport:
-- it stores opaque blobs in named slots and forwards them, never learning the
-- shared key (derived from ephemeral X25519 keypairs exchanged out-of-band via
-- the pairing code). Sessions are unauthenticated — the new device has no
-- account yet — and expire after a few minutes; nothing durable lives here, so
-- a wipe (e.g. a server redeploy) only fails an in-flight link, which retries.
--
-- `id` is an opaque server-issued token (base64url), not tied to any account.

CREATE TABLE provisioning_sessions (
    id         TEXT        NOT NULL PRIMARY KEY,  -- opaque session token
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    expires_at TIMESTAMPTZ NOT NULL              -- lazy-swept; reads gate on this
);

-- One row per (session, slot). Two slots are used: `handshake` (the scanner's
-- ephemeral pubkey) and `bundle` (the sealed ProvisioningBundle). Stored as
-- opaque bytes; `byte_len` mirrors length for a cheap size guard at read time.
CREATE TABLE provisioning_slots (
    session_id TEXT        NOT NULL REFERENCES provisioning_sessions(id) ON DELETE CASCADE,
    slot       TEXT        NOT NULL,             -- 'handshake' | 'bundle'
    ciphertext BYTEA       NOT NULL,
    byte_len   INTEGER     NOT NULL,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (session_id, slot)
);
