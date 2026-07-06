-- P2P extensions: email-LPO stamp, approval spend-limits (DoA), purchase debit
-- notes (buyer-issued supplier returns), and staff expense claims.

-- ── Email the LPO to the vendor ─────────────────────────────────────────────
ALTER TABLE purchase_orders ADD COLUMN IF NOT EXISTS sent_at timestamptz;

-- ── Approval spend-limits / Delegation of Authority ─────────────────────────
-- Per-role ceiling on what a user may approve. NULL max_amount = unlimited.
CREATE TABLE IF NOT EXISTS approval_limits (
    entity_id  uuid NOT NULL,
    role       text NOT NULL,
    max_amount numeric,
    PRIMARY KEY (entity_id, role)
);

-- ── Purchase debit notes (buyer-issued returns / overcharge claims) ─────────
CREATE TABLE IF NOT EXISTS purchase_debit_notes (
    id              uuid PRIMARY KEY,
    entity_id       uuid NOT NULL,
    number          text NOT NULL,            -- DN-YYYY-####
    vendor_id       uuid NOT NULL,
    applies_to_bill uuid,                      -- bills(id)
    po_id           uuid,
    debit_note_date date NOT NULL,
    reason          text,
    currency        text NOT NULL DEFAULT 'KES',
    subtotal        numeric NOT NULL DEFAULT 0,
    tax_total       numeric NOT NULL DEFAULT 0,
    gross_total     numeric NOT NULL DEFAULT 0,
    status          text NOT NULL DEFAULT 'issued',
    created_by      uuid,
    created_at      timestamptz NOT NULL DEFAULT now()
);
CREATE INDEX IF NOT EXISTS idx_debit_notes_entity ON purchase_debit_notes(entity_id);
CREATE UNIQUE INDEX IF NOT EXISTS idx_debit_notes_number ON purchase_debit_notes(entity_id, number);

CREATE TABLE IF NOT EXISTS purchase_debit_note_lines (
    id             uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    debit_note_id  uuid NOT NULL REFERENCES purchase_debit_notes(id) ON DELETE CASCADE,
    description    text NOT NULL,
    quantity       numeric NOT NULL DEFAULT 1,
    unit_price     numeric NOT NULL DEFAULT 0,
    account_code   text,
    line_total     numeric NOT NULL DEFAULT 0,
    line_no        int NOT NULL DEFAULT 0
);

-- ── Expense claims (staff self-service reimbursement) ───────────────────────
CREATE TABLE IF NOT EXISTS expense_claims (
    id               uuid PRIMARY KEY,
    entity_id        uuid NOT NULL,
    number           text NOT NULL,           -- EXP-YYYY-####
    claimant_id      uuid NOT NULL,
    title            text NOT NULL,
    currency         text NOT NULL DEFAULT 'KES',
    total            numeric NOT NULL DEFAULT 0,
    status           text NOT NULL DEFAULT 'draft', -- draft|submitted|approved|rejected|reimbursed
    approved_by      uuid,
    approved_at      timestamptz,
    rejection_reason text,
    notes            text,
    created_at       timestamptz NOT NULL DEFAULT now()
);
CREATE INDEX IF NOT EXISTS idx_expense_claims_entity ON expense_claims(entity_id, status);
CREATE UNIQUE INDEX IF NOT EXISTS idx_expense_claims_number ON expense_claims(entity_id, number);

CREATE TABLE IF NOT EXISTS expense_claim_lines (
    id           uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    claim_id     uuid NOT NULL REFERENCES expense_claims(id) ON DELETE CASCADE,
    expense_date date,
    description  text NOT NULL,
    account_code text,
    amount       numeric NOT NULL DEFAULT 0,
    line_no      int NOT NULL DEFAULT 0
);
