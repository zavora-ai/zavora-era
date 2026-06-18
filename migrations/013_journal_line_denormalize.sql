-- Zavora ERP — Migration 013: denormalize entity_id + date onto journal_lines
--
-- Reporting aggregates journal_lines but the lines carry no tenant/date of their
-- own, so even a one-month report must scan every line and join to entries.
-- Copying entity_id and the entry date onto each line (immutable, written once
-- at posting) lets period and as-at aggregations run as index-only scans on a
-- covering index — no join to journal_entries.
--
-- Safe because a journal line's tenant and date never change after posting
-- (the entry is immutable). The one-time backfill temporarily disables the
-- posted-line guard, which only blocks post-hoc edits.
--
-- Idempotent.

ALTER TABLE journal_lines
    ADD COLUMN IF NOT EXISTS entity_id  UUID,
    ADD COLUMN IF NOT EXISTS entry_date DATE;

-- One-time backfill from the parent entry (guard blocks updates to posted lines).
ALTER TABLE journal_lines DISABLE TRIGGER trg_prevent_posted_line_update;
UPDATE journal_lines jl
   SET entity_id = je.entity_id,
       entry_date = je.date
  FROM journal_entries je
 WHERE jl.entry_id = je.id
   AND (jl.entity_id IS NULL OR jl.entry_date IS NULL);
ALTER TABLE journal_lines ENABLE TRIGGER trg_prevent_posted_line_update;

-- Covering index: period-range and as-at aggregations become index-only.
CREATE INDEX IF NOT EXISTS idx_jl_entity_date_account
    ON journal_lines (entity_id, entry_date, account_code)
    INCLUDE (functional_debit, functional_credit);
