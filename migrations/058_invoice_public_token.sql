-- 058: public pay-link token for invoices.
--
-- A random, unguessable token lets a customer open a public invoice page (no
-- login) to view and pay the invoice. It is deliberately distinct from the
-- invoice `id` so a shareable pay link can't be derived from — or used to probe
-- — internal identifiers. Opening the public page stamps `viewed_at`.
ALTER TABLE invoices ADD COLUMN IF NOT EXISTS public_token TEXT;

-- Backfill existing invoices with a token.
UPDATE invoices
   SET public_token = replace(gen_random_uuid()::text, '-', '')
 WHERE public_token IS NULL;

-- New invoices get one automatically (the service INSERTs don't set it).
ALTER TABLE invoices
  ALTER COLUMN public_token SET DEFAULT replace(gen_random_uuid()::text, '-', '');

CREATE UNIQUE INDEX IF NOT EXISTS idx_invoices_public_token ON invoices(public_token);
