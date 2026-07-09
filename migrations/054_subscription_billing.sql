-- 054: Subscription billing via Paystack.
--
-- paystack_transactions now distinguishes an invoice charge (a customer paying
-- their bill) from a SUBSCRIPTION charge (a tenant paying for their Zavora
-- plan at signup / renewal), and records which plan the subscription is for.
ALTER TABLE paystack_transactions
    ADD COLUMN IF NOT EXISTS purpose TEXT NOT NULL DEFAULT 'invoice', -- 'invoice' | 'subscription'
    ADD COLUMN IF NOT EXISTS plan TEXT;

-- Per-tenant subscription state (plan, status, paid-through date) lives in a
-- dedicated JSONB column on entity_settings so it's read with the rest of the
-- config. Shape: { plan, status: 'trialing'|'active'|'past_due', current_period_end, updated_at }.
ALTER TABLE entity_settings
    ADD COLUMN IF NOT EXISTS subscription JSONB NOT NULL DEFAULT '{}'::jsonb;
