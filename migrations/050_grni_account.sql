-- 050: Seed the GRNI accrual account (3020) for existing entities.
--
-- The posting-setup default for inventory_clearing moves from 3010 (the AP
-- control — wrong: standalone receipts looked like vendor balances with no
-- vendor subledger behind them) to a dedicated "Goods Received Not Invoiced"
-- account. New tenants get 3020 from the COA template; this backfills every
-- entity that has the AP control but no 3020 yet, so posting resolution
-- doesn't fail on upgrade.
INSERT INTO accounts (entity_id, code, name, account_type, parent_code, is_control, is_active, tags)
SELECT a.entity_id, '3020', 'Goods Received Not Invoiced', 'Liability', '3000', false, true, '[]'::jsonb
FROM accounts a
WHERE a.code = '3010'
  AND NOT EXISTS (
      SELECT 1 FROM accounts x WHERE x.entity_id = a.entity_id AND x.code = '3020'
  );
