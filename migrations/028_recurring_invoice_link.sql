-- Zavora ERP — Migration 028: link generated invoices back to their recurring template
--
-- Recurring invoices generate ordinary invoices on a schedule, but there was no
-- way to tell which invoices a given recurring template produced (only a
-- run_count). This adds a nullable back-reference so a recurring invoice can show
-- its real generated-invoice history.

ALTER TABLE invoices
    ADD COLUMN IF NOT EXISTS recurring_invoice_id UUID;

CREATE INDEX IF NOT EXISTS idx_invoices_recurring
    ON invoices (entity_id, recurring_invoice_id)
    WHERE recurring_invoice_id IS NOT NULL;
