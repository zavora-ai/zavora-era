-- ============================================================
-- ENTERPRISE PAYROLL — Phase 1 foundation
-- Effective-dated statutory config, earning/deduction/department masters,
-- employee links, recurring & per-run variable inputs, loans, and payslip/pay-run
-- extensions (denormalized amounts, employee snapshot, YTD). All additive:
-- IF NOT EXISTS + nullable/defaulted columns keep legacy rows valid.
-- See docs/PAYROLL_HR_ENTERPRISE.md.
-- ============================================================

-- ── Effective-dated statutory configuration ─────────────────────────────────
-- One row per (tenant, effective_from). Payroll resolves the row with the
-- greatest effective_from <= the pay period end. `config` holds the full KRA
-- ruleset so a historical run is exactly reproducible.
CREATE TABLE IF NOT EXISTS payroll_statutory_config (
    id            UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    entity_id     UUID NOT NULL,
    effective_from DATE NOT NULL,
    name          TEXT NOT NULL,
    config        JSONB NOT NULL,
    created_by    UUID,
    created_at    TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE(entity_id, effective_from)
);
CREATE INDEX IF NOT EXISTS idx_statutory_config_entity ON payroll_statutory_config(entity_id, effective_from DESC);

-- ── Earning types (allowance/earning master) ────────────────────────────────
CREATE TABLE IF NOT EXISTS earning_types (
    id              UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    entity_id       UUID NOT NULL,
    code            TEXT NOT NULL,
    name            TEXT NOT NULL,
    taxable         BOOLEAN NOT NULL DEFAULT TRUE,
    pensionable     BOOLEAN NOT NULL DEFAULT TRUE,   -- subject to NSSF
    affects_shif    BOOLEAN NOT NULL DEFAULT TRUE,   -- included in SHA/housing base
    proratable      BOOLEAN NOT NULL DEFAULT TRUE,
    gl_account_code TEXT,
    sequence        INTEGER NOT NULL DEFAULT 100,
    active          BOOLEAN NOT NULL DEFAULT TRUE,
    is_system       BOOLEAN NOT NULL DEFAULT FALSE,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE(entity_id, code)
);
CREATE INDEX IF NOT EXISTS idx_earning_types_entity ON earning_types(entity_id);

-- ── Deduction types (voluntary/loan/welfare master; statutory are built-in) ──
CREATE TABLE IF NOT EXISTS deduction_types (
    id              UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    entity_id       UUID NOT NULL,
    code            TEXT NOT NULL,
    name            TEXT NOT NULL,
    category        TEXT NOT NULL DEFAULT 'voluntary', -- statutory|voluntary|loan|welfare
    pre_tax         BOOLEAN NOT NULL DEFAULT FALSE,     -- reduces taxable income
    gl_account_code TEXT,
    sequence        INTEGER NOT NULL DEFAULT 100,
    active          BOOLEAN NOT NULL DEFAULT TRUE,
    is_system       BOOLEAN NOT NULL DEFAULT FALSE,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE(entity_id, code)
);
CREATE INDEX IF NOT EXISTS idx_deduction_types_entity ON deduction_types(entity_id);

-- ── Departments (cost centres) ──────────────────────────────────────────────
CREATE TABLE IF NOT EXISTS departments (
    id                 UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    entity_id          UUID NOT NULL,
    code               TEXT NOT NULL,
    name               TEXT NOT NULL,
    cost_center        TEXT,
    dimension_value_id UUID,          -- link to analytical accounting dimension
    parent_id          UUID REFERENCES departments(id),
    active             BOOLEAN NOT NULL DEFAULT TRUE,
    created_at         TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE(entity_id, code)
);
CREATE INDEX IF NOT EXISTS idx_departments_entity ON departments(entity_id);

-- ── Employee links ──────────────────────────────────────────────────────────
ALTER TABLE employees ADD COLUMN IF NOT EXISTS department_id UUID REFERENCES departments(id);
ALTER TABLE employees ADD COLUMN IF NOT EXISTS pay_frequency TEXT NOT NULL DEFAULT 'Monthly';
CREATE INDEX IF NOT EXISTS idx_employees_department ON employees(department_id);

