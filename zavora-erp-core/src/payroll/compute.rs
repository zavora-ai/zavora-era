//! Input-aware payslip computation.
//!
//! Pure function that turns a set of **earnings** (basic + allowances + recurring
//! + per-run inputs) and **deductions** (voluntary/loan/welfare + per-run inputs)
//! into a full statutory breakdown against a [`StatutoryConfig`]. Honours each
//! earning's `taxable` flag and each deduction's `pre_tax` flag, and itemizes the
//! payslip so the register, payslip PDF, and statutory schedules can render lines.
//!
//! Proration (joiner/leaver, unpaid leave) is applied by the caller to the
//! relevant amounts before calling here, keeping this function pure/testable.

use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

use super::config::StatutoryConfig;

/// A single earning line on a payslip.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EarningLine {
    pub code: Option<String>,
    pub name: String,
    pub amount: Decimal,
    /// Included in PAYE taxable pay.
    pub taxable: bool,
    /// Included in the NSSF pensionable base.
    pub pensionable: bool,
    /// Included in the SHA / Housing Levy base.
    pub affects_shif: bool,
}

impl EarningLine {
    /// A fully-statutory earning (the common case: basic pay, standard allowances).
    pub fn standard(name: impl Into<String>, amount: Decimal, taxable: bool) -> Self {
        Self { code: None, name: name.into(), amount, taxable, pensionable: true, affects_shif: true }
    }
}

/// A single deduction line on a payslip (excludes the statutory PAYE/NSSF/SHA/HL/HELB,
/// which are computed).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeductionLine {
    pub code: Option<String>,
    pub name: String,
    pub amount: Decimal,
    /// Reduces taxable income (e.g. registered pension, mortgage interest).
    pub pre_tax: bool,
    /// statutory | voluntary | loan | welfare
    pub category: String,
}

/// The complete, itemized result of a payslip computation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComputedPayslip {
    pub gross: Decimal,
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
    /// Sum of the non-statutory deduction lines (voluntary/loan/welfare).
    pub other_deductions: Decimal,
    pub total_deductions: Decimal,
    pub net_salary: Decimal,
    /// Full employer cost = gross + employer NSSF + employer housing + NITA.
    pub employer_cost: Decimal,
    pub nita: Decimal,
    pub earnings: Vec<EarningLine>,
    pub deductions: Vec<DeductionLine>,
}

/// Inputs to a single payslip computation.
pub struct PayrollInputs {
    /// Basic pay (already prorated by the caller if applicable).
    pub basic_salary: Decimal,
    pub earnings: Vec<EarningLine>,
    pub deductions: Vec<DeductionLine>,
    pub helb: Decimal,
    pub personal_relief: Decimal,
    pub insurance_relief: Decimal,
    pub disability_exemption: bool,
}

