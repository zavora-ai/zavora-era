-- Zavora ERP — Migration 049: eTIMS OSCU/VSCU integration
--
-- KRA's eTIMS OSCU (Online Sales Control Unit) / VSCU (Virtual SCU) API lets an
-- ERP transmit tax invoices to KRA in real time and receive back a signed
-- receipt (SCU id, receipt number, internal data + signature) plus a QR the
-- buyer can verify. This migration adds:
--   * etims_devices        — per-entity device credentials + init state + the
--                            monotonic invoice counter KRA requires per branch.
--   * SCU receipt columns   on invoices to store what KRA signs and returns.
--   * etims_item_registry   — tracks which products have been registered with KRA
--                            (item code + KRA item-classification code).

CREATE TABLE IF NOT EXISTS etims_devices (
    entity_id       UUID PRIMARY KEY,
    enabled         BOOLEAN NOT NULL DEFAULT FALSE,
    -- 'sandbox' | 'production' — selects the KRA base URL.
    environment     TEXT NOT NULL DEFAULT 'sandbox',
    pin             TEXT,                       -- taxpayer PIN (tin)
    bhf_id          TEXT NOT NULL DEFAULT '00', -- branch id
    dvc_srl_no      TEXT,                       -- device serial number
    -- Returned by device initialisation:
    sdc_id          TEXT,
    mrc_no          TEXT,
    cmc_key         TEXT,                       -- communication key for auth headers
    initialized     BOOLEAN NOT NULL DEFAULT FALSE,
    initialized_at  TIMESTAMPTZ,
    -- KRA requires a strictly increasing invoice number per branch.
    last_invc_no    BIGINT NOT NULL DEFAULT 0,
    last_error      TEXT,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT now()
);

ALTER TABLE invoices
    ADD COLUMN IF NOT EXISTS etims_invc_no      BIGINT,  -- the sequential no. we sent KRA
    ADD COLUMN IF NOT EXISTS etims_rcpt_no      BIGINT,  -- curRcptNo from KRA
    ADD COLUMN IF NOT EXISTS etims_tot_rcpt_no  BIGINT,  -- totRcptNo from KRA
    ADD COLUMN IF NOT EXISTS etims_sdc_id       TEXT,
    ADD COLUMN IF NOT EXISTS etims_mrc_no       TEXT,
    ADD COLUMN IF NOT EXISTS etims_rcpt_sign    TEXT,    -- SCU signature
    ADD COLUMN IF NOT EXISTS etims_intrl_data   TEXT,    -- SCU internal data
    ADD COLUMN IF NOT EXISTS etims_vsdc_date    TEXT,    -- vsdcRcptPbctDate
    ADD COLUMN IF NOT EXISTS etims_qr_url       TEXT,    -- buyer verification URL
    ADD COLUMN IF NOT EXISTS etims_error        TEXT;    -- last transmission error

CREATE TABLE IF NOT EXISTS etims_item_registry (
    id            UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    entity_id     UUID NOT NULL,
    product_id    UUID NOT NULL,
    item_cd       TEXT NOT NULL,          -- our item code sent to KRA
    item_cls_cd   TEXT NOT NULL,          -- KRA item classification code
    registered    BOOLEAN NOT NULL DEFAULT FALSE,
    registered_at TIMESTAMPTZ,
    last_error    TEXT,
    created_at    TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (entity_id, product_id)
);

CREATE INDEX IF NOT EXISTS idx_etims_item_registry_entity ON etims_item_registry(entity_id);
