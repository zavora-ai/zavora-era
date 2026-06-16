-- Zavora ERP — Migration 006: authentication + performance indexes
--
-- P0 production readiness:
--   * Password hashing + account status on era_users (Req 1).
--   * Server-side refresh-token store for JWT sessions + revocation (Req 1.6).
--   * Foreign-key / frequently-filtered indexes (Req 24.1).
--
-- Idempotent: safe to re-run against a partially-migrated database.

-- ── era_users: authentication columns ──────────────────────────────────────
ALTER TABLE era_users
    ADD COLUMN IF NOT EXISTS password_hash TEXT,
    ADD COLUMN IF NOT EXISTS status        TEXT NOT NULL DEFAULT 'active',
    ADD COLUMN IF NOT EXISTS invited_at    TIMESTAMPTZ;

-- ── refresh_tokens: one row per issued refresh token (revocable session) ────
CREATE TABLE IF NOT EXISTS refresh_tokens (
    jti        UUID PRIMARY KEY,
    user_id    UUID NOT NULL,
    entity_id  UUID NOT NULL,
    expires_at TIMESTAMPTZ NOT NULL,
    revoked    BOOLEAN NOT NULL DEFAULT false,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_refresh_tokens_user
    ON refresh_tokens(user_id) WHERE revoked = false;
CREATE INDEX IF NOT EXISTS idx_refresh_tokens_expiry
    ON refresh_tokens(expires_at);

-- ── Performance indexes on FK / frequently-filtered columns (Req 24.1) ──────
CREATE INDEX IF NOT EXISTS idx_invoices_entity_customer
    ON invoices(entity_id, customer_id);
CREATE INDEX IF NOT EXISTS idx_invoices_issue_date
    ON invoices(entity_id, issue_date);
CREATE INDEX IF NOT EXISTS idx_bills_entity_vendor
    ON bills(entity_id, vendor_id);
CREATE INDEX IF NOT EXISTS idx_bills_issue_date
    ON bills(entity_id, issue_date);
CREATE INDEX IF NOT EXISTS idx_payments_entity_party
    ON payments(entity_id, party_id);
CREATE INDEX IF NOT EXISTS idx_payments_payment_date
    ON payments(entity_id, payment_date);
CREATE INDEX IF NOT EXISTS idx_journal_lines_account_code
    ON journal_lines(account_code);
CREATE INDEX IF NOT EXISTS idx_era_users_entity
    ON era_users(entity_id);
