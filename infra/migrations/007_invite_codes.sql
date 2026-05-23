CREATE TABLE invite_codes (
    code TEXT PRIMARY KEY,
    created_by_account_id BIGINT NOT NULL REFERENCES accounts(id),
    target_type TEXT NOT NULL,
    target_id TEXT NOT NULL,
    expires_at TIMESTAMPTZ,
    used_by_account_id BIGINT REFERENCES accounts(id),
    used_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX idx_invite_codes_expires ON invite_codes (expires_at);
