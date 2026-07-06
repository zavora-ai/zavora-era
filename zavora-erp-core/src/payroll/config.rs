//! Effective-dated statutory configuration for Kenyan payroll.
//!
//! Replaces the previously-hardcoded PAYE bands, NSSF tiers, SHA and Housing
//! Levy rates, and reliefs with a versioned, per-tenant config so a historical
//! pay run is exactly reproducible. `StatutoryConfig::finance_act_2024()` returns
//! the built-in default (identical to the former constants); tenants may store
//! effective-dated overrides in `payroll_statutory_config`.

use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use serde::{Deserialize, Serialize};

/// A PAYE band: everything up to `upper` (exclusive of the prior band) is taxed
/// at `rate`. `upper = None` marks the open-ended top band.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PayeBand {
    pub upper: Option<Decimal>,
    pub rate: Decimal,
}

/// The complete statutory ruleset applied to a pay period.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StatutoryConfig {
    pub name: String,
    pub paye_bands: Vec<PayeBand>,
    pub personal_relief: Decimal,
    pub insurance_relief_cap: Decimal,
    pub disability_exemption: Decimal,
    pub nssf_tier1_limit: Decimal,
    pub nssf_tier2_limit: Decimal,
    pub nssf_rate: Decimal,
    pub sha_rate: Decimal,
    /// Minimum monthly SHA contribution (0 disables the floor).
    pub sha_minimum: Decimal,
    pub housing_rate: Decimal,
    /// NITA levy per employee per month (employer cost; 0 disables).
    pub nita_per_employee: Decimal,
}

impl Default for StatutoryConfig {
    fn default() -> Self {
        Self::finance_act_2024()
    }
}

impl StatutoryConfig {
    /// Built-in default — identical to the former hardcoded constants
    /// (KRA Finance Act 2024, effective 2025/2026). `sha_minimum`/`nita` default
    /// to 0 so behaviour is byte-for-byte unchanged; set them via a tenant
    /// override to enable the SHA floor / NITA levy.
    pub fn finance_act_2024() -> Self {
        Self {
            name: "Finance Act 2024".to_string(),
            paye_bands: vec![
                PayeBand { upper: Some(dec!(24_000)), rate: dec!(0.10) },
                PayeBand { upper: Some(dec!(32_333)), rate: dec!(0.25) },
                PayeBand { upper: Some(dec!(500_000)), rate: dec!(0.30) },
                PayeBand { upper: Some(dec!(800_000)), rate: dec!(0.325) },
                PayeBand { upper: None, rate: dec!(0.35) },
            ],
            personal_relief: dec!(2_400),
            insurance_relief_cap: dec!(5_000),
            disability_exemption: dec!(150_000),
            nssf_tier1_limit: dec!(7_000),
            nssf_tier2_limit: dec!(36_000),
            nssf_rate: dec!(0.06),
            sha_rate: dec!(0.0275),
            sha_minimum: dec!(0),
            housing_rate: dec!(0.015),
            nita_per_employee: dec!(0),
        }
    }

    /// Progressive PAYE on monthly taxable income.
    pub fn compute_paye(&self, monthly_taxable_income: Decimal) -> Decimal {
        let mut remaining = monthly_taxable_income;
        let mut tax = Decimal::ZERO;
        let mut prev_upper = Decimal::ZERO;

        for band in &self.paye_bands {
            if remaining <= Decimal::ZERO {
                break;
            }
            let band_width = match band.upper {
                Some(u) => u - prev_upper,
                None => remaining, // open-ended top band absorbs the rest
            };
            let taxable_in_band = remaining.min(band_width);
            tax += taxable_in_band * band.rate;
            remaining -= taxable_in_band;
            prev_upper = band.upper.unwrap_or(prev_upper);
        }
        tax.max(Decimal::ZERO)
    }

    /// Employee NSSF: `rate` of gross capped at the tier-II limit.
    pub fn nssf_employee(&self, gross: Decimal) -> Decimal {
        (gross.min(self.nssf_tier2_limit) * self.nssf_rate).round_dp(2)
    }

    /// Employer NSSF (matches employee).
    pub fn nssf_employer(&self, gross: Decimal) -> Decimal {
        self.nssf_employee(gross)
    }

    /// SHA contribution (with optional monthly floor).
    pub fn sha(&self, gross: Decimal) -> Decimal {
        (gross * self.sha_rate).round_dp(2).max(self.sha_minimum)
    }

    /// Employee Housing Levy.
    pub fn housing_employee(&self, gross: Decimal) -> Decimal {
        (gross * self.housing_rate).round_dp(2)
    }

    /// Employer Housing Levy (matches employee).
    pub fn housing_employer(&self, gross: Decimal) -> Decimal {
        self.housing_employee(gross)
    }
}
