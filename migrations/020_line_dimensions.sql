-- Zavora ERP — Migration 020: dimensions on document lines
--
-- Lets invoice and bill lines carry analytical dimensions ({ type_code: value })
-- which propagate to the journal lines at posting, so dimensional reporting is
-- driven by real revenue/expense documents, not only manual journals.
-- Idempotent.

ALTER TABLE invoice_lines ADD COLUMN IF NOT EXISTS dimensions JSONB NOT NULL DEFAULT '{}'::jsonb;
ALTER TABLE bill_lines    ADD COLUMN IF NOT EXISTS dimensions JSONB NOT NULL DEFAULT '{}'::jsonb;
