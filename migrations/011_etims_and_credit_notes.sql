-- Zavora ERP — Migration 011: eTIMS status + supplier credit note line support
--
-- Kenya 2026 practice (KRA eTIMS): every tax document is either NOT yet
-- transmitted to KRA (freely editable/voidable) or already transmitted
-- (immutable — corrections only via a credit note that references the original).
-- We model that with an `etims_status` lifecycle flag on each tax document.
--
--   etims_status ∈ { 'not_transmitted', 'transmitted', 'transmission_failed' }
--
-- This migration:
--   * Adds eTIMS transmission tracking to invoices/credit notes and bills.
--   * Records the supplier's eTIMS invoice number on bills ("no invoice, no
--     deduction" — purchases need a compliant supplier invoice to be claimable).
--   * Extends supplier_credit_notes with the columns the line-item-aware service
--     needs (subtotal/tax_total/reason) and an etims_status.
--   * Adds `source_estimate` to invoices so estimate→invoice conversion can link
--     back to the originating quote.
--
-- Idempotent: safe to re-run.

-- ── Invoices / credit notes (shared `invoices` table) ──────────────────────
ALTER TABLE invoices
    ADD COLUMN IF NOT EXISTS etims_status         TEXT NOT NULL DEFAULT 'not_transmitted',
    ADD COLUMN IF NOT EXISTS etims_invoice_number TEXT,
    ADD COLUMN IF NOT EXISTS etims_transmitted_at TIMESTAMPTZ,
    ADD COLUMN IF NOT EXISTS source_estimate      UUID;

-- ── Bills (purchases) ──────────────────────────────────────────────────────
-- For bills the tax invoice is issued by the SUPPLIER; we capture their eTIMS
-- invoice number for deductibility and track whether it is recorded.
ALTER TABLE bills
    ADD COLUMN IF NOT EXISTS etims_status                  TEXT NOT NULL DEFAULT 'not_transmitted',
    ADD COLUMN IF NOT EXISTS supplier_etims_invoice_number TEXT,
    ADD COLUMN IF NOT EXISTS etims_transmitted_at          TIMESTAMPTZ;

-- ── Supplier credit notes ──────────────────────────────────────────────────
ALTER TABLE supplier_credit_notes
    ADD COLUMN IF NOT EXISTS subtotal     NUMERIC NOT NULL DEFAULT 0,
    ADD COLUMN IF NOT EXISTS tax_total    NUMERIC NOT NULL DEFAULT 0,
    ADD COLUMN IF NOT EXISTS reason       TEXT,
    ADD COLUMN IF NOT EXISTS currency     CHAR(3) NOT NULL DEFAULT 'KES',
    ADD COLUMN IF NOT EXISTS fx_rate      NUMERIC NOT NULL DEFAULT 1,
    ADD COLUMN IF NOT EXISTS etims_status TEXT NOT NULL DEFAULT 'not_transmitted';

-- Helpful lookup indexes
CREATE INDEX IF NOT EXISTS idx_invoices_etims_status ON invoices(entity_id, etims_status);
CREATE INDEX IF NOT EXISTS idx_scn_entity_vendor ON supplier_credit_notes(entity_id, vendor_id);
