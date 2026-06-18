-- Zavora ERP — Migration 014: account period-balance snapshots (Tier 2)
--
-- An "as-at" balance (Trial Balance, Balance Sheet) is the sum of all movement
-- up to a date — inherently O(entire ledger). To make it O(periods), we keep a
-- per-account, per-fiscal-period movement snapshot, maintained transactionally
-- with each posting. An as-at balance is then:
--
--     SUM(snapshot movement for periods that ended on/before the date)
--   + SUM(raw line movement in the still-open tail, up to the date)
--
-- so only one (current) period's lines are ever scanned, regardless of history.
--
-- Idempotent.

CREATE TABLE IF NOT EXISTS account_period_balances (
    entity_id    UUID NOT NULL,
    account_code TEXT NOT NULL,
    period_id    UUID NOT NULL,
    period_end   DATE NOT NULL,
    debit_total  NUMERIC NOT NULL DEFAULT 0,
    credit_total NUMERIC NOT NULL DEFAULT 0,
    PRIMARY KEY (entity_id, account_code, period_id)
);

CREATE INDEX IF NOT EXISTS idx_apb_entity_period_end
    ON account_period_balances (entity_id, period_end);

-- Backfill from existing posted lines, bucketed into their fiscal period.
INSERT INTO account_period_balances (entity_id, account_code, period_id, period_end, debit_total, credit_total)
SELECT jl.entity_id, jl.account_code, fp.id, fp.end_date,
       COALESCE(SUM(jl.functional_debit), 0),
       COALESCE(SUM(jl.functional_credit), 0)
FROM journal_lines jl
JOIN fiscal_periods fp
  ON fp.entity_id = jl.entity_id
 AND jl.entry_date BETWEEN fp.start_date AND fp.end_date
GROUP BY jl.entity_id, jl.account_code, fp.id, fp.end_date
ON CONFLICT (entity_id, account_code, period_id) DO NOTHING;
