-- ============================================================
-- PROCUREMENT (P2P) + VENDOR PORTAL — Phase 1
-- Tender/RFQ → bid → award → LPO → lodged invoice → statement.
-- Vendors are an external principal class (vendor_users), isolated from
-- internal staff (era_users) by a distinct 'Vendor' JWT role.
-- ============================================================

-- ── External supplier logins (portal) ──────────────────────────────────────
CREATE TABLE IF NOT EXISTS vendor_users (
    id            UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    entity_id     UUID NOT NULL,                 -- the tenant they supply
    email         TEXT NOT NULL,
    display_name  TEXT NOT NULL,
    company_name  TEXT NOT NULL,
    kra_pin       TEXT,
    phone         TEXT,
    password_hash TEXT NOT NULL,
    status        TEXT NOT NULL DEFAULT 'pending', -- pending|active|suspended|rejected
    vendor_id     UUID,                            -- linked vendors master (on approval)
    last_login    TIMESTAMPTZ,
    created_at    TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE(entity_id, email)
);
CREATE INDEX IF NOT EXISTS idx_vendor_users_entity ON vendor_users(entity_id);
CREATE INDEX IF NOT EXISTS idx_vendor_users_status ON vendor_users(entity_id, status);

-- ── Tenders / RFQs ──────────────────────────────────────────────────────────
CREATE TABLE IF NOT EXISTS tenders (
    id           UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    entity_id    UUID NOT NULL,
    number       TEXT NOT NULL,
    title        TEXT NOT NULL,
    description  TEXT,
    category     TEXT,
    closing_date DATE,
    status       TEXT NOT NULL DEFAULT 'draft', -- draft|open|closed|awarded|cancelled
    created_by   UUID,
    created_at   TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
CREATE INDEX IF NOT EXISTS idx_tenders_entity ON tenders(entity_id, status);

CREATE TABLE IF NOT EXISTS tender_lines (
    id          UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    tender_id   UUID NOT NULL REFERENCES tenders(id) ON DELETE CASCADE,
    description TEXT NOT NULL,
    quantity    NUMERIC NOT NULL DEFAULT 1,
    uom         TEXT NOT NULL DEFAULT 'unit',
    line_no     INT NOT NULL DEFAULT 0
);
CREATE INDEX IF NOT EXISTS idx_tender_lines_tender ON tender_lines(tender_id);

-- ── Bids (vendor responses to a tender) ─────────────────────────────────────
CREATE TABLE IF NOT EXISTS bids (
    id           UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    entity_id    UUID NOT NULL,
    tender_id    UUID NOT NULL REFERENCES tenders(id) ON DELETE CASCADE,
    vendor_id    UUID NOT NULL,
    currency     CHAR(3) NOT NULL DEFAULT 'KES',
    total_amount NUMERIC NOT NULL DEFAULT 0,
    notes        TEXT,
    status       TEXT NOT NULL DEFAULT 'submitted', -- submitted|shortlisted|awarded|rejected|withdrawn
    submitted_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE(tender_id, vendor_id)
);
CREATE INDEX IF NOT EXISTS idx_bids_entity ON bids(entity_id);
CREATE INDEX IF NOT EXISTS idx_bids_tender ON bids(tender_id);
CREATE INDEX IF NOT EXISTS idx_bids_vendor ON bids(entity_id, vendor_id);

CREATE TABLE IF NOT EXISTS bid_lines (
    id             UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    bid_id         UUID NOT NULL REFERENCES bids(id) ON DELETE CASCADE,
    tender_line_id UUID,
    description    TEXT NOT NULL,
    quantity       NUMERIC NOT NULL DEFAULT 1,
    unit_price     NUMERIC NOT NULL DEFAULT 0,
    amount         NUMERIC NOT NULL DEFAULT 0,
    line_no        INT NOT NULL DEFAULT 0
);
CREATE INDEX IF NOT EXISTS idx_bid_lines_bid ON bid_lines(bid_id);

-- ── Purchase orders (LPO) ───────────────────────────────────────────────────
CREATE TABLE IF NOT EXISTS purchase_orders (
    id            UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    entity_id     UUID NOT NULL,
    number        TEXT NOT NULL,
    vendor_id     UUID NOT NULL,
    tender_id     UUID,
    bid_id        UUID,
    currency      CHAR(3) NOT NULL DEFAULT 'KES',
    fx_rate       NUMERIC NOT NULL DEFAULT 1,
    subtotal      NUMERIC NOT NULL DEFAULT 0,
    tax_total     NUMERIC NOT NULL DEFAULT 0,
    gross_total   NUMERIC NOT NULL DEFAULT 0,
    status        TEXT NOT NULL DEFAULT 'issued', -- issued|acknowledged|partially_invoiced|invoiced|closed|cancelled
    issue_date    DATE NOT NULL DEFAULT CURRENT_DATE,
    delivery_date DATE,
    notes         TEXT,
    created_by    UUID,
    created_at    TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
CREATE INDEX IF NOT EXISTS idx_po_entity ON purchase_orders(entity_id, status);
CREATE INDEX IF NOT EXISTS idx_po_vendor ON purchase_orders(entity_id, vendor_id);

CREATE TABLE IF NOT EXISTS purchase_order_lines (
    id            UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    po_id         UUID NOT NULL REFERENCES purchase_orders(id) ON DELETE CASCADE,
    description   TEXT NOT NULL,
    quantity      NUMERIC NOT NULL DEFAULT 1,
    uom           TEXT NOT NULL DEFAULT 'unit',
    unit_price    NUMERIC NOT NULL DEFAULT 0,
    tax_treatment TEXT,
    account_code  TEXT,
    line_total    NUMERIC NOT NULL DEFAULT 0,
    line_no       INT NOT NULL DEFAULT 0
);
CREATE INDEX IF NOT EXISTS idx_po_lines_po ON purchase_order_lines(po_id);

-- ── Link a lodged vendor invoice (bill) back to its LPO (Phase-2 3-way match) ─
ALTER TABLE bills ADD COLUMN IF NOT EXISTS po_id UUID;
CREATE INDEX IF NOT EXISTS idx_bills_po ON bills(po_id) WHERE po_id IS NOT NULL;
