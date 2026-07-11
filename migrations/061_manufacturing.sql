-- 061: Manufacturing v1 — Bills of Materials + Work Orders.
--
-- Light manufacturing on top of the existing inventory + warehousing layer: a
-- finished-good product gets a BOM (recipe of component items + optional
-- labour/overhead); a work order produces N units, consuming components into
-- Work in Progress and receiving finished goods out of it. Non-breaking:
-- purely additive tables + a backfilled WIP account.

-- A bill of materials: the recipe to produce `output_quantity` units of a
-- finished good from the listed components. One BOM per finished-good product.
CREATE TABLE IF NOT EXISTS boms (
    id              UUID PRIMARY KEY,
    entity_id       UUID NOT NULL,
    product_id      UUID NOT NULL,              -- finished-good product (catalog)
    output_item_id  UUID NOT NULL,              -- finished-good inventory item produced
    output_quantity NUMERIC(20,4) NOT NULL DEFAULT 1,   -- units made per batch
    overhead_cost   NUMERIC(20,4) NOT NULL DEFAULT 0,    -- labour/overhead per batch
    notes           TEXT,
    is_active       BOOLEAN NOT NULL DEFAULT true,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (entity_id, product_id)
);

CREATE TABLE IF NOT EXISTS bom_lines (
    id                UUID PRIMARY KEY,
    bom_id            UUID NOT NULL REFERENCES boms(id) ON DELETE CASCADE,
    component_item_id UUID NOT NULL,            -- inventory item consumed
    quantity          NUMERIC(20,4) NOT NULL,   -- per BOM batch
    notes             TEXT
);

-- A production run against a BOM. Lifecycle: draft → in_progress (components
-- issued to WIP) → completed (finished goods received from WIP). Cancellable
-- only while draft.
CREATE TABLE IF NOT EXISTS work_orders (
    id                  UUID PRIMARY KEY,
    entity_id           UUID NOT NULL,
    number              TEXT NOT NULL,
    bom_id              UUID NOT NULL REFERENCES boms(id),
    output_item_id      UUID NOT NULL,
    quantity            NUMERIC(20,4) NOT NULL,           -- finished units to produce
    status              TEXT NOT NULL DEFAULT 'draft',    -- draft|in_progress|completed|cancelled
    source_warehouse_id UUID,                             -- consume components from
    dest_warehouse_id   UUID,                             -- receive finished goods into
    material_cost       NUMERIC(20,4) NOT NULL DEFAULT 0,
    overhead_cost       NUMERIC(20,4) NOT NULL DEFAULT 0,
    total_cost          NUMERIC(20,4) NOT NULL DEFAULT 0,
    output_unit_cost    NUMERIC(20,4) NOT NULL DEFAULT 0,
    notes               TEXT,
    start_journal_id    UUID,
    complete_journal_id UUID,
    started_at          TIMESTAMPTZ,
    completed_at        TIMESTAMPTZ,
    created_by          JSONB,
    created_at          TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (entity_id, number)
);

-- What a work order actually consumed (captured at start, at WAC).
CREATE TABLE IF NOT EXISTS work_order_consumptions (
    id                UUID PRIMARY KEY,
    work_order_id     UUID NOT NULL REFERENCES work_orders(id) ON DELETE CASCADE,
    component_item_id UUID NOT NULL,
    quantity          NUMERIC(20,4) NOT NULL,
    unit_cost         NUMERIC(20,4) NOT NULL,
    total_cost        NUMERIC(20,4) NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_boms_entity ON boms(entity_id);
CREATE INDEX IF NOT EXISTS idx_bom_lines_bom ON bom_lines(bom_id);
CREATE INDEX IF NOT EXISTS idx_work_orders_entity ON work_orders(entity_id);
CREATE INDEX IF NOT EXISTS idx_work_orders_status ON work_orders(entity_id, status);
CREATE INDEX IF NOT EXISTS idx_woc_wo ON work_order_consumptions(work_order_id);

-- Backfill Work in Progress (1510) for existing entities that already have the
-- Inventory asset (1500). New tenants get it from the COA template. WIP holds
-- component cost during production and nets to zero on completion.
INSERT INTO accounts (entity_id, code, name, account_type, parent_code, is_control, is_active, tags)
SELECT a.entity_id, '1510', 'Work in Progress', 'Asset', NULL, false, true, '[]'::jsonb
FROM accounts a
WHERE a.code = '1500'
  AND NOT EXISTS (SELECT 1 FROM accounts x WHERE x.entity_id = a.entity_id AND x.code = '1510');

-- Defensive: ensure Manufacturing Overhead (6300) exists for entities that have
-- the Cost of Sales parent (6000) but predate it in the template.
INSERT INTO accounts (entity_id, code, name, account_type, parent_code, is_control, is_active, tags)
SELECT a.entity_id, '6300', 'Manufacturing Overhead', 'Expense', '6000', false, true, '[]'::jsonb
FROM accounts a
WHERE a.code = '6000'
  AND NOT EXISTS (SELECT 1 FROM accounts x WHERE x.entity_id = a.entity_id AND x.code = '6300');
