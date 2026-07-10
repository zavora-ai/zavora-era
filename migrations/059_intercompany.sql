-- 059: Intercompany accounting + group consolidation (multi-company usage).
--
-- Dedicated intercompany control accounts let a charge between two group
-- companies post to BOTH ledgers with mirror balances, so consolidation can
-- eliminate them precisely (IC Receivable ↔ IC Payable, IC Income ↔ IC Charges)
-- rather than by a KRA-PIN heuristic.

-- 1) Seed the four IC control accounts into EVERY existing entity (idempotent).
INSERT INTO accounts (entity_id, code, name, account_type, parent_code, is_control, is_active)
SELECT s.entity_id, v.code, v.name, v.atype, v.parent, v.ctrl, true
FROM entity_settings s
CROSS JOIN (VALUES
    ('1250', 'Intercompany Receivable', 'Asset',     '1100', true),
    ('3030', 'Intercompany Payable',    'Liability', '3000', true),
    ('5180', 'Intercompany Income',     'Revenue',   NULL,   false),
    ('7160', 'Intercompany Charges',    'Expense',   '7000', false)
) AS v(code, name, atype, parent, ctrl)
ON CONFLICT (entity_id, code) DO NOTHING;

-- 2) A company group is a set of entities consolidated together, with one parent.
CREATE TABLE IF NOT EXISTS company_groups (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    name TEXT NOT NULL,
    presentation_currency CHAR(3) NOT NULL DEFAULT 'KES',
    created_by UUID,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE TABLE IF NOT EXISTS company_group_members (
    group_id UUID NOT NULL REFERENCES company_groups(id) ON DELETE CASCADE,
    entity_id UUID NOT NULL,
    is_parent BOOLEAN NOT NULL DEFAULT false,
    -- Ownership the group holds in this member (100 = wholly owned). Drives the
    -- non-controlling interest memo on consolidation.
    ownership_pct NUMERIC(6,3) NOT NULL DEFAULT 100,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (group_id, entity_id)
);
CREATE INDEX IF NOT EXISTS idx_group_members_entity ON company_group_members(entity_id);

-- 3) An intercompany transaction links the two mirrored journal entries it
-- posted (one per company), so it is auditable and reversible as a unit.
CREATE TABLE IF NOT EXISTS intercompany_transactions (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    group_id UUID REFERENCES company_groups(id) ON DELETE SET NULL,
    from_entity_id UUID NOT NULL,   -- the company that charges / lends (recognises IC receivable + income)
    to_entity_id UUID NOT NULL,     -- the company charged / borrowing (recognises IC charges + payable)
    amount NUMERIC(20,2) NOT NULL,
    currency CHAR(3) NOT NULL DEFAULT 'KES',
    tx_date DATE NOT NULL,
    description TEXT NOT NULL DEFAULT '',
    from_journal_id UUID,
    to_journal_id UUID,
    created_by UUID,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
CREATE INDEX IF NOT EXISTS idx_ic_tx_from ON intercompany_transactions(from_entity_id);
CREATE INDEX IF NOT EXISTS idx_ic_tx_to ON intercompany_transactions(to_entity_id);
CREATE INDEX IF NOT EXISTS idx_ic_tx_group ON intercompany_transactions(group_id);
