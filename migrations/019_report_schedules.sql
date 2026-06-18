-- Zavora ERP — Migration 019: scheduled report delivery
--
-- A schedule fires a report on a cadence and queues it to recipients via the
-- notification outbox (channel = email). The background scheduler advances
-- next_run_at after each run. Idempotent.

CREATE TABLE IF NOT EXISTS report_schedules (
    id          UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    entity_id   UUID NOT NULL,
    name        TEXT NOT NULL,
    report_type TEXT NOT NULL,
    cadence     TEXT NOT NULL DEFAULT 'monthly',   -- daily | weekly | monthly
    recipients  TEXT NOT NULL DEFAULT '',          -- comma-separated emails
    is_active   BOOLEAN NOT NULL DEFAULT true,
    next_run_at TIMESTAMPTZ,
    last_run_at TIMESTAMPTZ,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_report_schedules_due
    ON report_schedules (entity_id, is_active, next_run_at);
