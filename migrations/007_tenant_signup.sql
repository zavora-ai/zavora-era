-- Zavora ERP — Migration 007: tenant signup
--
-- Adds the human-readable organisation name to per-tenant configuration so a
-- newly provisioned tenant is identifiable in the UI and administrative views
-- (Req 12.1).
--
-- Idempotent and backward compatible: existing rows receive the default
-- placeholder so the NOT NULL column can be added without downtime.

ALTER TABLE entity_settings
    ADD COLUMN IF NOT EXISTS organization_name TEXT NOT NULL DEFAULT 'My Company';
