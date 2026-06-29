-- Zavora ERP — Migration 034: link stock movements to their journal entries
--
-- Standalone inventory receive/issue now post a GL journal (DR Inventory / CR
-- GRNI clearing on receipt; DR COGS / CR Inventory on issue). This column lets a
-- stock movement reference the journal entry it produced, so the inventory
-- subledger and the general ledger are auditably tied together.
--
-- Nullable: movements made by invoice posting (where the invoice owns the
-- journal) and pre-existing rows leave it NULL.

ALTER TABLE stock_movements
    ADD COLUMN IF NOT EXISTS journal_entry_id UUID;
