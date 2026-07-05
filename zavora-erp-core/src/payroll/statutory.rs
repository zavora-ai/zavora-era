//! Kenyan statutory payroll math. The numeric rules now live in
//! [`crate::payroll::config::StatutoryConfig`] (effective-dated, per-tenant); the
//! types here are thin, backward-compatible wrappers that delegate to the
//! built-in `finance_act_2024()` default, plus the payslip assembly.

use rust_decimal::Decimal;

use super::config::StatutoryConfig;

fn default_cfg() -> StatutoryConfig {
    StatutoryConfig::finance_act_2024()
}

/// Kenya PAYE tax bands. Backward-compatible facade over `StatutoryConfig`.
pub struct PayeBands;

impl PayeBands {
    /// Calculate monthly PAYE from taxable income (built-in default config).
    pub fn compute_paye(monthly_taxable_income: Decimal) -> Decimal {
        default_cfg().compute_paye(monthly_taxable_income)
    }

    /// Personal relief (KES 2,400 per month as of 2026).
    pub fn personal_relief() -> Decimal {
        default_cfg().personal_relief
    }

    /// Insurance relief cap (KES 5,000 per month).
    pub fn insurance_relief_cap() -> Decimal {
        default_cfg().insurance_relief_cap
    }

    /// Disability exemption (KES 150,000 per month).
    pub fn disability_exemption() -> Decimal {
        default_cfg().disability_exemption
    }
}

/// NSSF computation (Tier I and Tier II). Facade over `StatutoryConfig`.
pub struct NssfComputation;

impl NssfComputation {
    pub fn tier1_limit() -> Decimal {
        default_cfg().nssf_tier1_limit
    }
    pub fn tier2_limit() -> Decimal {
        default_cfg().nssf_tier2_limit
    }
    pub fn rate() -> Decimal {
        default_cfg().nssf_rate
    }
    pub fn compute_employee(gross: Decimal) -> Decimal {
        default_cfg().nssf_employee(gross)
    }
    pub fn compute_employer(gross: Decimal) -> Decimal {
        default_cfg().nssf_employer(gross)
    }
}

/// SHA (Social Health Authority) computation. Facade over `StatutoryConfig`.
pub struct ShaComputation;

impl ShaComputation {
    pub fn rate() -> Decimal {
        default_cfg().sha_rate
    }
    pub fn compute(gross: Decimal) -> Decimal {
        default_cfg().sha(gross)
    }
}

/// Housing Levy computation. Facade over `StatutoryConfig`.
pub struct HousingLevyComputation;

impl HousingLevyComputation {
    pub fn rate() -> Decimal {
        default_cfg().housing_rate
    }
    pub fn compute_employee(gross: Decimal) -> Decimal {
        default_cfg().housing_employee(gross)
    }
    pub fn compute_employer(gross: Decimal) -> Decimal {
        default_cfg().housing_employer(gross)
    }
}

/// Complete payslip deduction breakdown.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PayslipDeductions {
    pub gross_salary: Decimal,
    pub taxable_income: Decimal,
    pub paye: Decimal,
    pub personal_relief: Decimal,
    pub insurance_relief: Decimal,
    pub net_paye: Decimal,
    pub nssf_employee: Decimal,
    pub nssf_employer: Decimal,
    pub sha: Decimal,
    pub housing_levy_employee: Decimal,
    pub housing_levy_employer: Decimal,
    pub helb: Decimal,
    pub total_deductions: Decimal,
    pub net_salary: Decimal,
}

/// Compute full payslip deductions using the built-in default statutory config.
/// Retained for backward compatibility; the payroll engine uses
/// [`compute_payslip_deductions_cfg`] with an effective-dated config.
pub fn compute_payslip_deductions(
    gross_salary: Decimal,
    allowances_total: Decimal,
    helb_deduction: Decimal,
    personal_relief: Decimal,
    disability_exemption: bool,
) -> PayslipDeductions {
    compute_payslip_deductions_cfg(
        &default_cfg(),
        gross_salary,
        allowances_total,
        helb_deduction,
        personal_relief,
        disability_exemption,
    )
}

