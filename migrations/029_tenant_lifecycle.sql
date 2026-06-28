-- Zavora ERP — Migration 029: user-driven tenant lifecycle
--
-- Adds soft-archive ("close") support to per-tenant configuration so a user can
-- remove a tenant from their active workspace without destroying its books.
--
-- A hard delete is deliberately NOT offered: the immutability triggers in
-- 002_immutability_triggers.sql block deletion of posted journal lines, and the
-- ledger/audit trail must be preserved for KRA compliance. Archiving is the
-- reversible, audit-preserving equivalent — an archived tenant is hidden from
-- the switcher and cannot be switched into until it is restored.
--
-- Idempotent and backward compatible: existing tenants get NULL (i.e. active).

ALTER TABLE entity_settings
    ADD COLUMN IF NOT EXISTS archived_at TIMESTAMPTZ;

ALTER TABLE entity_settings
    ADD COLUMN IF NOT EXISTS archived_by UUID;

-- Partial index to keep the common "active tenants for this user" lookup fast.
CREATE INDEX IF NOT EXISTS idx_entity_settings_active
    ON entity_settings (entity_id)
    WHERE archived_at IS NULL;
