-- 051: Per-tenant period-control settings.
--
-- Soft close only admitted Manual JEs, but manual entries cannot touch the
-- AR/AP control accounts — so a late customer invoice or vendor bill for a
-- soft-closed month was un-enterable without a full reopen. Tenants can now
-- opt in to letting DOCUMENT postings (invoice/bill/credit-note/payment)
-- flow into soft-closed periods while the lock still blocks everything else.
ALTER TABLE entity_settings
    ADD COLUMN IF NOT EXISTS period_controls JSONB NOT NULL DEFAULT '{}'::jsonb;
