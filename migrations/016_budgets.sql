-- Zavora ERP — Migration 016: budgets
--
-- A budget figure per account, per fiscal period. Budget-vs-Actual compares the
-- sum of these against actual ledger movement for the same accounts/period.
-- Idempotent.

CREATE TABLE IF NOT EXISTS budget_entries (
    id           UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    entity_id    UUID NOT NULL,
    period_id    UUID NOT NULL REFERENCES fiscal_periods(id),
    account_code TEXT NOT NULL,
    amount       NUMERIC NOT NULL DEFAULT 0,
    created_at   TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at   TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (entity_id, period_id, account_code)
);

CREATE INDEX IF NOT EXISTS idx_budget_entity_period
    ON budget_entries (entity_id, period_id);
