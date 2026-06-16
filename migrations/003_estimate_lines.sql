-- Zavora ERP — Estimate line items
-- Mirrors the invoice_lines structure but keyed on estimate_id.

CREATE TABLE IF NOT EXISTS estimate_lines (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    estimate_id UUID NOT NULL REFERENCES estimates(id),
    product_id UUID REFERENCES products(id),
    description TEXT NOT NULL DEFAULT '',
    quantity NUMERIC NOT NULL DEFAULT 1,
    unit_price NUMERIC NOT NULL DEFAULT 0,
    discount_percent NUMERIC NOT NULL DEFAULT 0,
    account_code TEXT NOT NULL,
    vat_treatment TEXT NOT NULL DEFAULT 'Standard16',
    line_total NUMERIC NOT NULL DEFAULT 0,
    vat_amount NUMERIC NOT NULL DEFAULT 0
);

CREATE INDEX IF NOT EXISTS idx_estimate_lines_estimate ON estimate_lines(estimate_id);
