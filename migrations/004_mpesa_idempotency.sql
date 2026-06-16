-- Zavora ERP — M-Pesa webhook idempotency
-- A unique receipt per entity prevents duplicate payment creation from
-- at-least-once Daraja callbacks.

-- Drop old non-unique index if it exists, then create unique one.
DROP INDEX IF EXISTS idx_mpesa_receipt;

CREATE UNIQUE INDEX IF NOT EXISTS idx_mpesa_receipt_unique
    ON mpesa_transactions(entity_id, receipt_number);
