-- Bill lines table (mirrors invoice_lines for AP documents)
CREATE TABLE IF NOT EXISTS bill_lines (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    bill_id UUID NOT NULL REFERENCES bills(id) ON DELETE CASCADE,
    product_id UUID,
    description TEXT NOT NULL DEFAULT '',
    quantity NUMERIC NOT NULL DEFAULT 1,
    unit_price NUMERIC NOT NULL DEFAULT 0,
    discount_percent NUMERIC NOT NULL DEFAULT 0,
    account_code TEXT NOT NULL DEFAULT '7900',
    vat_treatment TEXT NOT NULL DEFAULT 'Standard16',
    line_total NUMERIC NOT NULL DEFAULT 0,
    vat_amount NUMERIC NOT NULL DEFAULT 0
);

CREATE INDEX IF NOT EXISTS idx_bill_lines_bill ON bill_lines(bill_id);