/// Compute a full payslip breakdown for one employee against `cfg`.
pub fn compute_payslip(cfg: &StatutoryConfig, inp: &PayrollInputs) -> ComputedPayslip {
    // Basic pay is fully statutory (taxable, pensionable, affects SHIF).
    let mut earnings = Vec::with_capacity(inp.earnings.len() + 1);
    earnings.push(EarningLine::standard("Basic Pay", inp.basic_salary, true));
    earnings.extend(inp.earnings.iter().cloned());

    let gross: Decimal = earnings.iter().map(|e| e.amount).sum();
    let pensionable_base: Decimal = earnings.iter().filter(|e| e.pensionable).map(|e| e.amount).sum();
    let shif_base: Decimal = earnings.iter().filter(|e| e.affects_shif).map(|e| e.amount).sum();
    let taxable_earnings: Decimal = earnings.iter().filter(|e| e.taxable).map(|e| e.amount).sum();

    let nssf_employee = cfg.nssf_employee(pensionable_base);
    let nssf_employer = cfg.nssf_employer(pensionable_base);
    let housing_levy_employee = cfg.housing_employee(shif_base);
    let housing_levy_employer = cfg.housing_employer(shif_base);
    let sha = cfg.sha(shif_base);

    let pre_tax_deductions: Decimal =
        inp.deductions.iter().filter(|d| d.pre_tax).map(|d| d.amount).sum();

    // Taxable income = taxable earnings − NSSF(ee) − housing(ee) − pre-tax deductions.
    let mut taxable_income =
        taxable_earnings - nssf_employee - housing_levy_employee - pre_tax_deductions;
    if inp.disability_exemption {
        taxable_income = (taxable_income - cfg.disability_exemption).max(Decimal::ZERO);
    }
    taxable_income = taxable_income.max(Decimal::ZERO);

    let paye = cfg.compute_paye(taxable_income);
    let insurance_relief = inp.insurance_relief.min(cfg.insurance_relief_cap);
    let net_paye = (paye - inp.personal_relief - insurance_relief).max(Decimal::ZERO);

    let other_deductions: Decimal = inp.deductions.iter().map(|d| d.amount).sum();
    let total_deductions =
        (net_paye + nssf_employee + sha + housing_levy_employee + inp.helb + other_deductions).round_dp(2);
    let net_salary = (gross - total_deductions).round_dp(2);

    let nita = cfg.nita_per_employee;
    let employer_cost = (gross + nssf_employer + housing_levy_employer + nita).round_dp(2);

    ComputedPayslip {
        gross: gross.round_dp(2),
        taxable_income: taxable_income.round_dp(2),
        paye: paye.round_dp(2),
        personal_relief: inp.personal_relief,
        insurance_relief,
        net_paye: net_paye.round_dp(2),
        nssf_employee,
        nssf_employer,
        sha,
        housing_levy_employee,
        housing_levy_employer,
        helb: inp.helb,
        other_deductions,
        total_deductions,
        net_salary,
        employer_cost,
        nita,
        earnings,
        deductions: inp.deductions.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    fn cfg() -> StatutoryConfig {
        StatutoryConfig::finance_act_2024()
    }

    #[test]
    fn matches_legacy_for_basic_plus_taxable_allowance() {
        // 80,000 basic + 20,000 taxable housing = 100,000 gross (the UI review case).
        let inp = PayrollInputs {
            basic_salary: dec!(80_000),
            earnings: vec![EarningLine::standard("Housing", dec!(20_000), true)],
            deductions: vec![],
            helb: Decimal::ZERO,
            personal_relief: dec!(2_400),
            insurance_relief: Decimal::ZERO,
            disability_exemption: false,
        };
        let p = compute_payslip(&cfg(), &inp);
        assert_eq!(p.gross, dec!(100_000));
        assert_eq!(p.nssf_employee, dec!(2_160));
        assert_eq!(p.housing_levy_employee, dec!(1_500));
        assert_eq!(p.sha, dec!(2_750));
        // taxable = 100,000 - 2,160 - 1,500 = 96,340
        assert_eq!(p.taxable_income, dec!(96_340));
        // PAYE: 2,400 + 2,083.25 + (96,340-32,333)*0.30 = 23,685.35
        assert_eq!(p.paye, dec!(23_685.35));
        assert_eq!(p.net_paye, dec!(21_285.35));
        assert_eq!(p.net_salary, dec!(72_304.65));
        // employer cost = 100,000 + 2,160 + 1,500 (nita 0) = 103,660
        assert_eq!(p.employer_cost, dec!(103_660));
    }

    #[test]
    fn non_taxable_allowance_is_excluded_from_paye_but_paid() {
        // 50,000 basic + 10,000 non-taxable reimbursement.
        let inp = PayrollInputs {
            basic_salary: dec!(50_000),
            earnings: vec![EarningLine {
                code: None, name: "Reimbursement".into(), amount: dec!(10_000),
                taxable: false, pensionable: false, affects_shif: false,
            }],
            deductions: vec![],
            helb: Decimal::ZERO,
            personal_relief: dec!(2_400),
            insurance_relief: Decimal::ZERO,
            disability_exemption: false,
        };
        let p = compute_payslip(&cfg(), &inp);
        assert_eq!(p.gross, dec!(60_000));
        // Statutory bases exclude the non-taxable, non-pensionable line → same as 50,000 salary.
        assert_eq!(p.nssf_employee, dec!(2_160));
        assert_eq!(p.sha, dec!(1_375));
        assert_eq!(p.housing_levy_employee, dec!(750));
        assert_eq!(p.taxable_income, dec!(47_090));
        assert_eq!(p.paye, dec!(8_910.35));
        // Net = 60,000 - (net_paye 6,510.35 + 2,160 + 1,375 + 750) = 49,204.65
        assert_eq!(p.net_salary, dec!(60_000) - (dec!(6_510.35) + dec!(2_160) + dec!(1_375) + dec!(750)));
    }

    #[test]
    fn pre_tax_deduction_reduces_taxable_income() {
        // 50,000 basic, 5,000 pre-tax pension contribution.
        let inp = PayrollInputs {
            basic_salary: dec!(50_000),
            earnings: vec![],
            deductions: vec![DeductionLine {
                code: Some("PENSION".into()), name: "Pension".into(), amount: dec!(5_000),
                pre_tax: true, category: "voluntary".into(),
            }],
            helb: Decimal::ZERO,
            personal_relief: dec!(2_400),
            insurance_relief: Decimal::ZERO,
            disability_exemption: false,
        };
        let p = compute_payslip(&cfg(), &inp);
        // taxable = 50,000 - 2,160 - 750 - 5,000 = 42,090
        assert_eq!(p.taxable_income, dec!(42_090));
        // net includes the 5,000 pension deduction from pay.
        assert_eq!(p.other_deductions, dec!(5_000));
        assert_eq!(p.net_salary, dec!(50_000) - p.total_deductions);
    }

    #[test]
    fn loan_installment_reduces_net_only() {
        let inp = PayrollInputs {
            basic_salary: dec!(50_000),
            earnings: vec![],
            deductions: vec![DeductionLine {
                code: None, name: "Staff Loan".into(), amount: dec!(4_000),
                pre_tax: false, category: "loan".into(),
            }],
            helb: Decimal::ZERO,
            personal_relief: dec!(2_400),
            insurance_relief: Decimal::ZERO,
            disability_exemption: false,
        };
        let p = compute_payslip(&cfg(), &inp);
        // Loan does not change tax.
        assert_eq!(p.taxable_income, dec!(47_090));
        assert_eq!(p.other_deductions, dec!(4_000));
    }
}
