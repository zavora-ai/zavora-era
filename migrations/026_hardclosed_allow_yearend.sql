-- Zavora ERP — Migration 026: allow year-end close/opening entries into hard-closed periods
--
-- The year-end close process hard-closes all 12 periods of a fiscal year, then
-- must post a closing entry into the (now hard-closed) last period and an opening
-- entry into the next year's first period. The original
-- `prevent_hardclosed_period_insert` trigger blocked ALL inserts into a
-- hard-closed period, which made year-end close impossible (it always failed at
-- the closing-entry step).
--
-- This redefines the trigger to permit exactly the two system-generated sources
-- used by the close process — YearEndClose and OpeningBalance — while still
-- blocking every other insert into a hard-closed period. `source` is stored as a
-- JSON string on journal_entries (e.g. '"YearEndClose"').
--
-- Idempotent (CREATE OR REPLACE).

CREATE OR REPLACE FUNCTION prevent_hardclosed_period_insert()
RETURNS TRIGGER AS $$
DECLARE
    period_status TEXT;
    entry_source TEXT;
BEGIN
    SELECT fp.status, je.source
      INTO period_status, entry_source
    FROM fiscal_periods fp
    JOIN journal_entries je ON je.period_id = fp.id
    WHERE je.id = NEW.entry_id;

    IF period_status = 'hard_closed'
       AND entry_source NOT IN ('"YearEndClose"', '"OpeningBalance"') THEN
        RAISE EXCEPTION 'Cannot insert journal lines into a hard-closed period (entry: %).', NEW.entry_id;
    END IF;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

-- Trigger binding is unchanged; CREATE OR REPLACE FUNCTION updates the body in place.
