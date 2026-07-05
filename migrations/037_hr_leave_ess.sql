-- ============================================================
-- HR & PEOPLE — Phase 1: Employee Self-Service (ESS) + Leave management
-- Employees are an EXTERNAL-style principal class (employee_users), entirely
-- separate from back-office staff (era_users), exactly mirroring the vendor
-- portal (vendor_users). Their logins carry a distinct 'Employee' JWT role.
-- Adds configurable leave (types, balances, requests, holidays). Kenyan
-- defaults are seeded per tenant by the app on first use.
-- ============================================================

-- ── Org fields on the employee master ───────────────────────────────────────
-- Nullable so existing payroll-only employees remain valid.
ALTER TABLE employees ADD COLUMN IF NOT EXISTS manager_id     UUID NULL REFERENCES employees(id);
ALTER TABLE employees ADD COLUMN IF NOT EXISTS department     TEXT NULL;
ALTER TABLE employees ADD COLUMN IF NOT EXISTS job_title      TEXT NULL;
ALTER TABLE employees ADD COLUMN IF NOT EXISTS personal_email TEXT NULL;
ALTER TABLE employees ADD COLUMN IF NOT EXISTS phone          TEXT NULL;

CREATE INDEX IF NOT EXISTS idx_employees_manager ON employees(manager_id);

-- ── Employee self-service logins (portal) ───────────────────────────────────
-- Separate principal class from era_users (back-office). Mirrors vendor_users.
CREATE TABLE IF NOT EXISTS employee_users (
    id            UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    entity_id     UUID NOT NULL,                     -- the employing tenant
    email         TEXT NOT NULL,
    display_name  TEXT NOT NULL,
    password_hash TEXT,                              -- NULL until invite accepted
    status        TEXT NOT NULL DEFAULT 'invited',   -- invited|active|suspended
    employee_id   UUID REFERENCES employees(id),     -- linked employee master
    last_login    TIMESTAMPTZ,
    created_at    TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE(entity_id, email)
);
CREATE INDEX IF NOT EXISTS idx_employee_users_entity   ON employee_users(entity_id);
CREATE INDEX IF NOT EXISTS idx_employee_users_employee ON employee_users(employee_id);

-- ── Leave types (configurable per tenant) ───────────────────────────────────
-- accrual_method: FixedAnnual | MonthlyAccrual | Unlimited
CREATE TABLE IF NOT EXISTS leave_types (
    id                  UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    entity_id           UUID NOT NULL,
    name                TEXT NOT NULL,
    code                TEXT NOT NULL,
    paid                BOOLEAN NOT NULL DEFAULT TRUE,
    accrual_method      TEXT NOT NULL DEFAULT 'MonthlyAccrual',
    days_per_year       NUMERIC(6,2) NOT NULL DEFAULT 0,
    carryover_max       NUMERIC(6,2) NOT NULL DEFAULT 0,
    requires_attachment BOOLEAN NOT NULL DEFAULT FALSE,
    is_statutory        BOOLEAN NOT NULL DEFAULT FALSE,
    active              BOOLEAN NOT NULL DEFAULT TRUE,
    created_at          TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE(entity_id, code)
);
CREATE INDEX IF NOT EXISTS idx_leave_types_entity ON leave_types(entity_id);

-- ── Leave balances (per employee, per type, per year) ───────────────────────
CREATE TABLE IF NOT EXISTS leave_balances (
    id             UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    entity_id      UUID NOT NULL,
    employee_id    UUID NOT NULL REFERENCES employees(id),
    leave_type_id  UUID NOT NULL REFERENCES leave_types(id),
    year           INTEGER NOT NULL,
    entitled_days  NUMERIC(6,2) NOT NULL DEFAULT 0,
    accrued_days   NUMERIC(6,2) NOT NULL DEFAULT 0,
    taken_days     NUMERIC(6,2) NOT NULL DEFAULT 0,
    pending_days   NUMERIC(6,2) NOT NULL DEFAULT 0,
    carried_over   NUMERIC(6,2) NOT NULL DEFAULT 0,
    updated_at     TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE(employee_id, leave_type_id, year)
);
CREATE INDEX IF NOT EXISTS idx_leave_balances_entity ON leave_balances(entity_id);
CREATE INDEX IF NOT EXISTS idx_leave_balances_emp    ON leave_balances(employee_id, year);

-- ── Leave requests ──────────────────────────────────────────────────────────
-- status: Pending | Approved | Declined | Cancelled
CREATE TABLE IF NOT EXISTS leave_requests (
    id              UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    entity_id       UUID NOT NULL,
    employee_id     UUID NOT NULL REFERENCES employees(id),
    leave_type_id   UUID NOT NULL REFERENCES leave_types(id),
    start_date      DATE NOT NULL,
    end_date        DATE NOT NULL,
    half_day_start  BOOLEAN NOT NULL DEFAULT FALSE,
    half_day_end    BOOLEAN NOT NULL DEFAULT FALSE,
    working_days    NUMERIC(6,2) NOT NULL,
    reason          TEXT,
    attachment_url  TEXT,
    status          TEXT NOT NULL DEFAULT 'Pending',
    approver_id     UUID NULL,
    decided_at      TIMESTAMPTZ NULL,
    decision_note   TEXT,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CHECK (end_date >= start_date)
);
CREATE INDEX IF NOT EXISTS idx_leave_requests_entity ON leave_requests(entity_id);
CREATE INDEX IF NOT EXISTS idx_leave_requests_emp    ON leave_requests(employee_id);
CREATE INDEX IF NOT EXISTS idx_leave_requests_status ON leave_requests(entity_id, status);

-- ── Holidays (public/company, exclude from working-day counts) ──────────────
CREATE TABLE IF NOT EXISTS holidays (
    id         UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    entity_id  UUID NOT NULL,
    date       DATE NOT NULL,
    name       TEXT NOT NULL,
    recurring  BOOLEAN NOT NULL DEFAULT FALSE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE(entity_id, date)
);
CREATE INDEX IF NOT EXISTS idx_holidays_entity ON holidays(entity_id);