/// Compute full payslip deductions against a specific statutory config.
///
/// `taxable_allowances` are added to the PAYE/SHA/NSSF/housing base;
/// `pre_tax_deductions` (e.g. pension contributions) reduce taxable income.
/// This entry point keeps the classic behaviour when the extra bases are zero.
pub fn compute_payslip_deductions_cfg(
    cfg: &StatutoryConfig,
    gross_salary: Decimal,
    allowances_total: Decimal,
    helb_deduction: Decimal,
    personal_relief: Decimal,
    disability_exemption: bool,
) -> PayslipDeductions {
    let total_gross = gross_salary + allowances_total;

    // NSSF is deducted before PAYE computation.
    let nssf_employee = cfg.nssf_employee(total_gross);
    let nssf_employer = cfg.nssf_employer(total_gross);

    // Housing levy.
    let housing_levy_employee = cfg.housing_employee(total_gross);
    let housing_levy_employer = cfg.housing_employer(total_gross);

    // Taxable income = gross - NSSF employee - Housing levy employee.
    let mut taxable_income = total_gross - nssf_employee - housing_levy_employee;

    if disability_exemption {
        taxable_income = (taxable_income - cfg.disability_exemption).max(Decimal::ZERO);
    }

    let paye = cfg.compute_paye(taxable_income);
    let net_paye = (paye - personal_relief).max(Decimal::ZERO);
    let sha = cfg.sha(total_gross);

    let total_deductions = net_paye + nssf_employee + sha + housing_levy_employee + helb_deduction;
    let net_salary = total_gross - total_deductions;

    PayslipDeductions {
        gross_salary: total_gross,
        taxable_income,
        paye,
        personal_relief,
        insurance_relief: Decimal::ZERO,
        net_paye,
        nssf_employee,
        nssf_employer,
        sha,
        housing_levy_employee,
        housing_levy_employer,
        helb: helb_deduction,
        total_deductions,
        net_salary,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    #[test]
    fn paye_first_band_is_ten_percent() {
        assert_eq!(PayeBands::compute_paye(dec!(20_000)), dec!(2_000));
        assert_eq!(PayeBands::compute_paye(dec!(24_000)), dec!(2_400));
    }

    #[test]
    fn paye_crosses_second_band() {
        assert_eq!(PayeBands::compute_paye(dec!(32_333)), dec!(4_483.25));
    }

    #[test]
    fn paye_third_band_known_value() {
        assert_eq!(PayeBands::compute_paye(dec!(47_090)), dec!(8_910.35));
    }

    #[test]
    fn nssf_caps_at_tier_two_limit() {
        assert_eq!(NssfComputation::compute_employee(dec!(30_000)), dec!(1_800));
        assert_eq!(NssfComputation::compute_employee(dec!(36_000)), dec!(2_160));
        assert_eq!(NssfComputation::compute_employee(dec!(100_000)), dec!(2_160));
        assert_eq!(
            NssfComputation::compute_employer(dec!(100_000)),
            NssfComputation::compute_employee(dec!(100_000))
        );
    }

    #[test]
    fn sha_is_two_point_seventy_five_percent() {
        assert_eq!(ShaComputation::compute(dec!(50_000)), dec!(1_375));
        assert_eq!(ShaComputation::compute(dec!(100_000)), dec!(2_750));
    }

    #[test]
    fn housing_levy_is_one_point_five_percent_both_sides() {
        assert_eq!(HousingLevyComputation::compute_employee(dec!(50_000)), dec!(750));
        assert_eq!(HousingLevyComputation::compute_employer(dec!(50_000)), dec!(750));
    }

    #[test]
    fn full_payslip_known_values() {
        let p = compute_payslip_deductions(
            dec!(50_000),
            Decimal::ZERO,
            Decimal::ZERO,
            PayeBands::personal_relief(),
            false,
        );
        assert_eq!(p.nssf_employee, dec!(2_160));
        assert_eq!(p.housing_levy_employee, dec!(750));
        assert_eq!(p.sha, dec!(1_375));
        assert_eq!(p.taxable_income, dec!(47_090));
        assert_eq!(p.paye, dec!(8_910.35));
        assert_eq!(p.net_paye, dec!(6_510.35));
        assert_eq!(p.total_deductions, dec!(6_510.35) + dec!(2_160) + dec!(1_375) + dec!(750));
        assert_eq!(p.net_salary, dec!(50_000) - p.total_deductions);
    }

    #[test]
    fn low_income_has_no_net_paye_after_relief() {
        let p = compute_payslip_deductions(
            dec!(20_000),
            Decimal::ZERO,
            Decimal::ZERO,
            PayeBands::personal_relief(),
            false,
        );
        assert_eq!(p.net_paye, Decimal::ZERO);
    }

    #[test]
    fn config_roundtrips_through_json() {
        let cfg = StatutoryConfig::finance_act_2024();
        let json = serde_json::to_value(&cfg).unwrap();
        let back: StatutoryConfig = serde_json::from_value(json).unwrap();
        // PAYE via the round-tripped config matches the facade.
        assert_eq!(back.compute_paye(dec!(47_090)), PayeBands::compute_paye(dec!(47_090)));
    }
}
