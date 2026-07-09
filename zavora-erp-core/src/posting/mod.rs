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

pub mod groups;

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
    /// Unapplied customer receipts (overpayments held on account — a liability:
    /// we owe the customer money or goods).
    pub unapplied_payments: String,
    /// Unapplied vendor payments (overpayments to a vendor — an asset: the
    /// vendor owes us the excess back, a prepayment in substance).
    pub unapplied_vendor_credits: String,

    // --- Tax ---
    /// VAT Output (payable) — charged on sales.
    pub vat_output: String,
    /// VAT Input (claimable) — incurred on purchases.
    pub vat_input: String,
    /// Withholding Tax payable to KRA (WHT this entity withholds from vendors).
    pub wht_payable: String,
    /// Withholding Tax receivable — WHT credits withheld by customers from this
    /// entity's income (a prepaid income-tax asset).
    pub wht_receivable: String,

    // --- Foreign exchange ---
    pub realised_fx_gain: String,
    pub realised_fx_loss: String,
    pub unrealised_fx_gain: String,
    pub unrealised_fx_loss: String,

    // --- Equity / period close ---
    pub retained_earnings: String,

    /// Account that absorbs sub-cent rounding differences when VAT line
    /// accumulation leaves a journal entry imbalanced by <= 0.01 (Req 5.3).
    pub rounding_adjustment: String,

    // --- Cash ---
    /// Fallback bank/cash account when none is specified on a bank account.
    pub default_bank: String,

    // --- Default income / expense (used when no product or master override) ---
    pub default_sales: String,
    pub default_purchase: String,
    pub default_expense: String,

    // --- Inventory ---
    /// Inventory asset (stock on hand) control account.
    pub inventory_asset: String,
    /// Cost of goods sold.
    pub cost_of_goods_sold: String,
    /// Goods-Received-Not-Invoiced clearing. Credited when stock is received
    /// without a vendor bill (standalone receipt); the later bill debits it.
    pub inventory_clearing: String,

    // --- Fixed assets ---
    /// Fixed-asset (cost) control account.
    pub fixed_asset: String,
    /// Accumulated depreciation (contra-asset).
    pub accumulated_depreciation: String,
    /// Depreciation expense.
    pub depreciation_expense: String,

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
            // Unapplied customer receipts (overpayments / on-account) post here.
            // Must be a seeded account, else overpayments orphan and break the
            // trial balance — "9100 Unapplied Customer Payments" is the seeded
            // liability for this.
            unapplied_payments: "9100".to_string(),
            // Vendor overpayments are an asset (the vendor owes the excess
            // back) — "9110 Unapplied Vendor Credits" is the seeded asset.
            unapplied_vendor_credits: "9110".to_string(),
            vat_output: "3100".to_string(),
            vat_input: "1300".to_string(),
            wht_payable: "3210".to_string(),
            wht_receivable: "1310".to_string(),
            realised_fx_gain: "8120".to_string(),
            realised_fx_loss: "8130".to_string(),
            unrealised_fx_gain: "8100".to_string(),
            unrealised_fx_loss: "8110".to_string(),
            retained_earnings: "4600".to_string(),
            // Sub-cent rounding differences. Defaults to the miscellaneous
            // expense account; an accountant can point this at a dedicated
            // "Rounding" GL account via the posting-setup UI.
            rounding_adjustment: "7900".to_string(),
            default_bank: "1020".to_string(),
            // Services-first defaults (Zavora): sales → Service Revenue, purchases →
            // Software/Cloud/Subscriptions. Goods sellers can repoint these in Settings.
            default_sales: "5100".to_string(),
            default_purchase: "7350".to_string(),
            default_expense: "7900".to_string(),
            // Stock on hand. 1500 is the seeded Inventory asset (1300 is VAT
            // Input — posting stock there understated inventory and corrupted
            // the VAT control).
            inventory_asset: "1500".to_string(),
            cost_of_goods_sold: "6000".to_string(),
            // Goods received not invoiced — the seeded GRNI accrual account.
            // Must NOT be the AP control (3010): receipts credited there looked
            // like vendor balances with no vendor subledger behind them.
            inventory_clearing: "3020".to_string(),
            fixed_asset: "2500".to_string(),
            accumulated_depreciation: "2600".to_string(),
            depreciation_expense: "7600".to_string(),
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
