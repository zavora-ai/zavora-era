-- ============================================================
-- HR — Onboarding (Cycle 7). Also used by offboarding (Cycle 8) via `type`.
-- A case tracks a new hire (or leaver) through a checklist; probation end is
-- tracked on the case for onboarding.
-- ============================================================

CREATE TABLE IF NOT EXISTS onboarding_cases (
    id            UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    entity_id     UUID NOT NULL,
    employee_id   UUID NOT NULL REFERENCES employees(id),
    type          TEXT NOT NULL DEFAULT 'Onboarding',   -- Onboarding | Offboarding
    status        TEXT NOT NULL DEFAULT 'InProgress',    -- InProgress | Complete | Cancelled
    start_date    DATE NOT NULL,
    target_date   DATE,
    probation_end DATE,
    notes         TEXT,
    created_by    UUID,
    created_at    TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    completed_at  TIMESTAMPTZ
);
CREATE INDEX IF NOT EXISTS idx_onboarding_cases_entity ON onboarding_cases(entity_id, type, status);
CREATE INDEX IF NOT EXISTS idx_onboarding_cases_emp ON onboarding_cases(employee_id);

CREATE TABLE IF NOT EXISTS onboarding_tasks (
    id          UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    entity_id   UUID NOT NULL,
    case_id     UUID NOT NULL REFERENCES onboarding_cases(id),
    title       TEXT NOT NULL,
    is_done     BOOLEAN NOT NULL DEFAULT FALSE,
    done_at     TIMESTAMPTZ,
    sort_order  INTEGER NOT NULL DEFAULT 0
);
CREATE INDEX IF NOT EXISTS idx_onboarding_tasks_case ON onboarding_tasks(case_id);
