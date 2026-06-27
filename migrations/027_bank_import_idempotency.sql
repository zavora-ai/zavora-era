-- Zavora ERP — Migration 027: bank statement import idempotency
--
-- Re-importing the same statement file previously created a second
-- statement_import and a full duplicate set of imported_transactions, which then
-- double-posted to the GL on categorisation. This adds two dedup layers:
--
-- 1. File-level: a content hash per (entity, bank account). Re-importing an
--    identical file is rejected/short-circuited.
-- 2. Line-level: a deterministic dedup key per transaction line so even a
--    partially-overlapping statement cannot insert a duplicate line.
--
-- Idempotent.

ALTER TABLE statement_imports
    ADD COLUMN IF NOT EXISTS content_hash TEXT;

CREATE UNIQUE INDEX IF NOT EXISTS idx_statement_imports_hash
    ON statement_imports (entity_id, bank_account_id, content_hash)
    WHERE content_hash IS NOT NULL;

ALTER TABLE imported_transactions
    ADD COLUMN IF NOT EXISTS dedup_key TEXT;

-- A line is considered a duplicate within a bank account when the same
-- (value_date, reference, debit, credit, description) tuple recurs. Backfill is
-- left NULL for historical rows (they predate dedup); the unique index only
-- constrains rows that carry a key.
CREATE UNIQUE INDEX IF NOT EXISTS idx_imported_txn_dedup
    ON imported_transactions (entity_id, bank_account, dedup_key)
    WHERE dedup_key IS NOT NULL;
