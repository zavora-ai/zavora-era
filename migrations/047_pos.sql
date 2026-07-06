-- Point of Sale: register/shift sessions and the sales they capture. A POS sale
-- reuses the existing spine — it posts an invoice (revenue + VAT + stock issue +
-- COGS) and records a payment (cash/M-Pesa) — and links it to an open shift so
-- the till can be reconciled with a Z-report at close.

CREATE TABLE IF NOT EXISTS pos_sessions (
    id               uuid PRIMARY KEY,
    entity_id        uuid NOT NULL,
    register_name    text NOT NULL DEFAULT 'Main Till',
    opened_by        uuid NOT NULL,
    opened_at        timestamptz NOT NULL DEFAULT now(),
    opening_float    numeric NOT NULL DEFAULT 0,
    cash_account_id  uuid,      -- bank_accounts row cash is deposited to
    mpesa_account_id uuid,      -- bank_accounts row M-Pesa is deposited to
    closed_by        uuid,
    closed_at        timestamptz,
    counted_cash     numeric,   -- physically counted at close
    expected_cash    numeric,   -- opening_float + cash sales
    cash_variance    numeric,   -- counted − expected
    status           text NOT NULL DEFAULT 'open',  -- open | closed
    notes            text
);
CREATE INDEX IF NOT EXISTS idx_pos_sessions_entity ON pos_sessions(entity_id, status);

CREATE TABLE IF NOT EXISTS pos_sales (
    id         uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    entity_id  uuid NOT NULL,
    session_id uuid NOT NULL REFERENCES pos_sessions(id) ON DELETE CASCADE,
    invoice_id uuid,
    payment_id uuid,
    tender     text NOT NULL,          -- cash | mpesa | card
    amount     numeric NOT NULL DEFAULT 0,
    created_at timestamptz NOT NULL DEFAULT now()
);
CREATE INDEX IF NOT EXISTS idx_pos_sales_session ON pos_sales(session_id);
