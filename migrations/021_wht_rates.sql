-- Zavora ERP — Migration 021: configurable withholding-tax rates
--
-- WHT rates were hardcoded in the Rust `WhtCategory::rates()` match. That made
-- them a second source of truth alongside any future config and meant a rate
-- change needed a redeploy. This table is now the ONLY source of truth: the
-- runtime reads rates from here with no code fallback. Rates are national
-- (statutory), so the table is global, not per-entity.
--
-- Seeded once with the KRA defaults; edit the rows to change a rate.
-- Idempotent.

CREATE TABLE IF NOT EXISTS wht_rates (
    category          TEXT PRIMARY KEY,
    resident_rate     NUMERIC NOT NULL,
    non_resident_rate NUMERIC NOT NULL,
    updated_at        TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

INSERT INTO wht_rates (category, resident_rate, non_resident_rate) VALUES
    ('Consultancy',    0.05, 0.20),
    ('ManagementFees', 0.05, 0.20),
    ('Rent',           0.10, 0.30),
    ('Royalties',      0.05, 0.20),
    ('Interest',       0.15, 0.15),
    ('Contractual',    0.03, 0.20),
    ('Dividends',      0.05, 0.15),
    ('Insurance',      0.05, 0.20),
    ('Transport',      0.02, 0.20),
    ('Other',          0.05, 0.20)
ON CONFLICT (category) DO NOTHING;
