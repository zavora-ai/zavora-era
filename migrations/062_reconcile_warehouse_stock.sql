-- 062: reconcile warehouse_stock with on_hand.
--
-- Before the fix in services/catalog.rs (post_opening_stock now mirrors opening
-- stock into a warehouse), a product created with opening stock booked on_hand
-- but never populated warehouse_stock. A later issue (sale/production) then drove
-- that item's warehouse_stock negative. This backfills the difference into each
-- affected item's default warehouse so the invariant
--   SUM(warehouse_stock.quantity) == inventory_items.on_hand
-- holds again. Idempotent: a clean database has no mismatches and this is a no-op.

-- 1. Ensure every entity that holds inventory has a default warehouse. Most were
--    created by migration 060; this covers entities that only gained inventory
--    afterwards through the pre-fix opening-stock path (which never triggered
--    default-warehouse creation).
INSERT INTO warehouses (id, entity_id, code, name, kind, is_default, is_active, created_at)
SELECT gen_random_uuid(), e.entity_id, 'MAIN', 'Main Warehouse', 'own', true, true, now()
FROM (SELECT DISTINCT entity_id FROM inventory_items) e
WHERE NOT EXISTS (SELECT 1 FROM warehouses w WHERE w.entity_id = e.entity_id);

-- 2. Top up the default warehouse by (on_hand - current warehouse total) for
--    every mismatched item.
WITH def AS (
    SELECT DISTINCT ON (entity_id) entity_id, id AS wh_id
    FROM warehouses
    WHERE is_active
    ORDER BY entity_id, is_default DESC, created_at ASC
),
mismatch AS (
    SELECT ii.id AS item_id, ii.entity_id,
           ii.on_hand - COALESCE((SELECT SUM(ws.quantity) FROM warehouse_stock ws WHERE ws.item_id = ii.id), 0) AS delta
    FROM inventory_items ii
    WHERE ii.on_hand <> COALESCE((SELECT SUM(ws.quantity) FROM warehouse_stock ws WHERE ws.item_id = ii.id), 0)
)
INSERT INTO warehouse_stock (id, entity_id, item_id, warehouse_id, quantity)
SELECT gen_random_uuid(), m.entity_id, m.item_id, d.wh_id, m.delta
FROM mismatch m
JOIN def d ON d.entity_id = m.entity_id
ON CONFLICT (item_id, warehouse_id)
DO UPDATE SET quantity = warehouse_stock.quantity + EXCLUDED.quantity;
