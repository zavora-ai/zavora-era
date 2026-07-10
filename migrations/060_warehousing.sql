-- 060: Optional multi-warehouse + 3PL warehousing.
--
-- Adds a warehouse master (own or third-party/3PL locations) and a
-- per-(item, warehouse) stock ledger. The inventory item's `on_hand` remains the
-- authoritative TOTAL; warehouse_stock records WHERE that stock sits, kept in
-- sync so SUM(warehouse_stock.quantity) == inventory_items.on_hand. Existing
-- single-location behaviour is preserved via a default "Main Warehouse".

CREATE TABLE IF NOT EXISTS warehouses (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    entity_id UUID NOT NULL,
    code TEXT NOT NULL,
    name TEXT NOT NULL,
    -- 'own' = the company's own location; '3pl' = third-party logistics provider.
    kind TEXT NOT NULL DEFAULT 'own',
    provider TEXT,          -- 3PL provider name (when kind = '3pl')
    location TEXT,
    is_default BOOLEAN NOT NULL DEFAULT false,
    is_active BOOLEAN NOT NULL DEFAULT true,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE(entity_id, code)
);
CREATE INDEX IF NOT EXISTS idx_warehouses_entity ON warehouses(entity_id);

CREATE TABLE IF NOT EXISTS warehouse_stock (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    entity_id UUID NOT NULL,
    item_id UUID NOT NULL REFERENCES inventory_items(id) ON DELETE CASCADE,
    warehouse_id UUID NOT NULL REFERENCES warehouses(id) ON DELETE CASCADE,
    quantity NUMERIC NOT NULL DEFAULT 0,
    UNIQUE(item_id, warehouse_id)
);
CREATE INDEX IF NOT EXISTS idx_warehouse_stock_wh ON warehouse_stock(warehouse_id);
CREATE INDEX IF NOT EXISTS idx_warehouse_stock_item ON warehouse_stock(item_id);

CREATE TABLE IF NOT EXISTS warehouse_transfers (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    entity_id UUID NOT NULL,
    item_id UUID NOT NULL,
    from_warehouse_id UUID NOT NULL,
    to_warehouse_id UUID NOT NULL,
    quantity NUMERIC NOT NULL,
    tx_date DATE NOT NULL,
    notes TEXT,
    created_by UUID,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
CREATE INDEX IF NOT EXISTS idx_warehouse_transfers_entity ON warehouse_transfers(entity_id);

-- Backfill: a default warehouse for every entity that already has inventory,
-- and seed each item's current on_hand into it (so nothing is "lost").
INSERT INTO warehouses (entity_id, code, name, kind, is_default)
SELECT DISTINCT entity_id, 'MAIN', 'Main Warehouse', 'own', true
FROM inventory_items
ON CONFLICT (entity_id, code) DO NOTHING;

INSERT INTO warehouse_stock (entity_id, item_id, warehouse_id, quantity)
SELECT i.entity_id, i.id, w.id, i.on_hand
FROM inventory_items i
JOIN warehouses w ON w.entity_id = i.entity_id AND w.is_default = true
ON CONFLICT (item_id, warehouse_id) DO NOTHING;
