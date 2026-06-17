-- Zavora ERP — Migration 012: reporting aggregation indexes
--
-- Every financial statement aggregates journal_lines for posted entries within
-- a date window. Two cheap, safe improvements:
--   * A composite (entity_id, status, date) index on journal_entries so the
--     "posted entries in range" selection is a single index scan.
--   * Drop the duplicate account_code index on journal_lines (idx_jl_account
--     already covers it) to cut write amplification.
--
-- NOTE: these speed up period-range reports (P&L, GL, VAT). Cumulative "as-at"
-- balances (Trial Balance, Balance Sheet) still scan the full ledger up to the
-- date by design; making those O(periods) instead of O(history) requires the
-- period-balance snapshot work tracked separately.
--
-- Idempotent.

CREATE INDEX IF NOT EXISTS idx_je_entity_status_date
    ON journal_entries(entity_id, status, date);

DROP INDEX IF EXISTS idx_journal_lines_account_code;
