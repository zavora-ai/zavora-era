-- Goods Receipt Notes (GRN) — record what actually arrived against a PO. The
-- receiving leg of P2P: it turns "ordered" into "received", and is the middle
-- document of the 3-way match (PO ↔ GRN ↔ invoice) that gates bill approval.

CREATE TABLE IF NOT EXISTS goods_receipts (
    id           uuid PRIMARY KEY,
    entity_id    uuid NOT NULL,
    number       text NOT NULL,                 -- GRN-YYYY-####
    po_id        uuid NOT NULL REFERENCES purchase_orders(id) ON DELETE CASCADE,
    receipt_date date NOT NULL,
    received_by  uuid,
    notes        text,
    created_at   timestamptz NOT NULL DEFAULT now()
);
CREATE INDEX IF NOT EXISTS idx_goods_receipts_po ON goods_receipts(po_id);
CREATE UNIQUE INDEX IF NOT EXISTS idx_goods_receipts_number ON goods_receipts(entity_id, number);

CREATE TABLE IF NOT EXISTS goods_receipt_lines (
    id                uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    grn_id            uuid NOT NULL REFERENCES goods_receipts(id) ON DELETE CASCADE,
    po_line_id        uuid,                      -- purchase_order_lines(id)
    description       text NOT NULL,
    quantity_received numeric NOT NULL DEFAULT 0,
    line_no           int NOT NULL DEFAULT 0
);
CREATE INDEX IF NOT EXISTS idx_goods_receipt_lines_grn ON goods_receipt_lines(grn_id);
