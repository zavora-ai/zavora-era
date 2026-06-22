-- Zavora ERP — Migration 024: tax filings + remittance
--
-- Records that a tax return (VAT/PAYE/WHT) was filed for a period and the
-- remittance paid to KRA, so the ledger reflects tax actually paid (not just the
-- reportable position). Idempotent.

CREATE TABLE IF NOT EXISTS tax_filings (
    id                    UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    entity_id             UUID NOT NULL,
    tax_type              TEXT NOT NULL,            -- VAT | PAYE | WHT
    period_from           DATE NOT NULL,
    period_to             DATE NOT NULL,
    amount                NUMERIC NOT NULL,
    status                TEXT NOT NULL DEFAULT 'filed',  -- filed | remitted
    remittance_journal_id UUID,
    remitted_at           TIMESTAMPTZ,
    filed_by              UUID,
    filed_at              TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_tax_filings_entity
    ON tax_filings (entity_id, tax_type, period_to);
