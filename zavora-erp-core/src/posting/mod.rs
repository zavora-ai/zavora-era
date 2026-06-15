//! Posting setup — centralised GL account determination.
//!
//! Phase 1 introduces a single default **posting setup** per entity that replaces
//! the hardcoded account-code constants previously scattered across the posting
//! services (payments, invoicing, payroll, fx, period close).
//!
//! Every posting path resolves its control/clearing/default accounts from this
//! struct instead of literals, so a custom chart of accounts or a second entity
//! no longer silently mis-posts.
//!
//! Later phases will layer posting-group *dimensions* (customer / vendor / product /
//! VAT business + product groups) resolved through setup matrices on top of this
//! struct; this type is the seam those matrices will plug into.

use serde::{Deserialize, Serialize};

/// Resolved GL account codes for an entity. Defaults mirror the Kenya Standard
/// chart of accounts seeded by `ledger::coa_template`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct PostingSetup {
    // --- Control / subledger ---
    /// Accounts Receivable control (Trade Debtors).
    pub accounts_receivable: String,
    /// Accounts Payable control (Trade Creditors).
    pub accounts_payable: String,
    /// Unapplied payments clearing account.
    pub unapplied_payments: String,

    // --- Tax ---
    /// VAT Output (payable) — charged on sales.
    pub vat_output: String,
    /// VAT Input (claimable) — incurred on purchases.
    pub vat_input: String,
    /// Withholding Tax payable to KRA.
    pub wht_payable: String,

    // --- Foreign exchange ---
    pub realised_fx_gain: String,
    pub realised_fx_loss: String,
    pub unrealised_fx_gain: String,
    pub unrealised_fx_loss: String,

    // --- Equity / period close ---
    pub retained_earnings: String,

    // --- Cash ---
    /// Fallback bank/cash account when none is specified on a bank account.
    pub default_bank: String,

    // --- Default income / expense (used when no product or master override) ---
    pub default_sales: String,
    pub default_purchase: String,
    pub default_expense: String,

    // --- Payroll ---
    pub salaries_expense: String,
    pub nssf_employer_expense: String,
    pub housing_levy_employer_expense: String,
    pub paye_payable: String,
    pub nssf_payable: String,
    pub sha_payable: String,
    pub helb_payable: String,
    pub housing_levy_payable: String,
    /// Net pay due to employees (wages payable / accrued).
    pub net_pay_payable: String,
}

impl Default for PostingSetup {
    fn default() -> Self {
        Self {
            accounts_receivable: "1200".to_string(),
            accounts_payable: "3010".to_string(),
            // NOTE: preserves the existing (pre-Phase-1) behaviour. "3050" is not
            // present in the seeded Kenya CoA — this is a known mismapping that the
            // posting-setup UI will let an accountant correct (candidates: 1700 /
            // 9100 for customer, 3600 / 9110 for vendor).
            unapplied_payments: "3050".to_string(),
            vat_output: "3100".to_string(),
            vat_input: "1300".to_string(),
            wht_payable: "3210".to_string(),
            realised_fx_gain: "8120".to_string(),
            realised_fx_loss: "8130".to_string(),
            unrealised_fx_gain: "8100".to_string(),
            unrealised_fx_loss: "8110".to_string(),
            retained_earnings: "4600".to_string(),
            default_bank: "1020".to_string(),
            default_sales: "5000".to_string(),
            default_purchase: "6000".to_string(),
            default_expense: "7900".to_string(),
            salaries_expense: "7010".to_string(),
            nssf_employer_expense: "7020".to_string(),
            housing_levy_employer_expense: "7030".to_string(),
            paye_payable: "3310".to_string(),
            nssf_payable: "3320".to_string(),
            sha_payable: "3330".to_string(),
            helb_payable: "3340".to_string(),
            housing_levy_payable: "3350".to_string(),
            net_pay_payable: "3400".to_string(),
        }
    }
}
