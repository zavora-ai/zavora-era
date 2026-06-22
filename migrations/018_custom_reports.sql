-- Zavora ERP — Migration 018: custom financial report definitions
--
-- User-defined statements (à la BC Account Schedules / NetSuite Financial Report
-- Builder): an ordered list of rows — headers, account-range rows (summing GL
-- movement over a code range in the chosen sign), and subtotals (summing other
-- rows). Stored as JSONB so the row model can evolve without migrations.
-- Idempotent.

CREATE TABLE IF NOT EXISTS custom_report_definitions (
    id         UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    entity_id  UUID NOT NULL,
    name       TEXT NOT NULL,
    definition JSONB NOT NULL DEFAULT '{"rows":[]}'::jsonb,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_custom_reports_entity
    ON custom_report_definitions (entity_id);
