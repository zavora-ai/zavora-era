-- Zavora ERP — M-Pesa webhook idempotency
-- Daraja delivers callbacks with at-least-once semantics, so retries must not
-- create duplicate payments. A unique receipt per entity lets the application
-- "claim" a receipt before recording a payment; a duplicate claim is rejected
-- by this constraint.

-- Replace the non-unique index with a unique one (per entity).
DROP INDEX IF EXISTS idx_mpesa_receipt;

CREATE UNIQUE INDEX idx_mpesa_receipt_unique
    ON mpesa_transactions(entity_id, receipt_number);
