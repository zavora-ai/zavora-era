-- 052: Seed the Corporation Tax Payable account (3510) for existing entities.
--
-- The CIT provision posting (DR 8500 / CR 3510) needs this liability. New
-- tenants get it from the COA template; this backfills every entity that has
-- the WHT Payable control (3200 — a proxy for "has the Kenya standard chart")
-- but no 3510 yet, so posting a provision doesn't fail on upgrade.
INSERT INTO accounts (entity_id, code, name, account_type, parent_code, is_control, is_active, tags)
SELECT a.entity_id, '3510', 'Corporation Tax Payable', 'Liability', NULL, false, true, '[]'::jsonb
FROM accounts a
WHERE a.code = '3200'
  AND NOT EXISTS (
      SELECT 1 FROM accounts x WHERE x.entity_id = a.entity_id AND x.code = '3510'
  );
