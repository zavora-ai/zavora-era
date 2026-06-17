-- Zavora ERP — Migration 009: organisation profile on signup
--
-- Captures the organisation's legal type and KRA PIN at tenant signup so a new
-- tenant's compliance profile is recorded up front (used on tax filings and
-- document headers).
--
-- Idempotent and backward compatible: existing rows keep NULL for both columns.

ALTER TABLE entity_settings
    ADD COLUMN IF NOT EXISTS organization_type TEXT,
    ADD COLUMN IF NOT EXISTS kra_pin           TEXT;
