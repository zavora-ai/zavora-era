-- Zavora ERP — Migration 008: production-readiness schema foundation
--
-- P0 production readiness — tables and schema changes that downstream tasks
-- depend on (Req 1.4, 3.4, 6.1, 17.1, 20.1, 24.1, 26.2):
--   * era_users.last_login_at (auth session tracking; password_hash/status/
--     invited_at were already added in migration 006).
--   * VAT + General posting-group tables and (biz × prod) posting matrices.
--   * supplier_credit_note_lines (per-line supplier credit note posting).
--   * Posting-group foreign-key columns on customers / vendors / products.
--   * entity_settings.invoice_template (JSONB) for the template editor.
--   * entity_settings.last_fiscal_year_allocated for gapless year-reset numbering.
--   * Supporting indexes on the new tables/columns.
--
-- Note on indexes: the design's "migration 006" index set (entity_id+customer_id,
-- status, date, vendor_id, party_id, payment_date, account_code) was already
-- applied by migration 006_auth_and_indexes.sql, so this migration only adds the
-- indexes for the new posting-group and supplier-credit-note objects below.
--
-- Note on CONCURRENTLY: the design uses CREATE INDEX CONCURRENTLY, which cannot
-- run inside a transaction. sqlx wraps each migration in a transaction, so we use
-- plain CREATE INDEX IF NOT EXISTS here.
--
-- Idempotent: safe to re-run against a partially-migrated database.

-- ── era_users: last login timestamp ────────────────────────────────────────
ALTER TABLE era_users
    ADD COLUMN IF NOT EXISTS last_login_at TIMESTAMPTZ;

-- ── VAT posting groups + matrix (Req 17.1) ─────────────────────────────────
CREATE TABLE IF NOT EXISTS vat_business_groups (
    id          UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    entity_id   UUID NOT NULL,
    code        TEXT NOT NULL,
    name        TEXT NOT NULL,
    description TEXT,
    UNIQUE(entity_id, code)
);

CREATE TABLE IF NOT EXISTS vat_product_groups (
    id          UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    entity_id   UUID NOT NULL,
    code        TEXT NOT NULL,
    name        TEXT NOT NULL,
    description TEXT,
    UNIQUE(entity_id, code)
);

CREATE TABLE IF NOT EXISTS vat_posting_matrix (
    id                 UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    entity_id          UUID NOT NULL,
    vat_biz_group_id   UUID NOT NULL REFERENCES vat_business_groups(id),
    vat_prod_group_id  UUID NOT NULL REFERENCES vat_product_groups(id),
    vat_rate           NUMERIC(5,2) NOT NULL,
    vat_output_account TEXT NOT NULL,
    vat_input_account  TEXT NOT NULL,
    UNIQUE(entity_id, vat_biz_group_id, vat_prod_group_id)
);

-- ── General posting groups + matrix (Req 17.2) ─────────────────────────────
CREATE TABLE IF NOT EXISTS general_business_groups (
    id        UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    entity_id UUID NOT NULL,
    code      TEXT NOT NULL,
    name      TEXT NOT NULL,
    UNIQUE(entity_id, code)
);

CREATE TABLE IF NOT EXISTS general_product_groups (
    id        UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    entity_id UUID NOT NULL,
    code      TEXT NOT NULL,
    name      TEXT NOT NULL,
    UNIQUE(entity_id, code)
);

CREATE TABLE IF NOT EXISTS general_posting_matrix (
    id                UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    entity_id         UUID NOT NULL,
    gen_biz_group_id  UUID NOT NULL REFERENCES general_business_groups(id),
    gen_prod_group_id UUID NOT NULL REFERENCES general_product_groups(id),
    sales_account     TEXT NOT NULL,
    purchase_account  TEXT NOT NULL,
    cogs_account      TEXT,
    UNIQUE(entity_id, gen_biz_group_id, gen_prod_group_id)
);

-- Posting-group lookup indexes (entity-scoped resolver queries)
CREATE INDEX IF NOT EXISTS idx_vat_business_groups_entity
    ON vat_business_groups(entity_id);
CREATE INDEX IF NOT EXISTS idx_vat_product_groups_entity
    ON vat_product_groups(entity_id);
CREATE INDEX IF NOT EXISTS idx_vat_posting_matrix_lookup
    ON vat_posting_matrix(entity_id, vat_biz_group_id, vat_prod_group_id);
CREATE INDEX IF NOT EXISTS idx_general_business_groups_entity
    ON general_business_groups(entity_id);
CREATE INDEX IF NOT EXISTS idx_general_product_groups_entity
    ON general_product_groups(entity_id);
CREATE INDEX IF NOT EXISTS idx_general_posting_matrix_lookup
    ON general_posting_matrix(entity_id, gen_biz_group_id, gen_prod_group_id);

-- ── Supplier credit note line items (Req 20.1) ─────────────────────────────
CREATE TABLE IF NOT EXISTS supplier_credit_note_lines (
    id              UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    credit_note_id  UUID NOT NULL REFERENCES supplier_credit_notes(id) ON DELETE CASCADE,
    product_id      UUID,
    description     TEXT NOT NULL,
    quantity        NUMERIC(18,4) NOT NULL DEFAULT 1,
    unit_price      NUMERIC(18,2) NOT NULL,
    vat_treatment   TEXT NOT NULL DEFAULT 'standard_16',
    vat_amount      NUMERIC(18,2) NOT NULL DEFAULT 0,
    gl_account_code TEXT NOT NULL,
    line_total      NUMERIC(18,2) NOT NULL,
    sort_order      INTEGER NOT NULL DEFAULT 0
);

CREATE INDEX IF NOT EXISTS idx_scn_lines_cn
    ON supplier_credit_note_lines(credit_note_id);

-- ── Posting-group references on parties and products (Req 17.3) ────────────
ALTER TABLE customers
    ADD COLUMN IF NOT EXISTS vat_business_group_id     UUID,
    ADD COLUMN IF NOT EXISTS general_business_group_id UUID;
ALTER TABLE vendors
    ADD COLUMN IF NOT EXISTS vat_business_group_id     UUID,
    ADD COLUMN IF NOT EXISTS general_business_group_id UUID;
ALTER TABLE products
    ADD COLUMN IF NOT EXISTS vat_product_group_id     UUID,
    ADD COLUMN IF NOT EXISTS general_product_group_id UUID;

-- ── entity_settings: invoice template + fiscal-year tracking ───────────────
-- Invoice template editor storage (Req 26.2)
ALTER TABLE entity_settings
    ADD COLUMN IF NOT EXISTS invoice_template JSONB NOT NULL DEFAULT '{}'::jsonb;

-- Gapless document-numbering year reset tracking (Req 6.1)
ALTER TABLE entity_settings
    ADD COLUMN IF NOT EXISTS last_fiscal_year_allocated INTEGER;
