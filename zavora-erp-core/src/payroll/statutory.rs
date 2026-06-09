use rust_decimal::Decimal;
use rust_decimal_macros::dec;

/// Kenya PAYE tax bands (Finance Act 2024, effective 2025/2026).
/// Monthly bands.
pub struct PayeBands;

impl PayeBands {
    /// Calculate monthly PAYE from taxable income.
    /// Uses progressive tax bands as per KRA.
    pub fn compute_paye(monthly_taxable_income: Decimal) -> Decimal {
        let bands: Vec<(Decimal, Decimal, Decimal)> = vec![
            // (upper_limit, rate, cumulative_tax_at_start_of_band)
            (dec!(24_000), dec!(0.10), dec!(0)),
            (dec!(32_333), dec!(0.25), dec!(2_400)),
            (dec!(500_000), dec!(0.30), dec!(4_483.25)),
            (dec!(800_000), dec!(0.325), dec!(144_583.25)),
            (Decimal::MAX, dec!(0.35), dec!(242_083.25)),
        ];

        let mut remaining = monthly_taxable_income;
        let mut tax = Decimal::ZERO;
        let mut prev_upper = Decimal::ZERO;

        for (upper, rate, _) in &bands {
            let band_width = *upper - prev_upper;
            if remaining <= Decimal::ZERO {
                break;
            }
            let taxable_in_band = remaining.min(band_width);
            tax += taxable_in_band * rate;
            remaining -= taxable_in_band;
            prev_upper = *upper;
        }

        tax.max(Decimal::ZERO)
    }

    /// Personal relief (KES 2,400 per month as of 2026).
    pub fn personal_relief() -> Decimal {
        dec!(2_400)
    }

    /// Insurance relief cap (KES 5,000 per month).
    pub fn insurance_relief_cap() -> Decimal {
        dec!(5_000)
    }

    /// Disability exemption (KES 150,000 per month).
    pub fn disability_exemption() -> Decimal {
        dec!(150_000)
    }
}

/// NSSF computation (Tier I and Tier II).
pub struct NssfComputation;

impl NssfComputation {
    /// NSSF Tier I upper limit.
    pub fn tier1_limit() -> Decimal {
        dec!(7_000)
    }

    /// NSSF Tier II upper limit.
    pub fn tier2_limit() -> Decimal {
        dec!(36_000)
    }

    /// Employee NSSF rate (6%).
    pub fn rate() -> Decimal {
        dec!(0.06)
    }

    /// Compute employee NSSF contribution from gross salary.
    pub fn compute_employee(gross: Decimal) -> Decimal {
        let pensionable = gross.min(Self::tier2_limit());
        (pensionable * Self::rate()).round_dp(2)
    }

    /// Compute employer NSSF contribution (matching).
    pub fn compute_employer(gross: Decimal) -> Decimal {
        Self::compute_employee(gross) // Employer matches employee
    }
}

/// SHA (Social Health Authority, replaces NHIF) computation.
pub struct ShaComputation;

impl ShaComputation {
    /// SHA rate (2.75% of gross).
    pub fn rate() -> Decimal {
        dec!(0.0275)
    }

    /// Compute SHA contribution.
    pub fn compute(gross: Decimal) -> Decimal {
        (gross * Self::rate()).round_dp(2)
    }
}

/// Housing Levy computation.
pub struct HousingLevyComputation;

impl HousingLevyComputation {
    /// Housing levy rate (1.5% of gross).
    pub fn rate() -> Decimal {
        dec!(0.015)
    }

    /// Compute employee housing levy.
    pub fn compute_employee(gross: Decimal) -> Decimal {
        (gross * Self::rate()).round_dp(2)
    }

    /// Compute employer housing levy (matching 1.5%).
    pub fn compute_employer(gross: Decimal) -> Decimal {
        Self::compute_employee(gross)
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

/// Compute full payslip deductions for an employee.
pub fn compute_payslip_deductions(
    gross_salary: Decimal,
    allowances_total: Decimal,
    helb_deduction: Decimal,
    personal_relief: Decimal,
    disability_exemption: bool,
) -> PayslipDeductions {
    let total_gross = gross_salary + allowances_total;

    // NSSF is deducted before PAYE computation
    let nssf_employee = NssfComputation::compute_employee(total_gross);
    let nssf_employer = NssfComputation::compute_employer(total_gross);

    // Housing levy
    let housing_levy_employee = HousingLevyComputation::compute_employee(total_gross);
    let housing_levy_employer = HousingLevyComputation::compute_employer(total_gross);

    // Taxable income = gross - NSSF employee - Housing levy employee
    let mut taxable_income = total_gross - nssf_employee - housing_levy_employee;

    // Disability exemption
    if disability_exemption {
        taxable_income = (taxable_income - PayeBands::disability_exemption()).max(Decimal::ZERO);
    }

    // Compute PAYE
    let paye = PayeBands::compute_paye(taxable_income);

    // Apply reliefs
    let net_paye = (paye - personal_relief).max(Decimal::ZERO);

    // SHA
    let sha = ShaComputation::compute(total_gross);

    // Total deductions
    let total_deductions = net_paye + nssf_employee + sha + housing_levy_employee + helb_deduction;

    // Net salary
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
