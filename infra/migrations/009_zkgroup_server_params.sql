-- The homeserver's zkgroup signing key (ServerSecretParams), used to issue
-- anonymous auth credentials and group send endorsements for action-bound
-- groups (see docs/03-groups.md §2.1, §3.3).
--
-- Singleton: at most one active row, identified by `version`. We keep a
-- version column rather than a single hard-coded row id so that future key
-- rotation (issuing under a new version while still verifying presentations
-- bound to old versions during a grace window) is a schema-compatible change.
-- Stage 5 ships with exactly one version pinned at 1; rotation policy is
-- deferred.
--
-- The `params` column is the bincode-serialized ServerSecretParams produced
-- by `zkgroup::serialize(&ServerSecretParams)`. It is private key material:
-- exposure compromises the server's authority to issue credentials.
CREATE TABLE zkgroup_server_params (
    version    INTEGER PRIMARY KEY,            -- public
    params     BYTEA NOT NULL,                 -- exempt (private signing key)
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()  -- public
);
