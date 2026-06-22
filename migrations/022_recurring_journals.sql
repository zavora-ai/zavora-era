-- Zavora ERP — Migration 022: recurring / accrual / prepayment journals
--
-- A template of balanced journal lines posted automatically on a cadence by the
-- background scheduler. `auto_reverse` handles accruals: the entry posts on the
-- run date and a mirror reversal posts on the first day of the next month
-- (prepayment amortisation just leaves auto_reverse off). Idempotent.

CREATE TABLE IF NOT EXISTS recurring_journals (
    id            UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    entity_id     UUID NOT NULL,
    name          TEXT NOT NULL,
    cadence       TEXT NOT NULL DEFAULT 'monthly',   -- weekly | monthly | quarterly
    lines         JSONB NOT NULL DEFAULT '[]'::jsonb, -- [{account_code, debit, credit, description}]
    auto_reverse  BOOLEAN NOT NULL DEFAULT false,
    is_active     BOOLEAN NOT NULL DEFAULT true,
    next_run_date DATE NOT NULL,
    last_run_at   TIMESTAMPTZ,
    created_at    TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_recurring_journals_due
    ON recurring_journals (entity_id, is_active, next_run_date);
