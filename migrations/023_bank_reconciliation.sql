-- Zavora ERP — Migration 023: formal bank reconciliation
--
-- 1. Add reconciliation-metadata columns to journal_entries (the existing
--    confirm-match code already writes these, but they were never created — so
--    that path silently failed). These are metadata, not accounting data.
-- 2. Relax the posted-journal immutability trigger to permit updating ONLY the
--    reconciliation columns; every accounting field stays immutable.
-- 3. Add a bank_reconciliations table to record a completed (locked) rec.
-- Idempotent.

ALTER TABLE journal_entries ADD COLUMN IF NOT EXISTS reconciled    BOOLEAN NOT NULL DEFAULT false;
ALTER TABLE journal_entries ADD COLUMN IF NOT EXISTS reconciled_at TIMESTAMPTZ;

CREATE OR REPLACE FUNCTION prevent_posted_journal_update()
RETURNS TRIGGER AS $$
BEGIN
    IF OLD.status = 'posted' THEN
        -- Permit updating ONLY reconciliation metadata: if every accounting
        -- field is unchanged, the only difference is reconciled/reconciled_at.
        IF NEW.status     IS NOT DISTINCT FROM OLD.status
           AND NEW.number IS NOT DISTINCT FROM OLD.number
           AND NEW.date   IS NOT DISTINCT FROM OLD.date
           AND NEW.period_id   IS NOT DISTINCT FROM OLD.period_id
           AND NEW.source      IS NOT DISTINCT FROM OLD.source
           AND NEW.source_id   IS NOT DISTINCT FROM OLD.source_id
           AND NEW.reference   IS NOT DISTINCT FROM OLD.reference
           AND NEW.description IS NOT DISTINCT FROM OLD.description
           AND NEW.entity_id   IS NOT DISTINCT FROM OLD.entity_id THEN
            RETURN NEW;
        END IF;
        -- Otherwise a posted entry may only transition status -> reversed.
        IF NEW.status = 'reversed' THEN
            IF NEW.number != OLD.number OR NEW.date != OLD.date OR
               NEW.reference != OLD.reference OR NEW.description != OLD.description THEN
                RAISE EXCEPTION 'Only status may be updated on a posted journal entry (id: %).', OLD.id;
            END IF;
            RETURN NEW;
        END IF;
        RAISE EXCEPTION 'Cannot modify a posted journal entry (id: %). Use reversal instead.', OLD.id;
    END IF;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE TABLE IF NOT EXISTS bank_reconciliations (
    id                        UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    entity_id                 UUID NOT NULL,
    bank_account_id           UUID NOT NULL,
    statement_date            DATE NOT NULL,
    statement_closing_balance NUMERIC NOT NULL,
    gl_balance                NUMERIC NOT NULL,
    cleared_balance           NUMERIC NOT NULL,
    difference                NUMERIC NOT NULL,
    status                    TEXT NOT NULL DEFAULT 'completed',
    completed_by              UUID,
    completed_at              TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_bank_recs_account
    ON bank_reconciliations (entity_id, bank_account_id, statement_date);
