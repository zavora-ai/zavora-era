-- Zavora ERP — Migration 015: link journal entries back to their source document
--
-- Journal entries record a `source` *type* (Invoice, Bill, CreditNote, …) and a
-- `reference` *string* (the document number), but nothing that points to the
-- source document's row. Drill-down (General Ledger line → source document)
-- therefore had no id to navigate by. Storing the source document's id closes
-- the loop: GL line → journal entry → source document.
--
-- Nullable: manual journals, FX revaluations, depreciation and closing entries
-- have no single source document.
--
-- Idempotent.

ALTER TABLE journal_entries
    ADD COLUMN IF NOT EXISTS source_id UUID;

CREATE INDEX IF NOT EXISTS idx_je_source_id
    ON journal_entries (entity_id, source_id);

-- Backfill existing entries by matching the document number (reference) within
-- the entity. `source` is stored as a JSON string, e.g. '"Invoice"'. Document
-- numbers are unique per entity, so the join is unambiguous. Only fills NULLs.
--
-- The posted-journal guard blocks any update to a posted entry, so disable it
-- for this one-time, metadata-only backfill (it does not touch any amount,
-- account or date — only the new source_id link).
ALTER TABLE journal_entries DISABLE TRIGGER trg_prevent_posted_journal_update;

UPDATE journal_entries je
   SET source_id = i.id
  FROM invoices i
 WHERE je.source_id IS NULL
   AND je.entity_id = i.entity_id
   AND je.reference = i.number
   AND je.source IN ('"Invoice"', '"CreditNote"');

UPDATE journal_entries je
   SET source_id = b.id
  FROM bills b
 WHERE je.source_id IS NULL
   AND je.entity_id = b.entity_id
   AND je.reference = b.number
   AND je.source = '"Bill"';

UPDATE journal_entries je
   SET source_id = scn.id
  FROM supplier_credit_notes scn
 WHERE je.source_id IS NULL
   AND je.entity_id = scn.entity_id
   AND je.reference = scn.credit_note_number
   AND je.source = '"SupplierCreditNote"';

ALTER TABLE journal_entries ENABLE TRIGGER trg_prevent_posted_journal_update;
