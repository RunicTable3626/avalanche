-- Stage 3: bot account fields.
-- display_name: plaintext name for bot accounts, NULL for human accounts.
-- is_bot: distinguishes bot-owned accounts from human accounts; set at
--   registration time by the Project that created the bot.

ALTER TABLE accounts
    ADD COLUMN display_name TEXT,
    ADD COLUMN is_bot BOOLEAN NOT NULL DEFAULT FALSE;
