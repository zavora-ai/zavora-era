-- 063: Projects v1 — job/project accounting for NGOs (grants/funds) and
-- construction (job costing).
--
-- A project is a first-class record that is ALSO backed by a `PROJECT` GL
-- dimension value (created lazily by the projects service). Every cost/revenue
-- document that carries dimensions (invoices, bills, journals) can be tagged to
-- the project, so actuals roll up through the real ledger — no parallel silo.
-- Additive + non-breaking.

CREATE TABLE IF NOT EXISTS projects (
    id             UUID PRIMARY KEY,
    entity_id      UUID NOT NULL,
    code           TEXT NOT NULL,              -- also the PROJECT dimension value code
    name           TEXT NOT NULL,
    client_id      UUID,                        -- customer/donor (loose ref to customers)
    donor          TEXT,                        -- free-text funder (NGO) when not a customer
    manager        TEXT,                        -- project manager (name/free text)
    status         TEXT NOT NULL DEFAULT 'active',  -- planning|active|on_hold|completed|closed
    billing_method TEXT NOT NULL DEFAULT 'time_and_materials', -- fixed_fee|time_and_materials|milestone|non_billable
    budget_amount  NUMERIC(20,2) NOT NULL DEFAULT 0,   -- overall budget (roll-up of lines if used)
    currency       TEXT NOT NULL DEFAULT 'KES',
    start_date     DATE,
    end_date       DATE,
    notes          TEXT,
    is_active      BOOLEAN NOT NULL DEFAULT true,
    created_at     TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (entity_id, code)
);

-- Budget per cost category (optionally mapped to a GL account) — drives
-- budget-vs-actual (the donor report + construction job budget).
CREATE TABLE IF NOT EXISTS project_budget_lines (
    id           UUID PRIMARY KEY,
    project_id   UUID NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    category     TEXT NOT NULL,                 -- e.g. Labour, Materials, Travel, Subcontract
    account_code TEXT,                          -- optional GL account this budget maps to
    amount       NUMERIC(20,2) NOT NULL DEFAULT 0,
    notes        TEXT
);

-- Lightweight work breakdown (phases/tasks) for grouping time + costs.
CREATE TABLE IF NOT EXISTS project_tasks (
    id            UUID PRIMARY KEY,
    project_id    UUID NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    name          TEXT NOT NULL,
    budget_hours  NUMERIC(20,2) NOT NULL DEFAULT 0,
    budget_amount NUMERIC(20,2) NOT NULL DEFAULT 0,
    status        TEXT NOT NULL DEFAULT 'open',  -- open|done
    sort_order    INT NOT NULL DEFAULT 0
);

CREATE INDEX IF NOT EXISTS idx_projects_entity ON projects(entity_id);
CREATE INDEX IF NOT EXISTS idx_project_budget_lines_project ON project_budget_lines(project_id);
CREATE INDEX IF NOT EXISTS idx_project_tasks_project ON project_tasks(project_id);
