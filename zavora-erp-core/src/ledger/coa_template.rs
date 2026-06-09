use serde::{Deserialize, Serialize};

use super::account::{AccountType, CreateAccountRequest};

/// Available chart of accounts templates.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum CoaTemplate {
    /// Kenya standard COA as defined in spec section 4.2
    KenyaStandard,
    /// Minimal COA for small businesses
    Minimal,
    /// Custom — no template accounts seeded
    Custom,
}

/// Returns the Kenya Standard chart of accounts template.
/// Code ranges per spec section 4.2.
pub fn kenya_standard_coa() -> Vec<CreateAccountRequest> {
    vec![
        // === Current Assets (1000–1999) ===
        acct("1000", "Cash and Bank", AccountType::Asset, None, true),
        acct("1010", "Petty Cash", AccountType::Asset, Some("1000"), false),
        acct("1020", "Main Bank Account - KES", AccountType::Asset, Some("1000"), false),
        acct("1030", "M-Pesa Float", AccountType::Asset, Some("1000"), false),
        acct("1040", "Bank Account - USD", AccountType::Asset, Some("1000"), false),
        acct("1100", "Accounts Receivable", AccountType::Asset, None, true),
        acct("1200", "Trade Debtors", AccountType::Asset, Some("1100"), true),
        acct("1300", "VAT Input (Claimable)", AccountType::Asset, None, false),
        acct("1400", "Prepaid Expenses", AccountType::Asset, None, false),
        acct("1500", "Inventory", AccountType::Asset, None, false),
        acct("1600", "Other Current Assets", AccountType::Asset, None, false),
        acct("1700", "Unapplied Customer Payments", AccountType::Asset, None, false),
        // === Non-Current Assets (2000–2499) ===
        acct("2000", "Non-Current Assets", AccountType::Asset, None, true),
        acct("2100", "Long-term Investments", AccountType::Asset, Some("2000"), false),
        acct("2200", "Long-term Receivables", AccountType::Asset, Some("2000"), false),
        // === Fixed Assets & Depreciation (2500–2999) ===
        acct("2500", "Fixed Assets", AccountType::Asset, None, true),
        acct("2510", "Land and Buildings", AccountType::Asset, Some("2500"), false),
        acct("2520", "Motor Vehicles", AccountType::Asset, Some("2500"), false),
        acct("2530", "Plant and Machinery", AccountType::Asset, Some("2500"), false),
        acct("2540", "Furniture and Fittings", AccountType::Asset, Some("2500"), false),
        acct("2550", "Computer Equipment", AccountType::Asset, Some("2500"), false),
        acct("2600", "Accumulated Depreciation", AccountType::ContraAsset, None, true),
        acct("2610", "Acc. Depr. - Buildings", AccountType::ContraAsset, Some("2600"), false),
        acct("2620", "Acc. Depr. - Motor Vehicles", AccountType::ContraAsset, Some("2600"), false),
        acct("2630", "Acc. Depr. - Plant & Machinery", AccountType::ContraAsset, Some("2600"), false),
        acct("2640", "Acc. Depr. - Furniture", AccountType::ContraAsset, Some("2600"), false),
        acct("2650", "Acc. Depr. - Computers", AccountType::ContraAsset, Some("2600"), false),
        // === Current Liabilities (3000–3999) ===
        acct("3000", "Accounts Payable", AccountType::Liability, None, true),
        acct("3010", "Trade Creditors", AccountType::Liability, Some("3000"), true),
        acct("3100", "VAT Output (Payable)", AccountType::Liability, None, false),
        acct("3200", "WHT Payable", AccountType::Liability, None, false),
        acct("3210", "WHT Payable - Vendors", AccountType::Liability, Some("3200"), false),
        acct("3300", "Payroll Liabilities", AccountType::Liability, None, true),
        acct("3310", "PAYE Payable", AccountType::Liability, Some("3300"), false),
        acct("3320", "NSSF Payable", AccountType::Liability, Some("3300"), false),
        acct("3330", "SHA Payable (NHIF)", AccountType::Liability, Some("3300"), false),
        acct("3340", "HELB Payable", AccountType::Liability, Some("3300"), false),
        acct("3350", "Housing Levy Payable", AccountType::Liability, Some("3300"), false),
        acct("3400", "Accrued Expenses", AccountType::Liability, None, false),
        acct("3500", "Other Current Liabilities", AccountType::Liability, None, false),
        acct("3600", "Unapplied Vendor Credits", AccountType::Liability, None, false),
        // === Non-Current Liabilities (4000–4499) ===
        acct("4000", "Long-term Liabilities", AccountType::Liability, None, true),
        acct("4100", "Bank Loans", AccountType::Liability, Some("4000"), false),
        acct("4200", "Directors Loans", AccountType::Liability, Some("4000"), false),
        // === Equity (4500–4999) ===
        acct("4500", "Share Capital", AccountType::Equity, None, false),
        acct("4600", "Retained Earnings", AccountType::Equity, None, false),
        acct("4700", "Current Year Earnings", AccountType::Equity, None, false),
        acct("4800", "Dividends Declared", AccountType::Equity, None, false),
        // === Revenue (5000–5999) ===
        acct("5000", "Sales Revenue", AccountType::Revenue, None, false),
        acct("5100", "Service Revenue", AccountType::Revenue, None, false),
        acct("5200", "Other Income", AccountType::Revenue, None, false),
        acct("5300", "Discounts Allowed", AccountType::ContraRevenue, None, false),
        acct("5400", "Sales Returns", AccountType::ContraRevenue, None, false),
        // === Cost of Goods Sold (6000–6999) ===
        acct("6000", "Cost of Goods Sold", AccountType::Expense, None, false),
        acct("6100", "Direct Materials", AccountType::Expense, Some("6000"), false),
        acct("6200", "Direct Labour", AccountType::Expense, Some("6000"), false),
        acct("6300", "Manufacturing Overhead", AccountType::Expense, Some("6000"), false),
        // === Operating Expenses (7000–7999) ===
        acct("7000", "Operating Expenses", AccountType::Expense, None, true),
        acct("7010", "Salaries and Wages", AccountType::Expense, Some("7000"), false),
        acct("7020", "Employer NSSF Contribution", AccountType::Expense, Some("7000"), false),
        acct("7030", "Employer Housing Levy", AccountType::Expense, Some("7000"), false),
        acct("7100", "Rent Expense", AccountType::Expense, Some("7000"), false),
        acct("7200", "Utilities", AccountType::Expense, Some("7000"), false),
        acct("7300", "Office Supplies", AccountType::Expense, Some("7000"), false),
        acct("7400", "Insurance", AccountType::Expense, Some("7000"), false),
        acct("7500", "Professional Fees", AccountType::Expense, Some("7000"), false),
        acct("7600", "Depreciation Expense", AccountType::Expense, Some("7000"), false),
        acct("7700", "Advertising & Marketing", AccountType::Expense, Some("7000"), false),
        acct("7800", "Travel & Transport", AccountType::Expense, Some("7000"), false),
        acct("7900", "Miscellaneous Expenses", AccountType::Expense, Some("7000"), false),
        // === Finance Income/Expense (8000–8499) ===
        acct("8000", "Finance Income", AccountType::Revenue, None, false),
        acct("8010", "Interest Income", AccountType::Revenue, Some("8000"), false),
        acct("8050", "Finance Expense", AccountType::Expense, None, false),
        acct("8060", "Interest Expense", AccountType::Expense, Some("8050"), false),
        acct("8070", "Bank Charges", AccountType::Expense, Some("8050"), false),
        acct("8100", "Unrealised FX Gain", AccountType::Revenue, None, false),
        acct("8110", "Unrealised FX Loss", AccountType::Expense, None, false),
        acct("8120", "Realised FX Gain", AccountType::Revenue, None, false),
        acct("8130", "Realised FX Loss", AccountType::Expense, None, false),
        // === Tax Expense (8500–8999) ===
        acct("8500", "Corporate Income Tax", AccountType::Expense, None, false),
        acct("8600", "Deferred Tax", AccountType::Expense, None, false),
        // === Control / Clearing / Suspense (9000–9999) ===
        acct("9000", "Suspense Account", AccountType::Asset, None, false),
        acct("9100", "Unapplied Customer Payments", AccountType::Liability, None, false),
        acct("9110", "Unapplied Vendor Credits", AccountType::Asset, None, false),
        acct("9200", "Inter-company Clearing", AccountType::Asset, None, false),
        acct("9300", "Opening Balance Equity", AccountType::Equity, None, false),
        acct("9900", "Rounding Differences", AccountType::Expense, None, false),
    ]
}

fn acct(
    code: &str,
    name: &str,
    account_type: AccountType,
    parent_code: Option<&str>,
    is_control: bool,
) -> CreateAccountRequest {
    CreateAccountRequest {
        code: code.to_string(),
        name: name.to_string(),
        account_type,
        parent_code: parent_code.map(|s| s.to_string()),
        currency: None,
        is_control,
        tags: vec![],
    }
}
