-- 053: Paystack card-payment transactions (idempotency + reconciliation).
--
-- One row per Paystack charge we initialise or receive a webhook for. The
-- unique (entity_id, reference) claim makes duplicate `charge.success`
-- webhooks (Paystack retries) safe — the same pattern mpesa_transactions uses.
CREATE TABLE IF NOT EXISTS paystack_transactions (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    entity_id UUID NOT NULL,
    reference TEXT NOT NULL,
    invoice_id UUID,
    amount NUMERIC NOT NULL DEFAULT 0,
    currency TEXT NOT NULL DEFAULT 'KES',
    customer_email TEXT,
    status TEXT NOT NULL DEFAULT 'initialized', -- initialized | success | failed
    authorization_url TEXT,
    payment_id UUID,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (entity_id, reference)
);

CREATE INDEX IF NOT EXISTS idx_paystack_tx_entity ON paystack_transactions(entity_id);
CREATE INDEX IF NOT EXISTS idx_paystack_tx_invoice ON paystack_transactions(invoice_id);
