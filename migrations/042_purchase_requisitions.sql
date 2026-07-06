-- Purchase requisitions: the self-service front-door of P2P. Staff raise an
-- internal request to buy → it is routed for approval → an approved requisition
-- is converted by a buyer into a tender (competitive) or a direct purchase order.

CREATE TABLE IF NOT EXISTS purchase_requisitions (
    id                uuid PRIMARY KEY,
    entity_id         uuid NOT NULL,
    number            text NOT NULL,               -- PR-YYYY-####
    title             text NOT NULL,
    justification     text,                         -- why it's needed (business case)
    department        text,
    cost_center       text,
    currency          text NOT NULL DEFAULT 'KES',
    needed_by         date,
    estimated_total   numeric NOT NULL DEFAULT 0,
    -- draft → submitted → approved | rejected → converted ; or cancelled
    status            text NOT NULL DEFAULT 'draft',
    requested_by      uuid NOT NULL,
    approved_by       uuid,
    approved_at       timestamptz,
    rejection_reason  text,
    converted_to_type text,                         -- 'tender' | 'purchase_order'
    converted_to_id   uuid,
    notes             text,
    created_at        timestamptz NOT NULL DEFAULT now()
);
CREATE INDEX IF NOT EXISTS idx_requisitions_entity ON purchase_requisitions(entity_id, status);
CREATE UNIQUE INDEX IF NOT EXISTS idx_requisitions_number ON purchase_requisitions(entity_id, number);

CREATE TABLE IF NOT EXISTS purchase_requisition_lines (
    id                   uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    pr_id                uuid NOT NULL REFERENCES purchase_requisitions(id) ON DELETE CASCADE,
    description          text NOT NULL,
    quantity             numeric NOT NULL DEFAULT 1,
    uom                  text NOT NULL DEFAULT 'unit',
    estimated_unit_price numeric NOT NULL DEFAULT 0,
    account_code         text,
    line_total           numeric NOT NULL DEFAULT 0,
    line_no              int NOT NULL DEFAULT 0
);
CREATE INDEX IF NOT EXISTS idx_requisition_lines_pr ON purchase_requisition_lines(pr_id);

-- Traceability: link the resulting sourcing doc back to its requisition.
ALTER TABLE tenders         ADD COLUMN IF NOT EXISTS requisition_id uuid;
ALTER TABLE purchase_orders ADD COLUMN IF NOT EXISTS requisition_id uuid;