-- ── Recurring items (fixed each run: extra earnings / voluntary deductions) ──
CREATE TABLE IF NOT EXISTS employee_recurring_items (
    id          UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    entity_id   UUID NOT NULL,
    employee_id UUID NOT NULL REFERENCES employees(id),
    kind        TEXT NOT NULL,          -- earning|deduction
    type_code   TEXT,                   -- references earning_types/deduction_types.code
    name        TEXT NOT NULL,
    amount      NUMERIC(14,2) NOT NULL,
    taxable     BOOLEAN,                -- override; NULL = use type default
    start_date  DATE NOT NULL DEFAULT CURRENT_DATE,
    end_date    DATE,
    active      BOOLEAN NOT NULL DEFAULT TRUE,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
CREATE INDEX IF NOT EXISTS idx_recurring_items_emp ON employee_recurring_items(employee_id) WHERE active;
CREATE INDEX IF NOT EXISTS idx_recurring_items_entity ON employee_recurring_items(entity_id);

-- ── Per-run variable inputs (one-off bonuses/overtime/advances/deductions) ───
CREATE TABLE IF NOT EXISTS pay_run_inputs (
    id          UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    entity_id   UUID NOT NULL,
    pay_run_id  UUID NOT NULL REFERENCES pay_runs(id) ON DELETE CASCADE,
    employee_id UUID NOT NULL REFERENCES employees(id),
    kind        TEXT NOT NULL,          -- earning|deduction
    type_code   TEXT,
    name        TEXT NOT NULL,
    amount      NUMERIC(14,2) NOT NULL,
    taxable     BOOLEAN NOT NULL DEFAULT TRUE,
    note        TEXT,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
CREATE INDEX IF NOT EXISTS idx_pay_run_inputs_run ON pay_run_inputs(pay_run_id);
CREATE INDEX IF NOT EXISTS idx_pay_run_inputs_emp ON pay_run_inputs(pay_run_id, employee_id);

-- ── Loans & amortization ledger ─────────────────────────────────────────────
CREATE TABLE IF NOT EXISTS employee_loans (
    id            UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    entity_id     UUID NOT NULL,
    employee_id   UUID NOT NULL REFERENCES employees(id),
    name          TEXT NOT NULL,
    principal     NUMERIC(14,2) NOT NULL,
    balance       NUMERIC(14,2) NOT NULL,
    installment   NUMERIC(14,2) NOT NULL,
    interest_rate NUMERIC(6,4) NOT NULL DEFAULT 0,
    start_date    DATE NOT NULL DEFAULT CURRENT_DATE,
    status        TEXT NOT NULL DEFAULT 'active',   -- active|settled|suspended
    created_at    TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
CREATE INDEX IF NOT EXISTS idx_employee_loans_emp ON employee_loans(employee_id) WHERE status = 'active';
CREATE INDEX IF NOT EXISTS idx_employee_loans_entity ON employee_loans(entity_id);

CREATE TABLE IF NOT EXISTS loan_repayments (
    id            UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    entity_id     UUID NOT NULL,
    loan_id       UUID NOT NULL REFERENCES employee_loans(id),
    pay_run_id    UUID NOT NULL REFERENCES pay_runs(id) ON DELETE CASCADE,
    amount        NUMERIC(14,2) NOT NULL,
    balance_after NUMERIC(14,2) NOT NULL,
    created_at    TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE(loan_id, pay_run_id)
);
CREATE INDEX IF NOT EXISTS idx_loan_repayments_run ON loan_repayments(pay_run_id);

-- ── Pay-run extensions ──────────────────────────────────────────────────────
ALTER TABLE pay_runs ADD COLUMN IF NOT EXISTS name                TEXT;
ALTER TABLE pay_runs ADD COLUMN IF NOT EXISTS pay_group           TEXT;
ALTER TABLE pay_runs ADD COLUMN IF NOT EXISTS employee_count      INTEGER NOT NULL DEFAULT 0;
ALTER TABLE pay_runs ADD COLUMN IF NOT EXISTS total_employer_cost NUMERIC NOT NULL DEFAULT 0;
ALTER TABLE pay_runs ADD COLUMN IF NOT EXISTS notes               TEXT;

-- ── Payslip extensions: denormalized amounts + snapshot + itemization + YTD ──
ALTER TABLE payslips ADD COLUMN IF NOT EXISTS employee_name      TEXT;
ALTER TABLE payslips ADD COLUMN IF NOT EXISTS staff_number       TEXT;
ALTER TABLE payslips ADD COLUMN IF NOT EXISTS kra_pin            TEXT;
ALTER TABLE payslips ADD COLUMN IF NOT EXISTS department_id      UUID;
ALTER TABLE payslips ADD COLUMN IF NOT EXISTS gross              NUMERIC(14,2) NOT NULL DEFAULT 0;
ALTER TABLE payslips ADD COLUMN IF NOT EXISTS taxable            NUMERIC(14,2) NOT NULL DEFAULT 0;
ALTER TABLE payslips ADD COLUMN IF NOT EXISTS paye               NUMERIC(14,2) NOT NULL DEFAULT 0;
ALTER TABLE payslips ADD COLUMN IF NOT EXISTS nssf_employee      NUMERIC(14,2) NOT NULL DEFAULT 0;
ALTER TABLE payslips ADD COLUMN IF NOT EXISTS nssf_employer      NUMERIC(14,2) NOT NULL DEFAULT 0;
ALTER TABLE payslips ADD COLUMN IF NOT EXISTS sha                NUMERIC(14,2) NOT NULL DEFAULT 0;
ALTER TABLE payslips ADD COLUMN IF NOT EXISTS housing_employee   NUMERIC(14,2) NOT NULL DEFAULT 0;
ALTER TABLE payslips ADD COLUMN IF NOT EXISTS housing_employer   NUMERIC(14,2) NOT NULL DEFAULT 0;
ALTER TABLE payslips ADD COLUMN IF NOT EXISTS helb               NUMERIC(14,2) NOT NULL DEFAULT 0;
ALTER TABLE payslips ADD COLUMN IF NOT EXISTS total_deductions   NUMERIC(14,2) NOT NULL DEFAULT 0;
ALTER TABLE payslips ADD COLUMN IF NOT EXISTS net                NUMERIC(14,2) NOT NULL DEFAULT 0;
ALTER TABLE payslips ADD COLUMN IF NOT EXISTS earnings           JSONB NOT NULL DEFAULT '[]'::jsonb;
ALTER TABLE payslips ADD COLUMN IF NOT EXISTS deductions_detail  JSONB NOT NULL DEFAULT '[]'::jsonb;
ALTER TABLE payslips ADD COLUMN IF NOT EXISTS ytd                JSONB NOT NULL DEFAULT '{}'::jsonb;
CREATE INDEX IF NOT EXISTS idx_payslips_employee ON payslips(employee_id);
