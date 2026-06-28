-- Zavora ERP — Migration 030: control accounts on general business groups
--
-- BC "specific posting groups": a customer/vendor business group also carries the
-- balance-sheet control accounts (A/R for customers, A/P for vendors), so posting
-- can route receivables/payables by group instead of one flat account for all.
--
-- Stored on general_business_groups (the "who you deal with" dimension). NULL =
-- fall back to the per-record account, then the flat PostingSetup.

ALTER TABLE general_business_groups
    ADD COLUMN IF NOT EXISTS receivables_account TEXT;

ALTER TABLE general_business_groups
    ADD COLUMN IF NOT EXISTS payables_account TEXT;
