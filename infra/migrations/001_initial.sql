-- Stage 2: initial homeserver schema.
-- All message content columns are bytea — the server stores ciphertext it cannot read.
-- Internal bigint PKs for efficient joins; external API uses DIDs and device_id integers.

CREATE TABLE accounts (
    id          BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    did         TEXT NOT NULL UNIQUE,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE did_documents (
    account_id  BIGINT PRIMARY KEY REFERENCES accounts(id),
    document    JSONB NOT NULL,
    updated_at  TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE devices (
    id              BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    account_id      BIGINT NOT NULL REFERENCES accounts(id),
    device_id       INTEGER NOT NULL,
    identity_key    BYTEA NOT NULL,
    registration_id INTEGER NOT NULL,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (account_id, device_id)
);

CREATE TABLE session_tokens (
    token       TEXT PRIMARY KEY,
    device_pk   BIGINT NOT NULL REFERENCES devices(id),
    issued_at   TIMESTAMPTZ NOT NULL DEFAULT now(),
    expires_at  TIMESTAMPTZ NOT NULL
);
CREATE INDEX idx_session_tokens_expires ON session_tokens (expires_at);

CREATE TABLE signed_prekeys (
    id          INTEGER NOT NULL,
    device_pk   BIGINT NOT NULL REFERENCES devices(id),
    public_key  BYTEA NOT NULL,
    signature   BYTEA NOT NULL,
    uploaded_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (device_pk, id)
);

CREATE TABLE one_time_prekeys (
    id          INTEGER NOT NULL,
    device_pk   BIGINT NOT NULL REFERENCES devices(id),
    public_key  BYTEA NOT NULL,
    uploaded_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (device_pk, id)
);

CREATE TABLE kyber_prekeys (
    id          INTEGER NOT NULL,
    device_pk   BIGINT NOT NULL REFERENCES devices(id),
    public_key  BYTEA NOT NULL,
    signature   BYTEA NOT NULL,
    uploaded_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (device_pk, id)
);

CREATE TABLE message_queue (
    id                  BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    recipient_device_pk BIGINT NOT NULL REFERENCES devices(id),
    sender_account_id   BIGINT,
    sender_device_pk    BIGINT,
    ciphertext          BYTEA NOT NULL,
    message_kind        SMALLINT NOT NULL,
    enqueued_at         TIMESTAMPTZ NOT NULL DEFAULT now(),
    expires_at          TIMESTAMPTZ NOT NULL
);
CREATE INDEX idx_message_queue_recipient ON message_queue (recipient_device_pk, enqueued_at);
CREATE INDEX idx_message_queue_expires ON message_queue (expires_at);

CREATE TABLE push_pseudonyms (
    pseudonym       TEXT PRIMARY KEY,
    device_pk       BIGINT NOT NULL REFERENCES devices(id),
    registered_at   TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE rate_limit_counters (
    account_id   BIGINT NOT NULL REFERENCES accounts(id),
    action       TEXT NOT NULL,
    window_start TIMESTAMPTZ NOT NULL,
    count        INTEGER NOT NULL DEFAULT 1,
    PRIMARY KEY (account_id, action, window_start)
);
