-- 055: Amortisation schedules — prepayments and deferred revenue.
--
-- A prepaid expense (e.g. a year's insurance paid upfront) or deferred revenue
-- (a customer paying upfront for a year's service) is booked to a balance-sheet
-- holding account, then released to the P&L in equal monthly installments by a
-- schedule that auto-posts each month (idempotent catch-up, like depreciation).
CREATE TABLE IF NOT EXISTS amortization_schedules (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    entity_id UUID NOT NULL,
    kind TEXT NOT NULL,               -- 'prepaid_expense' | 'deferred_revenue'
    description TEXT NOT NULL,
    -- Balance-sheet holding account (prepaid asset 1400 / deferred-rev liability 3450).
    balance_account TEXT NOT NULL,
    -- P&L account released into (expense for prepaid, revenue for deferred).
    pnl_account TEXT NOT NULL,
    total_amount NUMERIC(18,2) NOT NULL,
    periods INT NOT NULL,             -- number of monthly installments
    start_date DATE NOT NULL,         -- month of the first installment
    amortized_periods INT NOT NULL DEFAULT 0,  -- installments already posted (catch-up)
    status TEXT NOT NULL DEFAULT 'active',      -- active | completed | cancelled
    created_by JSONB,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_amort_entity ON amortization_schedules(entity_id);
CREATE INDEX IF NOT EXISTS idx_amort_active ON amortization_schedules(entity_id, status);

-- Seed Deferred Revenue (3450) for existing tenants that have the Kenya chart
-- (proxied by the presence of Accrued Expenses 3400).
INSERT INTO accounts (entity_id, code, name, account_type, parent_code, is_control, is_active, tags)
SELECT a.entity_id, '3450', 'Deferred Revenue', 'Liability', NULL, false, true, '[]'::jsonb
FROM accounts a
WHERE a.code = '3400'
  AND NOT EXISTS (SELECT 1 FROM accounts x WHERE x.entity_id = a.entity_id AND x.code = '3450');
