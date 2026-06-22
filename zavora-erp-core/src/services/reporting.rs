use chrono::{Datelike, NaiveDate, Utc};
use rust_decimal::Decimal;
use uuid::Uuid;

use crate::engine::ErpEngine;
use crate::error::ErpResult;
use crate::reporting::*;

/// As-at per-account movement (`account_code`, `total_debit`, `total_credit`)
/// with `$1 = entity_id`, `$2 = as_at`. Computed from period snapshots for every
/// period that ended on/before the date, plus the raw lines in the still-open
/// tail — so only one period's lines are ever scanned, not the whole ledger.
const ASAT_MOVEMENTS: &str = r#"
    SELECT account_code, SUM(d) AS total_debit, SUM(c) AS total_credit FROM (
        SELECT account_code, debit_total AS d, credit_total AS c
        FROM account_period_balances
        WHERE entity_id = $1 AND period_end <= $2
        UNION ALL
        SELECT account_code, COALESCE(functional_debit, 0) AS d, COALESCE(functional_credit, 0) AS c
        FROM journal_lines
        WHERE entity_id = $1 AND entry_date <= $2
          AND entry_date > COALESCE(
              (SELECT MAX(period_end) FROM account_period_balances WHERE entity_id = $1 AND period_end <= $2),
              DATE '0001-01-01')
    ) tail
    GROUP BY account_code
"#;

/// Generate a report based on the request type.
pub async fn generate_report(engine: &ErpEngine, req: ReportRequest) -> ErpResult<ReportData> {
    let now = Utc::now();
    let entity_id = req.entity_id;

    let content = match req.report_type {
        ReportType::TrialBalance => {
            let report = trial_balance(engine, entity_id, req.parameters).await?;
            ReportContent::TrialBalance(report)
        }
        ReportType::BalanceSheet => {
            let report = balance_sheet(engine, entity_id, req.parameters).await?;
            ReportContent::BalanceSheet(report)
        }
        ReportType::ProfitAndLoss => {
            let report = profit_and_loss(engine, entity_id, req.parameters).await?;
            ReportContent::ProfitAndLoss(report)
        }
        ReportType::CashFlow => {
            let report = cash_flow(engine, entity_id, req.parameters).await?;
            ReportContent::CashFlow(report)
        }
        ReportType::ArAgeing => {
            let report = ar_ageing(engine, entity_id, req.parameters).await?;
            ReportContent::ArAgeing(report)
        }
        ReportType::ApAgeing => {
            let report = ap_ageing(engine, entity_id, req.parameters).await?;
            ReportContent::ApAgeing(report)
        }
        ReportType::GlDetail => {
            let report = gl_detail(engine, entity_id, req.parameters).await?;
            ReportContent::GlDetail(report)
        }
        ReportType::VatReturn => {
            let report = vat_return(engine, entity_id, req.parameters).await?;
            ReportContent::VatReturn(report)
        }
        ReportType::CustomerStatement => {
            let report = party_statement(engine, entity_id, req.parameters, PartyKind::Customer).await?;
            ReportContent::PartyStatement(report)
        }
        ReportType::VendorStatement => {
            let report = party_statement(engine, entity_id, req.parameters, PartyKind::Vendor).await?;
            ReportContent::PartyStatement(report)
        }
        ReportType::PayrollSummary => {
            let report = payroll_summary(engine, entity_id, req.parameters).await?;
            ReportContent::PayrollSummary(report)
        }
        ReportType::PayeP10 => {
            let report = paye_p10(engine, entity_id, req.parameters).await?;
            ReportContent::PayeP10(report)
        }
        ReportType::WhtCertificate => {
            let report = wht_report(engine, entity_id, req.parameters).await?;
            ReportContent::WhtReport(report)
        }
        ReportType::SalesTaxSummary => {
            let report = vat_detail(engine, entity_id, req.parameters).await?;
            ReportContent::VatDetail(report)
        }
        ReportType::IncomeByCustomer => {
            let report = party_ranking(engine, entity_id, req.parameters, PartyKind::Customer).await?;
            ReportContent::PartyRanking(report)
        }
        ReportType::ExpenseByVendor => {
            let report = party_ranking(engine, entity_id, req.parameters, PartyKind::Vendor).await?;
            ReportContent::PartyRanking(report)
        }
        ReportType::InventoryValuation => {
            let report = inventory_valuation(engine, entity_id, req.parameters).await?;
            ReportContent::InventoryValuation(report)
        }
        ReportType::FixedAssetRegister => {
            let report = fixed_asset_register(engine, entity_id, req.parameters).await?;
            ReportContent::FixedAssetRegister(report)
        }
        ReportType::BankReconSummary => {
            let report = bank_recon_summary(engine, entity_id, req.parameters).await?;
            ReportContent::BankReconSummary(report)
        }
        ReportType::BudgetVsActual => {
            let report = budget_vs_actual(engine, entity_id, req.parameters).await?;
            ReportContent::BudgetVsActual(report)
        }
        ReportType::DimensionalAnalysis => {
            let report = dimensional_analysis(engine, entity_id, req.parameters).await?;
            ReportContent::DimensionalAnalysis(report)
        }
        ReportType::EquityChanges => {
            let report = equity_changes(engine, entity_id, req.parameters).await?;
            ReportContent::EquityChanges(report)
        }
        ReportType::CashFlowDirect => {
            let report = cash_flow_direct(engine, entity_id, req.parameters).await?;
            ReportContent::CashFlowDirect(report)
        }
        _ => {
            ReportContent::Generic(serde_json::json!({"message": "Report type not yet implemented"}))
        }
    };

    Ok(ReportData {
        report_type: req.report_type.clone(),
        generated_at: now,
        entity_id: req.entity_id,
        title: title_for(&req.report_type),
        subtitle: None,
        content,
    })
}

/// Human-readable title for a report type (shown on the statement letterhead).
fn title_for(report_type: &ReportType) -> String {
    match report_type {
        ReportType::TrialBalance => "Trial Balance",
        ReportType::BalanceSheet => "Balance Sheet",
        ReportType::ProfitAndLoss => "Profit & Loss Statement",
        ReportType::CashFlow => "Cash Flow Statement",
        ReportType::ArAgeing => "Accounts Receivable Ageing",
        ReportType::ApAgeing => "Accounts Payable Ageing",
        ReportType::VatReturn => "VAT Return",
        ReportType::GlDetail => "General Ledger",
        ReportType::CustomerStatement => "Customer Statement",
        ReportType::VendorStatement => "Vendor Statement",
        ReportType::IncomeByCustomer => "Income by Customer",
        ReportType::ExpenseByVendor => "Expense by Vendor",
        ReportType::InventoryValuation => "Inventory Valuation",
        ReportType::FixedAssetRegister => "Fixed-Asset Register",
        ReportType::BudgetVsActual => "Budget vs Actual",
        ReportType::DimensionalAnalysis => "Dimensional Analysis",
        ReportType::EquityChanges => "Statement of Changes in Equity",
        ReportType::CashFlowDirect => "Cash Flow Statement (Direct)",
        ReportType::CustomerPaymentHistory => "Customer Payment History",
        ReportType::BankReconSummary => "Bank Reconciliation Summary",
        ReportType::PayrollSummary => "Payroll Summary",
        ReportType::PayeP10 => "PAYE Return (P10)",
        ReportType::WhtCertificate => "Withholding Tax (WHT) Schedule",
        ReportType::SalesTaxSummary => "VAT Summary by Rate",
    }
    .to_string()
}

/// Generate dashboard summary.
pub async fn dashboard_summary(engine: &ErpEngine, entity_id: Uuid) -> ErpResult<DashboardSummary> {
    let now = Utc::now();
    let today = now.date_naive();

    // Total receivable
    let total_receivable = sqlx::query_scalar::<_, Decimal>(
        "SELECT COALESCE(SUM(balance_due), 0) FROM invoices WHERE entity_id = $1 AND status NOT IN ('paid', 'voided')",
    )
    .bind(entity_id)
    .fetch_one(engine.pool())
    .await
    .unwrap_or(Decimal::ZERO);

    // Overdue receivable
    let overdue_receivable = sqlx::query_scalar::<_, Decimal>(
        "SELECT COALESCE(SUM(balance_due), 0) FROM invoices WHERE entity_id = $1 AND status NOT IN ('paid', 'voided') AND due_date < $2",
    )
    .bind(entity_id)
    .bind(today)
    .fetch_one(engine.pool())
    .await
    .unwrap_or(Decimal::ZERO);

    let overdue_invoice_count = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM invoices WHERE entity_id = $1 AND status NOT IN ('paid', 'voided') AND due_date < $2",
    )
    .bind(entity_id)
    .bind(today)
    .fetch_one(engine.pool())
    .await
    .unwrap_or(0) as u32;

    // Total payable
    let total_payable = sqlx::query_scalar::<_, Decimal>(
        "SELECT COALESCE(SUM(balance_due), 0) FROM bills WHERE entity_id = $1 AND status NOT IN ('paid', 'cancelled')",
    )
    .bind(entity_id)
    .fetch_one(engine.pool())
    .await
    .unwrap_or(Decimal::ZERO);

    let overdue_payable = sqlx::query_scalar::<_, Decimal>(
        "SELECT COALESCE(SUM(balance_due), 0) FROM bills WHERE entity_id = $1 AND status NOT IN ('paid', 'cancelled') AND due_date < $2",
    )
    .bind(entity_id)
    .bind(today)
    .fetch_one(engine.pool())
    .await
    .unwrap_or(Decimal::ZERO);

    let overdue_bill_count = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM bills WHERE entity_id = $1 AND status NOT IN ('paid', 'cancelled') AND due_date < $2",
    )
    .bind(entity_id)
    .bind(today)
    .fetch_one(engine.pool())
    .await
    .unwrap_or(0) as u32;

    // Cash and bank — sum of all cash/bank account balances
    let cash_and_bank = sqlx::query_scalar::<_, Decimal>(
        r#"SELECT COALESCE(SUM(COALESCE(jl.functional_debit, 0) - COALESCE(jl.functional_credit, 0)), 0)
           FROM journal_lines jl
           JOIN journal_entries je ON jl.entry_id = je.id
           JOIN accounts a ON jl.account_code = a.code AND a.entity_id = je.entity_id
           WHERE je.entity_id = $1 AND je.status = 'posted'
           AND a.code LIKE '1%' AND a.account_type = 'asset' AND a.code < '1100'"#,
    )
    .bind(entity_id)
    .fetch_one(engine.pool())
    .await
    .unwrap_or(Decimal::ZERO);

    // Pending approvals
    let pending_approvals = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM bills WHERE entity_id = $1 AND status = 'pending_approval'",
    )
    .bind(entity_id)
    .fetch_one(engine.pool())
    .await
    .unwrap_or(0) as u32;

    // Uncategorised transactions
    let uncategorised_txns = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM imported_transactions WHERE entity_id = $1 AND category_status = 'uncategorised'",
    )
    .bind(entity_id)
    .fetch_one(engine.pool())
    .await
    .unwrap_or(0) as u32;

    Ok(DashboardSummary {
        as_at: now,
        total_receivable,
        overdue_receivable,
        overdue_invoice_count,
        total_payable,
        overdue_payable,
        overdue_bill_count,
        cash_and_bank,
        net_income_mtd: Decimal::ZERO, // TODO: compute from P&L
        net_income_prior: Decimal::ZERO,
        revenue_6m: Vec::new(),
        expenses_6m: Vec::new(),
        recent_transactions: Vec::new(),
        outstanding_invoices: Vec::new(),
        pending_approvals,
        uncategorised_txns,
    })
}

/// Trial balance report.
async fn trial_balance(engine: &ErpEngine, entity_id: Uuid, params: ReportParameters) -> ErpResult<TrialBalanceReport> {
    let as_at = params.as_at.unwrap_or_else(|| Utc::now().date_naive());

    // Pre-aggregate posted movements up to the as-at date in a subquery, then
    // join to accounts. Gating the line sums INSIDE the subquery (rather than a
    // LEFT JOIN on the entry) is essential: otherwise lines whose entry falls
    // outside the date window still leak into the totals.
    let lines = sqlx::query_as::<_, TrialBalanceQueryRow>(&format!(
        r#"SELECT a.code as account_code, a.name as account_name,
                  COALESCE(m.total_debit, 0)  as total_debit,
                  COALESCE(m.total_credit, 0) as total_credit
           FROM accounts a
           LEFT JOIN ({ASAT_MOVEMENTS}) m ON m.account_code = a.code
           WHERE a.entity_id = $1 AND a.is_active = true
             AND (COALESCE(m.total_debit, 0) <> 0 OR COALESCE(m.total_credit, 0) <> 0)
           ORDER BY a.code"#
    ))
    .bind(entity_id)
    .bind(as_at)
    .fetch_all(engine.pool())
    .await?;

    let report_lines: Vec<TrialBalanceLine> = lines
        .iter()
        .map(|r| {
            let net = r.total_debit - r.total_credit;
            TrialBalanceLine {
                account_code: r.account_code.clone(),
                account_name: r.account_name.clone(),
                opening_debit: Decimal::ZERO,
                opening_credit: Decimal::ZERO,
                movement_debit: r.total_debit,
                movement_credit: r.total_credit,
                closing_debit: if net > Decimal::ZERO { net } else { Decimal::ZERO },
                closing_credit: if net < Decimal::ZERO { -net } else { Decimal::ZERO },
            }
        })
        .collect();

    let total_debits: Decimal = report_lines.iter().map(|l| l.closing_debit).sum();
    let total_credits: Decimal = report_lines.iter().map(|l| l.closing_credit).sum();
    let difference = total_debits - total_credits;

    Ok(TrialBalanceReport {
        as_at,
        lines: report_lines,
        total_debits,
        total_credits,
        is_balanced: difference.abs() <= crate::money::ROUNDING_TOLERANCE,
        difference,
    })
}

#[derive(Debug, sqlx::FromRow)]
struct TrialBalanceQueryRow {
    account_code: String,
    account_name: String,
    total_debit: Decimal,
    total_credit: Decimal,
}

/// One year before `d` (default comparative reference for as-at reports).
fn prior_year(d: NaiveDate) -> NaiveDate {
    NaiveDate::from_ymd_opt(d.year() - 1, d.month(), d.day())
        .unwrap_or_else(|| NaiveDate::from_ymd_opt(d.year() - 1, d.month(), 28).unwrap())
}

/// Balance sheet, with an optional comparative period column.
async fn balance_sheet(engine: &ErpEngine, entity_id: Uuid, params: ReportParameters) -> ErpResult<BalanceSheetReport> {
    let as_at = params.as_at.unwrap_or_else(|| Utc::now().date_naive());
    let mut report = balance_sheet_at(engine, entity_id, as_at).await?;

    if params.comparative == Some(true) {
        let cmp_at = params.compare_to.unwrap_or_else(|| prior_year(as_at));
        let cmp = balance_sheet_at(engine, entity_id, cmp_at).await?;
        report.comparative_as_at = Some(cmp_at);
        report.total_assets_comparative = Some(cmp.total_assets);
        report.total_liabilities_comparative = Some(cmp.total_liabilities);
        report.total_equity_comparative = Some(cmp.total_equity);
        // Attach prior amounts to each line by account code.
        let prior: std::collections::HashMap<(usize, String), Decimal> = [&cmp.assets, &cmp.liabilities, &cmp.equity]
            .iter()
            .enumerate()
            .flat_map(|(i, secs)| secs.iter().flat_map(move |s| s.lines.iter().map(move |l| ((i, l.account_code.clone()), l.amount))))
            .collect();
        for (i, secs) in [&mut report.assets, &mut report.liabilities, &mut report.equity].into_iter().enumerate() {
            for s in secs.iter_mut() {
                for l in s.lines.iter_mut() {
                    l.comparative = prior.get(&(i, l.account_code.clone())).copied();
                }
            }
        }
    }
    Ok(report)
}

/// Balance sheet as at a single date (no comparative).
async fn balance_sheet_at(engine: &ErpEngine, entity_id: Uuid, as_at: NaiveDate) -> ErpResult<BalanceSheetReport> {
    // Posted balances as at the date, date-gated inside the subquery (see the
    // trial-balance note on why the gate must be on the line aggregation).
    let rows = sqlx::query_as::<_, BalanceSheetQueryRow>(&format!(
        r#"SELECT a.code, a.name, a.account_type, COALESCE(m.total_debit, 0) - COALESCE(m.total_credit, 0) as balance
           FROM accounts a
           JOIN ({ASAT_MOVEMENTS}) m ON m.account_code = a.code
           WHERE a.entity_id = $1 AND a.is_active = true
             AND a.account_type IN ('Asset', 'ContraAsset', 'Liability', 'ContraLiability', 'Equity')
             AND (COALESCE(m.total_debit, 0) - COALESCE(m.total_credit, 0)) <> 0
           ORDER BY a.code"#
    ))
    .bind(entity_id)
    .bind(as_at)
    .fetch_all(engine.pool())
    .await?;

    let mut assets = Vec::new();
    let mut liabilities = Vec::new();
    let mut equity = Vec::new();

    for row in &rows {
        // Present each line in its section's natural sign: assets carry their
        // debit balance directly (a contra-asset like accumulated depreciation
        // shows negative, reducing the section); liabilities and equity carry
        // the credit balance as a positive (= -(debit - credit)). Contras net
        // automatically — no blanket abs() that would wrongly add them.
        let (bucket, amount) = match row.account_type.as_str() {
            "Asset" | "ContraAsset" => (&mut assets, row.balance),
            "Liability" | "ContraLiability" => (&mut liabilities, -row.balance),
            "Equity" => (&mut equity, -row.balance),
            _ => continue,
        };
        bucket.push(BalanceSheetLine {
            account_code: row.code.clone(),
            account_name: row.name.clone(),
            amount,
            comparative: None,
        });
    }

    // Current-year (unclosed) earnings = net of all P&L-type movements up to the
    // as-at date, folded into equity so Assets = Liabilities + Equity holds even
    // before a year-end close has moved profit into retained earnings.
    let pnl_net: Decimal = sqlx::query_scalar::<_, Decimal>(&format!(
        r#"SELECT COALESCE(SUM(m.total_debit - m.total_credit), 0)
           FROM ({ASAT_MOVEMENTS}) m
           JOIN accounts a ON a.entity_id = $1 AND a.code = m.account_code
           WHERE a.account_type IN ('Revenue', 'ContraRevenue', 'Expense', 'ContraExpense')"#
    ))
    .bind(entity_id)
    .bind(as_at)
    .fetch_one(engine.pool())
    .await?;
    let current_year_earnings = -pnl_net; // credit-positive net income

    if current_year_earnings != Decimal::ZERO {
        equity.push(BalanceSheetLine {
            account_code: "—".to_string(),
            account_name: "Current Year Earnings".to_string(),
            amount: current_year_earnings,
            comparative: None,
        });
    }

    let total_assets: Decimal = assets.iter().map(|l| l.amount).sum();
    let total_liabilities: Decimal = liabilities.iter().map(|l| l.amount).sum();
    let total_equity: Decimal = equity.iter().map(|l| l.amount).sum();
    let difference = total_assets - (total_liabilities + total_equity);

    Ok(BalanceSheetReport {
        as_at,
        assets: vec![BalanceSheetSection { name: "Assets".to_string(), lines: assets, total: total_assets }],
        liabilities: vec![BalanceSheetSection { name: "Liabilities".to_string(), lines: liabilities, total: total_liabilities }],
        equity: vec![BalanceSheetSection { name: "Equity".to_string(), lines: equity, total: total_equity }],
        total_assets,
        total_liabilities,
        total_equity,
        current_year_earnings,
        comparative_as_at: None,
        total_assets_comparative: None,
        total_liabilities_comparative: None,
        total_equity_comparative: None,
        is_balanced: difference.abs() <= crate::money::ROUNDING_TOLERANCE,
        difference,
    })
}

#[derive(Debug, sqlx::FromRow)]
struct BalanceSheetQueryRow {
    code: String,
    name: String,
    account_type: String,
    balance: Decimal,
}

/// Profit & Loss report, with an optional comparative period column.
async fn profit_and_loss(engine: &ErpEngine, entity_id: Uuid, params: ReportParameters) -> ErpResult<ProfitAndLossReport> {
    let today = Utc::now().date_naive();
    let period_from = params.period_from.unwrap_or(NaiveDate::from_ymd_opt(today.year(), 1, 1).unwrap());
    let period_to = params.period_to.unwrap_or(today);

    let mut report = profit_and_loss_period(engine, entity_id, period_from, period_to).await?;
    if params.comparative == Some(true) {
        let cfrom = prior_year(period_from);
        let cto = prior_year(period_to);
        let cmp = profit_and_loss_period(engine, entity_id, cfrom, cto).await?;
        report.comparative_from = Some(cfrom);
        report.comparative_to = Some(cto);
        report.total_revenue_comparative = Some(cmp.total_revenue);
        report.gross_profit_comparative = Some(cmp.gross_profit);
        report.operating_profit_comparative = Some(cmp.operating_profit);
        report.net_profit_comparative = Some(cmp.net_profit);
        // Prior amounts per account code (an account belongs to exactly one section).
        let prior: std::collections::HashMap<String, Decimal> = [&cmp.revenue, &cmp.cost_of_sales, &cmp.operating_expenses, &cmp.other_income_expense]
            .iter()
            .flat_map(|secs| secs.iter().flat_map(|s| s.lines.iter().map(|l| (l.account_code.clone(), l.amount))))
            .collect();
        for secs in [&mut report.revenue, &mut report.cost_of_sales, &mut report.operating_expenses, &mut report.other_income_expense] {
            for s in secs.iter_mut() {
                for l in s.lines.iter_mut() {
                    l.comparative = prior.get(&l.account_code).copied();
                }
            }
        }
    }
    Ok(report)
}

/// Profit & Loss for a single period (no comparative).
async fn profit_and_loss_period(
    engine: &ErpEngine,
    entity_id: Uuid,
    period_from: NaiveDate,
    period_to: NaiveDate,
) -> ErpResult<ProfitAndLossReport> {

    let rows = sqlx::query_as::<_, PnlQueryRow>(
        r#"SELECT a.code, a.name, a.account_type, COALESCE(m.balance, 0) as balance
           FROM accounts a
           JOIN (
               SELECT jl.account_code,
                      SUM(COALESCE(jl.functional_debit, 0) - COALESCE(jl.functional_credit, 0)) as balance
               FROM journal_lines jl
               WHERE jl.entity_id = $1
                 AND jl.entry_date >= $2 AND jl.entry_date <= $3
               GROUP BY jl.account_code
           ) m ON m.account_code = a.code
           WHERE a.entity_id = $1
             AND a.account_type IN ('Revenue', 'ContraRevenue', 'Expense', 'ContraExpense')
             AND COALESCE(m.balance, 0) <> 0
           ORDER BY a.code"#,
    )
    .bind(entity_id)
    .bind(period_from)
    .bind(period_to)
    .fetch_all(engine.pool())
    .await?;

    let mut revenue_lines = Vec::new();
    let mut cogs_lines = Vec::new();
    let mut opex_lines = Vec::new();
    let mut other_lines = Vec::new();

    for row in &rows {
        let code_num: u32 = row.code.parse().unwrap_or(0);
        // Sign-correct, contra-aware presentation: revenue is credit-natured so
        // we negate (revenue positive; a contra-revenue debit balance becomes
        // negative and reduces the section). Expenses are debit-natured and kept
        // as-is (a contra-expense credit becomes negative, reducing the section).
        match row.account_type.as_str() {
            "Revenue" | "ContraRevenue" => revenue_lines.push(PnlLine {
                account_code: row.code.clone(),
                account_name: row.name.clone(),
                amount: -row.balance,
                comparative: None,
            }),
            "Expense" | "ContraExpense" => {
                let line = PnlLine {
                    account_code: row.code.clone(),
                    account_name: row.name.clone(),
                    amount: row.balance,
                    comparative: None,
                };
                if code_num >= 6000 && code_num < 7000 {
                    cogs_lines.push(line);
                } else if code_num >= 7000 && code_num < 8000 {
                    opex_lines.push(line);
                } else {
                    other_lines.push(line);
                }
            }
            _ => {}
        }
    }

    let total_revenue: Decimal = revenue_lines.iter().map(|l| l.amount).sum();
    let total_cogs: Decimal = cogs_lines.iter().map(|l| l.amount).sum();
    let gross_profit = total_revenue - total_cogs;
    let total_opex: Decimal = opex_lines.iter().map(|l| l.amount).sum();
    let operating_profit = gross_profit - total_opex;
    let total_other: Decimal = other_lines.iter().map(|l| l.amount).sum();
    let net_profit = operating_profit - total_other;

    Ok(ProfitAndLossReport {
        period_from,
        period_to,
        revenue: vec![PnlSection { name: "Revenue".to_string(), lines: revenue_lines, total: total_revenue }],
        cost_of_sales: vec![PnlSection { name: "Cost of Sales".to_string(), lines: cogs_lines, total: total_cogs }],
        operating_expenses: vec![PnlSection { name: "Operating Expenses".to_string(), lines: opex_lines, total: total_opex }],
        other_income_expense: vec![PnlSection { name: "Other".to_string(), lines: other_lines, total: total_other }],
        total_revenue,
        total_cost_of_sales: total_cogs,
        gross_profit,
        total_operating_expenses: total_opex,
        operating_profit,
        net_profit,
        comparative_from: None,
        comparative_to: None,
        total_revenue_comparative: None,
        gross_profit_comparative: None,
        operating_profit_comparative: None,
        net_profit_comparative: None,
    })
}

#[derive(Debug, sqlx::FromRow)]
struct PnlQueryRow {
    code: String,
    name: String,
    account_type: String,
    balance: Decimal,
}

/// Cash flow statement (indirect method).
///
/// Operating: Net income + depreciation + changes in working capital (AR, AP, inventory)
/// Investing: Asset purchases/disposals
/// Financing: Loan movements, equity changes
async fn cash_flow(engine: &ErpEngine, entity_id: Uuid, params: ReportParameters) -> ErpResult<CashFlowReport> {
    let today = Utc::now().date_naive();
    let period_from = params.period_from.unwrap_or(NaiveDate::from_ymd_opt(today.year(), 1, 1).unwrap());
    let period_to = params.period_to.unwrap_or(today);

    // --- Net Income from P&L ---
    let _net_income = sqlx::query_scalar::<_, Decimal>(
        r#"SELECT COALESCE(
               SUM(CASE WHEN a.account_type IN ('revenue', 'contra_revenue') 
                   THEN COALESCE(jl.functional_credit, 0) - COALESCE(jl.functional_debit, 0)
                   ELSE COALESCE(jl.functional_debit, 0) - COALESCE(jl.functional_credit, 0) END * -1
               ), 0)
           FROM journal_lines jl
           JOIN journal_entries je ON je.id = jl.entry_id
           JOIN accounts a ON a.code = jl.account_code AND a.entity_id = je.entity_id
           WHERE je.entity_id = $1 AND je.status = 'posted'
             AND je.date >= $2 AND je.date <= $3
             AND a.account_type IN ('revenue', 'contra_revenue', 'expense', 'contra_expense')"#,
    )
    .bind(entity_id)
    .bind(period_from)
    .bind(period_to)
    .fetch_one(engine.pool())
    .await
    .unwrap_or(Decimal::ZERO);

    // More reliable: revenue credits - expense debits
    let revenue = sqlx::query_scalar::<_, Decimal>(
        r#"SELECT COALESCE(SUM(COALESCE(jl.functional_credit, 0) - COALESCE(jl.functional_debit, 0)), 0)
           FROM journal_lines jl
           JOIN journal_entries je ON je.id = jl.entry_id
           JOIN accounts a ON a.code = jl.account_code AND a.entity_id = je.entity_id
           WHERE je.entity_id = $1 AND je.status = 'posted'
             AND je.date >= $2 AND je.date <= $3
             AND a.account_type IN ('revenue', 'contra_revenue')"#,
    )
    .bind(entity_id)
    .bind(period_from)
    .bind(period_to)
    .fetch_one(engine.pool())
    .await
    .unwrap_or(Decimal::ZERO);

    let expenses = sqlx::query_scalar::<_, Decimal>(
        r#"SELECT COALESCE(SUM(COALESCE(jl.functional_debit, 0) - COALESCE(jl.functional_credit, 0)), 0)
           FROM journal_lines jl
           JOIN journal_entries je ON je.id = jl.entry_id
           JOIN accounts a ON a.code = jl.account_code AND a.entity_id = je.entity_id
           WHERE je.entity_id = $1 AND je.status = 'posted'
             AND je.date >= $2 AND je.date <= $3
             AND a.account_type IN ('expense', 'contra_expense')"#,
    )
    .bind(entity_id)
    .bind(period_from)
    .bind(period_to)
    .fetch_one(engine.pool())
    .await
    .unwrap_or(Decimal::ZERO);

    let computed_net_income = revenue - expenses;

    // --- Add back: Depreciation expense (non-cash) ---
    let depreciation = sqlx::query_scalar::<_, Decimal>(
        r#"SELECT COALESCE(SUM(COALESCE(jl.functional_debit, 0)), 0)
           FROM journal_lines jl
           JOIN journal_entries je ON je.id = jl.entry_id
           WHERE je.entity_id = $1 AND je.status = 'posted'
             AND je.date >= $2 AND je.date <= $3
             AND je.source = '"Depreciation"'"#,
    )
    .bind(entity_id)
    .bind(period_from)
    .bind(period_to)
    .fetch_one(engine.pool())
    .await
    .unwrap_or(Decimal::ZERO);

    // --- Changes in working capital ---
    // Change in AR (increase = cash outflow, decrease = cash inflow)
    let ar_change = working_capital_change(engine, entity_id, "1200", "1299", period_from, period_to).await?;

    // Change in AP (increase = cash inflow, decrease = cash outflow)
    let ap_change = working_capital_change(engine, entity_id, "3000", "3099", period_from, period_to).await?;

    // Change in Inventory (increase = cash outflow)
    let inventory_change = working_capital_change(engine, entity_id, "1300", "1399", period_from, period_to).await?;

    let mut operating_lines = Vec::new();
    operating_lines.push(CashFlowLine { description: "Net income".to_string(), amount: computed_net_income });
    if depreciation != Decimal::ZERO {
        operating_lines.push(CashFlowLine { description: "Add back: Depreciation".to_string(), amount: depreciation });
    }
    if ar_change != Decimal::ZERO {
        // AR increase means less cash (negative), AR decrease means more cash (positive)
        operating_lines.push(CashFlowLine { description: "Change in accounts receivable".to_string(), amount: -ar_change });
    }
    if ap_change != Decimal::ZERO {
        // AP increase means more cash (positive), AP decrease means less cash (negative)
        operating_lines.push(CashFlowLine { description: "Change in accounts payable".to_string(), amount: ap_change });
    }
    if inventory_change != Decimal::ZERO {
        operating_lines.push(CashFlowLine { description: "Change in inventory".to_string(), amount: -inventory_change });
    }

    let operating_total: Decimal = operating_lines.iter().map(|l| l.amount).sum();

    // --- Investing activities: Fixed asset purchases ---
    let asset_purchases = sqlx::query_scalar::<_, Decimal>(
        r#"SELECT COALESCE(SUM(COALESCE(jl.functional_debit, 0) - COALESCE(jl.functional_credit, 0)), 0)
           FROM journal_lines jl
           JOIN journal_entries je ON je.id = jl.entry_id
           JOIN accounts a ON a.code = jl.account_code AND a.entity_id = je.entity_id
           WHERE je.entity_id = $1 AND je.status = 'posted'
             AND je.date >= $2 AND je.date <= $3
             AND a.code >= '2500' AND a.code < '2700'
             AND a.account_type = 'asset'"#,
    )
    .bind(entity_id)
    .bind(period_from)
    .bind(period_to)
    .fetch_one(engine.pool())
    .await
    .unwrap_or(Decimal::ZERO);

    let mut investing_lines = Vec::new();
    if asset_purchases != Decimal::ZERO {
        investing_lines.push(CashFlowLine { description: "Purchase of fixed assets".to_string(), amount: -asset_purchases });
    }
    let investing_total: Decimal = investing_lines.iter().map(|l| l.amount).sum();

    // --- Financing activities: Long-term liabilities and equity ---
    let loan_movements = working_capital_change(engine, entity_id, "3200", "3999", period_from, period_to).await?;
    let equity_movements = working_capital_change(engine, entity_id, "4000", "4999", period_from, period_to).await?;

    let mut financing_lines = Vec::new();
    if loan_movements != Decimal::ZERO {
        financing_lines.push(CashFlowLine { description: "Loan proceeds / (repayments)".to_string(), amount: loan_movements });
    }
    if equity_movements != Decimal::ZERO {
        financing_lines.push(CashFlowLine { description: "Equity movements".to_string(), amount: equity_movements });
    }
    let financing_total: Decimal = financing_lines.iter().map(|l| l.amount).sum();

    // --- Opening and closing cash ---
    let opening_cash = sqlx::query_scalar::<_, Decimal>(
        r#"SELECT COALESCE(SUM(COALESCE(jl.functional_debit, 0) - COALESCE(jl.functional_credit, 0)), 0)
           FROM journal_lines jl
           JOIN journal_entries je ON je.id = jl.entry_id
           JOIN accounts a ON a.code = jl.account_code AND a.entity_id = je.entity_id
           WHERE je.entity_id = $1 AND je.status = 'posted'
             AND je.date < $2
             AND a.code >= '1000' AND a.code < '1100'"#,
    )
    .bind(entity_id)
    .bind(period_from)
    .fetch_one(engine.pool())
    .await
    .unwrap_or(Decimal::ZERO);

    let net_change = operating_total + investing_total + financing_total;
    let closing_cash = opening_cash + net_change;

    Ok(CashFlowReport {
        period_from,
        period_to,
        operating_activities: CashFlowSection { lines: operating_lines, total: operating_total },
        investing_activities: CashFlowSection { lines: investing_lines, total: investing_total },
        financing_activities: CashFlowSection { lines: financing_lines, total: financing_total },
        net_change,
        opening_cash,
        closing_cash,
    })
}

/// Calculate the net movement in accounts within a code range during a period.
/// Returns positive for net debit increase, negative for net credit increase.
async fn working_capital_change(
    engine: &ErpEngine,
    entity_id: Uuid,
    code_from: &str,
    code_to: &str,
    period_from: NaiveDate,
    period_to: NaiveDate,
) -> ErpResult<Decimal> {
    let change = sqlx::query_scalar::<_, Decimal>(
        r#"SELECT COALESCE(SUM(COALESCE(jl.functional_debit, 0) - COALESCE(jl.functional_credit, 0)), 0)
           FROM journal_lines jl
           JOIN journal_entries je ON je.id = jl.entry_id
           WHERE je.entity_id = $1 AND je.status = 'posted'
             AND je.date >= $2 AND je.date <= $3
             AND jl.account_code >= $4 AND jl.account_code <= $5"#,
    )
    .bind(entity_id)
    .bind(period_from)
    .bind(period_to)
    .bind(code_from)
    .bind(code_to)
    .fetch_one(engine.pool())
    .await
    .unwrap_or(Decimal::ZERO);

    Ok(change)
}

/// Export report data to CSV format.
pub fn export_to_csv(report: &ReportData) -> ErpResult<Vec<u8>> {
    let mut output = Vec::new();

    match &report.content {
        ReportContent::TrialBalance(tb) => {
            output.extend_from_slice(b"Account Code,Account Name,Opening Debit,Opening Credit,Movement Debit,Movement Credit,Closing Debit,Closing Credit\n");
            for line in &tb.lines {
                let row = format!(
                    "{},{},{},{},{},{},{},{}\n",
                    csv_escape(&line.account_code),
                    csv_escape(&line.account_name),
                    line.opening_debit,
                    line.opening_credit,
                    line.movement_debit,
                    line.movement_credit,
                    line.closing_debit,
                    line.closing_credit,
                );
                output.extend_from_slice(row.as_bytes());
            }
            let totals = format!(",Totals,,,,{},{}\n", tb.total_debits, tb.total_credits);
            output.extend_from_slice(totals.as_bytes());
        }
        ReportContent::ProfitAndLoss(pnl) => {
            output.extend_from_slice(b"Section,Account Code,Account Name,Amount\n");
            for section in &pnl.revenue {
                for line in &section.lines {
                    let row = format!("Revenue,{},{},{}\n", csv_escape(&line.account_code), csv_escape(&line.account_name), line.amount);
                    output.extend_from_slice(row.as_bytes());
                }
            }
            output.extend_from_slice(format!(",,Total Revenue,{}\n", pnl.total_revenue).as_bytes());
            for section in &pnl.cost_of_sales {
                for line in &section.lines {
                    let row = format!("Cost of Sales,{},{},{}\n", csv_escape(&line.account_code), csv_escape(&line.account_name), line.amount);
                    output.extend_from_slice(row.as_bytes());
                }
            }
            output.extend_from_slice(format!(",,Gross Profit,{}\n", pnl.gross_profit).as_bytes());
            for section in &pnl.operating_expenses {
                for line in &section.lines {
                    let row = format!("Operating Expenses,{},{},{}\n", csv_escape(&line.account_code), csv_escape(&line.account_name), line.amount);
                    output.extend_from_slice(row.as_bytes());
                }
            }
            output.extend_from_slice(format!(",,Operating Profit,{}\n", pnl.operating_profit).as_bytes());
            output.extend_from_slice(format!(",,Net Profit,{}\n", pnl.net_profit).as_bytes());
        }
        ReportContent::BalanceSheet(bs) => {
            output.extend_from_slice(b"Section,Account Code,Account Name,Amount\n");
            for section in &bs.assets {
                for line in &section.lines {
                    let row = format!("Assets,{},{},{}\n", csv_escape(&line.account_code), csv_escape(&line.account_name), line.amount);
                    output.extend_from_slice(row.as_bytes());
                }
            }
            output.extend_from_slice(format!(",,Total Assets,{}\n", bs.total_assets).as_bytes());
            for section in &bs.liabilities {
                for line in &section.lines {
                    let row = format!("Liabilities,{},{},{}\n", csv_escape(&line.account_code), csv_escape(&line.account_name), line.amount);
                    output.extend_from_slice(row.as_bytes());
                }
            }
            output.extend_from_slice(format!(",,Total Liabilities,{}\n", bs.total_liabilities).as_bytes());
            for section in &bs.equity {
                for line in &section.lines {
                    let row = format!("Equity,{},{},{}\n", csv_escape(&line.account_code), csv_escape(&line.account_name), line.amount);
                    output.extend_from_slice(row.as_bytes());
                }
            }
            output.extend_from_slice(format!(",,Total Equity,{}\n", bs.total_equity).as_bytes());
        }
        ReportContent::CashFlow(cf) => {
            output.extend_from_slice(b"Section,Description,Amount\n");
            for line in &cf.operating_activities.lines {
                let row = format!("Operating,{},{}\n", csv_escape(&line.description), line.amount);
                output.extend_from_slice(row.as_bytes());
            }
            output.extend_from_slice(format!(",Total Operating,{}\n", cf.operating_activities.total).as_bytes());
            for line in &cf.investing_activities.lines {
                let row = format!("Investing,{},{}\n", csv_escape(&line.description), line.amount);
                output.extend_from_slice(row.as_bytes());
            }
            output.extend_from_slice(format!(",Total Investing,{}\n", cf.investing_activities.total).as_bytes());
            for line in &cf.financing_activities.lines {
                let row = format!("Financing,{},{}\n", csv_escape(&line.description), line.amount);
                output.extend_from_slice(row.as_bytes());
            }
            output.extend_from_slice(format!(",Total Financing,{}\n", cf.financing_activities.total).as_bytes());
            output.extend_from_slice(format!(",Net Change in Cash,{}\n", cf.net_change).as_bytes());
            output.extend_from_slice(format!(",Opening Cash,{}\n", cf.opening_cash).as_bytes());
            output.extend_from_slice(format!(",Closing Cash,{}\n", cf.closing_cash).as_bytes());
        }
        ReportContent::ArAgeing(ageing) | ReportContent::ApAgeing(ageing) => {
            output.extend_from_slice(b"Party Name,Current,1-30 Days,31-60 Days,61-90 Days,Over 90,Total\n");
            for line in &ageing.lines {
                let row = format!(
                    "{},{},{},{},{},{},{}\n",
                    csv_escape(&line.party_name),
                    line.current,
                    line.days_1_30,
                    line.days_31_60,
                    line.days_61_90,
                    line.over_90,
                    line.total,
                );
                output.extend_from_slice(row.as_bytes());
            }
            let totals = format!(
                "Totals,{},{},{},{},{},{}\n",
                ageing.totals.current,
                ageing.totals.days_1_30,
                ageing.totals.days_31_60,
                ageing.totals.days_61_90,
                ageing.totals.over_90,
                ageing.totals.total,
            );
            output.extend_from_slice(totals.as_bytes());
        }
        ReportContent::GlDetail(gl) => {
            output.extend_from_slice(format!("Account: {} - {}\n", gl.account_code, gl.account_name).as_bytes());
            output.extend_from_slice(b"Date,Journal Number,Description,Reference,Debit,Credit,Balance\n");
            for line in &gl.lines {
                let row = format!(
                    "{},{},{},{},{},{},{}\n",
                    line.date,
                    csv_escape(&line.journal_number),
                    csv_escape(&line.description),
                    csv_escape(&line.reference),
                    line.debit,
                    line.credit,
                    line.balance,
                );
                output.extend_from_slice(row.as_bytes());
            }
        }
        ReportContent::VatReturn(v) => {
            output.extend_from_slice(b"Line,Amount\n");
            output.extend_from_slice(format!("Output VAT (on sales),{}\n", v.output_vat).as_bytes());
            output.extend_from_slice(format!("Input VAT (on purchases),{}\n", v.input_vat).as_bytes());
            let label = if v.is_payable { "Net VAT payable to KRA" } else { "Net VAT credit carried forward" };
            output.extend_from_slice(format!("{},{}\n", label, v.net_vat.abs()).as_bytes());
        }
        ReportContent::PartyStatement(s) => {
            output.extend_from_slice(format!("Statement for {} ({})\n", csv_escape(&s.party_name), s.party_kind).as_bytes());
            output.extend_from_slice(b"Date,Type,Reference,Charge,Payment,Balance\n");
            output.extend_from_slice(format!(",,Opening Balance,,,{}\n", s.opening_balance).as_bytes());
            for line in &s.lines {
                let row = format!(
                    "{},{},{},{},{},{}\n",
                    line.date,
                    csv_escape(&line.doc_type),
                    csv_escape(&line.reference),
                    line.charge,
                    line.payment,
                    line.balance,
                );
                output.extend_from_slice(row.as_bytes());
            }
            output.extend_from_slice(format!(",,Closing Balance,{},{},{}\n", s.total_charges, s.total_payments, s.closing_balance).as_bytes());
        }
        ReportContent::PayrollSummary(p) => {
            output.extend_from_slice(b"Employee,Gross,PAYE,NSSF,SHA,Housing Levy,HELB,Net\n");
            for e in &p.employees {
                let row = format!(
                    "{},{},{},{},{},{},{},{}\n",
                    csv_escape(&e.employee_name),
                    e.gross, e.paye, e.nssf, e.sha, e.housing_levy, e.helb, e.net,
                );
                output.extend_from_slice(row.as_bytes());
            }
            let t = &p.totals;
            output.extend_from_slice(
                format!(
                    "Total,{},{},{},{},{},{},{}\n",
                    t.gross, t.paye, t.nssf, t.sha, t.housing_levy, t.helb, t.net
                )
                .as_bytes(),
            );
        }
        ReportContent::PayeP10(p) => {
            output.extend_from_slice(b"Staff No,Employee,KRA PIN,Gross Pay,Taxable Pay,Tax,Personal Relief,Insurance Relief,PAYE Payable\n");
            for l in &p.lines {
                let row = format!(
                    "{},{},{},{},{},{},{},{},{}\n",
                    csv_escape(&l.staff_number),
                    csv_escape(&l.employee_name),
                    csv_escape(&l.kra_pin),
                    l.gross_pay, l.taxable_pay, l.tax, l.personal_relief, l.insurance_relief, l.paye_payable,
                );
                output.extend_from_slice(row.as_bytes());
            }
            output.extend_from_slice(
                format!(",Total,,{},{},{},{},,{}\n", p.total_gross, p.total_taxable, p.total_paye, p.total_relief, p.total_payable).as_bytes(),
            );
        }
        ReportContent::WhtReport(w) => {
            output.extend_from_slice(b"Date,Bill,Vendor,KRA PIN,Category,Base Amount,WHT Amount\n");
            for l in &w.lines {
                let row = format!(
                    "{},{},{},{},{},{},{}\n",
                    l.date,
                    csv_escape(&l.document_number),
                    csv_escape(&l.vendor_name),
                    csv_escape(l.kra_pin.as_deref().unwrap_or("")),
                    csv_escape(l.wht_category.as_deref().unwrap_or("")),
                    l.base_amount, l.wht_amount,
                );
                output.extend_from_slice(row.as_bytes());
            }
            output.extend_from_slice(format!(",,,,Total,{},{}\n", w.total_base, w.total_wht).as_bytes());
        }
        ReportContent::VatDetail(v) => {
            output.extend_from_slice(b"Section,Treatment,Documents,Taxable Amount,VAT Amount\n");
            for b in &v.output {
                output.extend_from_slice(format!("Output,{},{},{},{}\n", csv_escape(&b.treatment), b.document_count, b.taxable_amount, b.vat_amount).as_bytes());
            }
            output.extend_from_slice(format!(",Total Output,,{},{}\n", v.total_output_taxable, v.total_output_vat).as_bytes());
            for b in &v.input {
                output.extend_from_slice(format!("Input,{},{},{},{}\n", csv_escape(&b.treatment), b.document_count, b.taxable_amount, b.vat_amount).as_bytes());
            }
            output.extend_from_slice(format!(",Total Input,,{},{}\n", v.total_input_taxable, v.total_input_vat).as_bytes());
            let label = if v.is_payable { "Net VAT payable to KRA" } else { "Net VAT credit carried forward" };
            output.extend_from_slice(format!(",{},,,{}\n", label, v.net_vat.abs()).as_bytes());
        }
        ReportContent::PartyRanking(p) => {
            let party = if p.party_kind == "vendor" { "Vendor" } else { "Customer" };
            output.extend_from_slice(format!("{},Documents,Amount,% of Total\n", party).as_bytes());
            for l in &p.lines {
                let row = format!(
                    "{},{},{},{}\n",
                    csv_escape(&l.party_name),
                    l.document_count,
                    l.amount,
                    l.percent.round_dp(1),
                );
                output.extend_from_slice(row.as_bytes());
            }
            output.extend_from_slice(format!("Total,,{},100.0\n", p.total).as_bytes());
        }
        ReportContent::InventoryValuation(inv) => {
            output.extend_from_slice(b"SKU,Description,UoM,On Hand,Unit Cost,Total Value,Costing Method\n");
            for l in &inv.lines {
                let row = format!(
                    "{},{},{},{},{},{},{}\n",
                    csv_escape(&l.sku),
                    csv_escape(&l.description),
                    csv_escape(&l.uom),
                    l.on_hand, l.unit_cost, l.total_value,
                    csv_escape(&l.costing_method),
                );
                output.extend_from_slice(row.as_bytes());
            }
            output.extend_from_slice(format!(",,,,Total,{},\n", inv.total_value).as_bytes());
        }
        ReportContent::FixedAssetRegister(fa) => {
            output.extend_from_slice(b"Asset No,Description,Category,Acquired,Cost,Accum. Depreciation,Net Book Value,Status\n");
            for l in &fa.lines {
                let row = format!(
                    "{},{},{},{},{},{},{},{}\n",
                    csv_escape(&l.asset_number),
                    csv_escape(&l.description),
                    csv_escape(&l.category),
                    l.acquisition_date,
                    l.cost, l.accumulated_depreciation, l.net_book_value,
                    csv_escape(&l.status),
                );
                output.extend_from_slice(row.as_bytes());
            }
            output.extend_from_slice(
                format!(",,,Total,{},{},{},\n", fa.total_cost, fa.total_accumulated_depreciation, fa.total_net_book_value).as_bytes(),
            );
        }
        ReportContent::BankReconSummary(br) => {
            output.extend_from_slice(b"Account,Bank,GL Account,Statement Balance,GL Balance,Difference,Unmatched Items,Unreconciled Amount,Reconciled\n");
            for l in &br.accounts {
                let row = format!(
                    "{},{},{},{},{},{},{},{},{}\n",
                    csv_escape(&l.account_name),
                    csv_escape(&l.bank_name),
                    csv_escape(&l.gl_account),
                    l.statement_balance, l.gl_balance, l.difference,
                    l.unmatched_count, l.unreconciled_amount,
                    if l.is_reconciled { "Yes" } else { "No" },
                );
                output.extend_from_slice(row.as_bytes());
            }
        }
        ReportContent::BudgetVsActual(r) => {
            output.extend_from_slice(b"Account Code,Account,Actual,Budget,Variance,Variance %\n");
            for l in &r.lines {
                let row = format!(
                    "{},{},{},{},{},{}\n",
                    csv_escape(&l.account_code),
                    csv_escape(&l.account_name),
                    l.actual, l.budget, l.variance,
                    l.variance_pct.map(|p| p.round_dp(1).to_string()).unwrap_or_default(),
                );
                output.extend_from_slice(row.as_bytes());
            }
            output.extend_from_slice(format!(",Total,{},{},{},\n", r.total_actual, r.total_budget, r.total_variance).as_bytes());
        }
        ReportContent::DimensionalAnalysis(r) => {
            output.extend_from_slice(format!("Dimension: {}\n", csv_escape(&r.dimension_type)).as_bytes());
            output.extend_from_slice(b"Value,Name,Debit,Credit,Net\n");
            for l in &r.lines {
                let row = format!(
                    "{},{},{},{},{}\n",
                    csv_escape(&l.value_code), csv_escape(&l.value_name), l.debit, l.credit, l.net,
                );
                output.extend_from_slice(row.as_bytes());
            }
            output.extend_from_slice(format!("Total,,{},{},{}\n", r.total_debit, r.total_credit, r.total_net).as_bytes());
        }
        ReportContent::EquityChanges(r) => {
            output.extend_from_slice(b"Account,Opening,Movement,Closing\n");
            for l in &r.lines {
                output.extend_from_slice(format!("{},{},{},{}\n", csv_escape(&l.account_name), l.opening, l.movement, l.closing).as_bytes());
            }
            output.extend_from_slice(format!("Profit for the period,,{},\n", r.profit_for_period).as_bytes());
            output.extend_from_slice(format!("Closing equity,{},,{}\n", r.opening_total, r.closing_total).as_bytes());
        }
        ReportContent::CashFlowDirect(r) => {
            output.extend_from_slice(b"Section,Account,Amount\n");
            for l in &r.receipts {
                output.extend_from_slice(format!("Receipt,{},{}\n", csv_escape(&l.account_name), l.amount).as_bytes());
            }
            output.extend_from_slice(format!(",Total receipts,{}\n", r.total_receipts).as_bytes());
            for l in &r.payments {
                output.extend_from_slice(format!("Payment,{},{}\n", csv_escape(&l.account_name), l.amount).as_bytes());
            }
            output.extend_from_slice(format!(",Total payments,{}\n", r.total_payments).as_bytes());
            output.extend_from_slice(format!(",Net change,{}\n", r.net_change).as_bytes());
            output.extend_from_slice(format!(",Opening cash,{}\n", r.opening_cash).as_bytes());
            output.extend_from_slice(format!(",Closing cash,{}\n", r.closing_cash).as_bytes());
        }
        ReportContent::Generic(_) => {
            output.extend_from_slice(b"Report type does not support CSV export\n");
        }
    }

    Ok(output)
}

/// Escape a string for CSV output (wrap in quotes if it contains commas or quotes).
fn csv_escape(s: &str) -> String {
    if s.contains(',') || s.contains('"') || s.contains('\n') {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s.to_string()
    }
}

/// AR ageing report.
async fn ar_ageing(engine: &ErpEngine, entity_id: Uuid, params: ReportParameters) -> ErpResult<AgeingReport> {
    let as_at = params.as_at.unwrap_or_else(|| Utc::now().date_naive());

    let rows = sqlx::query_as::<_, AgeingQueryRow>(
        r#"SELECT 
               i.customer_id as party_id,
               c.name as party_name,
               i.balance_due,
               i.due_date
           FROM invoices i
           JOIN customers c ON c.id = i.customer_id
           WHERE i.entity_id = $1 AND i.status NOT IN ('paid', 'voided') AND i.balance_due > 0"#,
    )
    .bind(entity_id)
    .fetch_all(engine.pool())
    .await?;

    let mut line_map: std::collections::HashMap<Uuid, AgeingLine> = std::collections::HashMap::new();

    for row in rows {
        let days_overdue = (as_at - row.due_date).num_days();
        let entry = line_map.entry(row.party_id).or_insert_with(|| AgeingLine {
            party_id: row.party_id,
            party_name: row.party_name.clone(),
            current: Decimal::ZERO,
            days_1_30: Decimal::ZERO,
            days_31_60: Decimal::ZERO,
            days_61_90: Decimal::ZERO,
            over_90: Decimal::ZERO,
            total: Decimal::ZERO,
        });

        if days_overdue <= 0 {
            entry.current += row.balance_due;
        } else if days_overdue <= 30 {
            entry.days_1_30 += row.balance_due;
        } else if days_overdue <= 60 {
            entry.days_31_60 += row.balance_due;
        } else if days_overdue <= 90 {
            entry.days_61_90 += row.balance_due;
        } else {
            entry.over_90 += row.balance_due;
        }
        entry.total += row.balance_due;
    }

    let lines: Vec<AgeingLine> = line_map.into_values().collect();
    let totals = AgeingBuckets {
        current: lines.iter().map(|l| l.current).sum(),
        days_1_30: lines.iter().map(|l| l.days_1_30).sum(),
        days_31_60: lines.iter().map(|l| l.days_31_60).sum(),
        days_61_90: lines.iter().map(|l| l.days_61_90).sum(),
        over_90: lines.iter().map(|l| l.over_90).sum(),
        total: lines.iter().map(|l| l.total).sum(),
    };

    Ok(AgeingReport { as_at, lines, totals })
}

/// AP ageing report.
async fn ap_ageing(engine: &ErpEngine, entity_id: Uuid, params: ReportParameters) -> ErpResult<AgeingReport> {
    let as_at = params.as_at.unwrap_or_else(|| Utc::now().date_naive());

    let rows = sqlx::query_as::<_, AgeingQueryRow>(
        r#"SELECT 
               b.vendor_id as party_id,
               v.name as party_name,
               b.balance_due,
               b.due_date
           FROM bills b
           JOIN vendors v ON v.id = b.vendor_id
           WHERE b.entity_id = $1 AND b.status NOT IN ('paid', 'cancelled') AND b.balance_due > 0"#,
    )
    .bind(entity_id)
    .fetch_all(engine.pool())
    .await?;

    let mut line_map: std::collections::HashMap<Uuid, AgeingLine> = std::collections::HashMap::new();

    for row in rows {
        let days_overdue = (as_at - row.due_date).num_days();
        let entry = line_map.entry(row.party_id).or_insert_with(|| AgeingLine {
            party_id: row.party_id,
            party_name: row.party_name.clone(),
            current: Decimal::ZERO,
            days_1_30: Decimal::ZERO,
            days_31_60: Decimal::ZERO,
            days_61_90: Decimal::ZERO,
            over_90: Decimal::ZERO,
            total: Decimal::ZERO,
        });

        if days_overdue <= 0 {
            entry.current += row.balance_due;
        } else if days_overdue <= 30 {
            entry.days_1_30 += row.balance_due;
        } else if days_overdue <= 60 {
            entry.days_31_60 += row.balance_due;
        } else if days_overdue <= 90 {
            entry.days_61_90 += row.balance_due;
        } else {
            entry.over_90 += row.balance_due;
        }
        entry.total += row.balance_due;
    }

    let lines: Vec<AgeingLine> = line_map.into_values().collect();
    let totals = AgeingBuckets {
        current: lines.iter().map(|l| l.current).sum(),
        days_1_30: lines.iter().map(|l| l.days_1_30).sum(),
        days_31_60: lines.iter().map(|l| l.days_31_60).sum(),
        days_61_90: lines.iter().map(|l| l.days_61_90).sum(),
        over_90: lines.iter().map(|l| l.over_90).sum(),
        total: lines.iter().map(|l| l.total).sum(),
    };

    Ok(AgeingReport { as_at, lines, totals })
}

#[derive(Debug, sqlx::FromRow)]
struct AgeingQueryRow {
    party_id: Uuid,
    party_name: String,
    balance_due: Decimal,
    due_date: NaiveDate,
}

#[derive(Clone, Copy, PartialEq)]
enum PartyKind {
    Customer,
    Vendor,
}

#[derive(Debug, sqlx::FromRow)]
struct StatementRow {
    date: NaiveDate,
    doc_type: String,
    reference: String,
    charge: Decimal,
    payment: Decimal,
}

/// Customer or vendor statement: opening balance + dated charges/payments with a
/// running balance. A customer's charges are invoices and payments are receipts;
/// a vendor's charges are bills and payments are payments we made. Payments are
/// matched to the party by `party_id` (customer and vendor ids never collide).
async fn party_statement(
    engine: &ErpEngine,
    entity_id: Uuid,
    params: ReportParameters,
    kind: PartyKind,
) -> ErpResult<PartyStatementReport> {
    let today = Utc::now().date_naive();
    let period_to = params.period_to.unwrap_or(today);
    // Default to the start of period_to's year if no explicit start given.
    let period_from = params
        .period_from
        .unwrap_or_else(|| NaiveDate::from_ymd_opt(period_to.year(), 1, 1).unwrap());

    let (party_id, party_table, doc_table, fk_col, charge_label, party_kind) = match kind {
        PartyKind::Customer => (
            params.customer_id,
            "customers",
            "invoices",
            "customer_id",
            "Invoice",
            "customer",
        ),
        PartyKind::Vendor => (
            params.vendor_id,
            "vendors",
            "bills",
            "vendor_id",
            "Bill",
            "vendor",
        ),
    };

    let party_id = party_id.ok_or_else(|| crate::error::ErpError::ValidationFailed {
        message: format!("A {party_kind} must be selected for this statement"),
    })?;

    let party_name: String = sqlx::query_scalar(&format!(
        "SELECT name FROM {party_table} WHERE id = $1 AND entity_id = $2"
    ))
    .bind(party_id)
    .bind(entity_id)
    .fetch_optional(engine.pool())
    .await?
    .ok_or_else(|| crate::error::ErpError::NotFound {
        entity_type: party_kind.to_string(),
        id: party_id,
    })?;

    // Invoices/bills excluded while still a draft (not yet a real obligation).
    let charge_status_excl = "('draft', 'voided', 'cancelled')";
    let pay_label = match kind {
        PartyKind::Customer => "Receipt",
        PartyKind::Vendor => "Payment",
    };

    // Opening balance = charges - payments strictly before the period start.
    let opening_charges: Decimal = sqlx::query_scalar(&format!(
        "SELECT COALESCE(SUM(gross_total), 0) FROM {doc_table}
         WHERE entity_id = $1 AND {fk_col} = $2
           AND status NOT IN {charge_status_excl} AND issue_date < $3"
    ))
    .bind(entity_id)
    .bind(party_id)
    .bind(period_from)
    .fetch_one(engine.pool())
    .await?;

    let opening_payments: Decimal = sqlx::query_scalar(
        "SELECT COALESCE(SUM(amount), 0) FROM payments
         WHERE entity_id = $1 AND party_id = $2 AND status = 'completed' AND payment_date < $3",
    )
    .bind(entity_id)
    .bind(party_id)
    .bind(period_from)
    .fetch_one(engine.pool())
    .await?;

    let opening_balance = opening_charges - opening_payments;

    // Activity within the period, charges and payments interleaved by date.
    let rows = sqlx::query_as::<_, StatementRow>(&format!(
        "SELECT issue_date AS date, '{charge_label}' AS doc_type, number AS reference,
                gross_total AS charge, 0::numeric AS payment
         FROM {doc_table}
         WHERE entity_id = $1 AND {fk_col} = $2
           AND status NOT IN {charge_status_excl} AND issue_date BETWEEN $3 AND $4
         UNION ALL
         SELECT payment_date AS date, '{pay_label}' AS doc_type, number AS reference,
                0::numeric AS charge, amount AS payment
         FROM payments
         WHERE entity_id = $1 AND party_id = $2 AND status = 'completed'
           AND payment_date BETWEEN $3 AND $4
         ORDER BY date, doc_type"
    ))
    .bind(entity_id)
    .bind(party_id)
    .bind(period_from)
    .bind(period_to)
    .fetch_all(engine.pool())
    .await?;

    let mut balance = opening_balance;
    let mut total_charges = Decimal::ZERO;
    let mut total_payments = Decimal::ZERO;
    let mut lines = Vec::with_capacity(rows.len());
    for r in rows {
        balance += r.charge - r.payment;
        total_charges += r.charge;
        total_payments += r.payment;
        lines.push(StatementLine {
            date: r.date,
            doc_type: r.doc_type,
            reference: r.reference,
            charge: r.charge,
            payment: r.payment,
            balance,
        });
    }

    Ok(PartyStatementReport {
        party_id,
        party_name,
        party_kind: party_kind.to_string(),
        period_from,
        period_to,
        opening_balance,
        lines,
        total_charges,
        total_payments,
        closing_balance: balance,
    })
}

#[derive(Debug, sqlx::FromRow)]
struct PayRunSummaryRow {
    id: Uuid,
    pay_date: NaiveDate,
    status: String,
    total_gross: Decimal,
    total_paye: Decimal,
    total_nssf: Decimal,
    total_sha: Decimal,
    total_housing_levy: Decimal,
    total_helb: Decimal,
    total_net: Decimal,
}

#[derive(Debug, sqlx::FromRow)]
struct PayslipSummaryRow {
    pay_run_id: Uuid,
    employee_id: Uuid,
    employee_name: String,
    gross: Decimal,
    paye: Decimal,
    nssf: Decimal,
    sha: Decimal,
    housing_levy: Decimal,
    helb: Decimal,
    net: Decimal,
}

/// Payroll summary across the pay runs whose pay date falls in the period.
/// Run-level money comes from `pay_runs`; per-employee figures are read from
/// each payslip's deduction breakdown (JSONB) and aggregated. Draft runs are
/// excluded.
async fn payroll_summary(
    engine: &ErpEngine,
    entity_id: Uuid,
    params: ReportParameters,
) -> ErpResult<PayrollSummaryReport> {
    let today = Utc::now().date_naive();
    let period_to = params.period_to.unwrap_or(today);
    let period_from = params
        .period_from
        .unwrap_or_else(|| NaiveDate::from_ymd_opt(period_to.year(), 1, 1).unwrap());

    let runs = sqlx::query_as::<_, PayRunSummaryRow>(
        "SELECT id, pay_date, status, total_gross, total_paye, total_nssf, total_sha,
                total_housing_levy, total_helb, total_net
         FROM pay_runs
         WHERE entity_id = $1 AND status <> 'draft' AND pay_date BETWEEN $2 AND $3
         ORDER BY pay_date",
    )
    .bind(entity_id)
    .bind(period_from)
    .bind(period_to)
    .fetch_all(engine.pool())
    .await?;

    let payslips = sqlx::query_as::<_, PayslipSummaryRow>(
        "SELECT ps.pay_run_id, ps.employee_id, e.full_name AS employee_name,
                (ps.deductions->>'gross_salary')::numeric          AS gross,
                (ps.deductions->>'net_paye')::numeric              AS paye,
                (ps.deductions->>'nssf_employee')::numeric         AS nssf,
                (ps.deductions->>'sha')::numeric                   AS sha,
                (ps.deductions->>'housing_levy_employee')::numeric AS housing_levy,
                (ps.deductions->>'helb')::numeric                  AS helb,
                (ps.deductions->>'net_salary')::numeric            AS net
         FROM payslips ps
         JOIN pay_runs pr ON pr.id = ps.pay_run_id
         JOIN employees e ON e.id = ps.employee_id
         WHERE pr.entity_id = $1 AND pr.status <> 'draft' AND pr.pay_date BETWEEN $2 AND $3",
    )
    .bind(entity_id)
    .bind(period_from)
    .bind(period_to)
    .fetch_all(engine.pool())
    .await?;

    // Employees per run, and per-employee aggregation across runs.
    let mut count_by_run: std::collections::HashMap<Uuid, u32> = std::collections::HashMap::new();
    let mut emp_map: std::collections::HashMap<Uuid, PayrollEmployeeLine> = std::collections::HashMap::new();
    for p in &payslips {
        *count_by_run.entry(p.pay_run_id).or_insert(0) += 1;
        let e = emp_map.entry(p.employee_id).or_insert_with(|| PayrollEmployeeLine {
            employee_id: p.employee_id,
            employee_name: p.employee_name.clone(),
            gross: Decimal::ZERO,
            paye: Decimal::ZERO,
            nssf: Decimal::ZERO,
            sha: Decimal::ZERO,
            housing_levy: Decimal::ZERO,
            helb: Decimal::ZERO,
            net: Decimal::ZERO,
        });
        e.gross += p.gross;
        e.paye += p.paye;
        e.nssf += p.nssf;
        e.sha += p.sha;
        e.housing_levy += p.housing_levy;
        e.helb += p.helb;
        e.net += p.net;
    }

    let run_lines: Vec<PayrollRunLine> = runs
        .iter()
        .map(|r| PayrollRunLine {
            pay_run_id: r.id,
            pay_date: r.pay_date,
            status: r.status.clone(),
            employee_count: *count_by_run.get(&r.id).unwrap_or(&0),
            gross: r.total_gross,
            paye: r.total_paye,
            nssf: r.total_nssf,
            sha: r.total_sha,
            housing_levy: r.total_housing_levy,
            helb: r.total_helb,
            net: r.total_net,
        })
        .collect();

    let totals = PayrollTotals {
        gross: run_lines.iter().map(|r| r.gross).sum(),
        paye: run_lines.iter().map(|r| r.paye).sum(),
        nssf: run_lines.iter().map(|r| r.nssf).sum(),
        sha: run_lines.iter().map(|r| r.sha).sum(),
        housing_levy: run_lines.iter().map(|r| r.housing_levy).sum(),
        helb: run_lines.iter().map(|r| r.helb).sum(),
        net: run_lines.iter().map(|r| r.net).sum(),
    };

    let mut employees: Vec<PayrollEmployeeLine> = emp_map.into_values().collect();
    employees.sort_by(|a, b| a.employee_name.cmp(&b.employee_name));

    Ok(PayrollSummaryReport {
        period_from,
        period_to,
        run_count: run_lines.len() as u32,
        employee_count: employees.len() as u32,
        runs: run_lines,
        employees,
        totals,
    })
}

/// Resolve a period [from, to], defaulting to the current month if neither is set.
fn resolve_period(params: &ReportParameters) -> (NaiveDate, NaiveDate) {
    let today = Utc::now().date_naive();
    match (params.period_from, params.period_to) {
        (Some(f), Some(t)) => (f, t),
        (Some(f), None) => (f, today),
        (None, Some(t)) => (NaiveDate::from_ymd_opt(t.year(), t.month(), 1).unwrap(), t),
        (None, None) => (NaiveDate::from_ymd_opt(today.year(), today.month(), 1).unwrap(), today),
    }
}

#[derive(Debug, sqlx::FromRow)]
struct PayeP10Row {
    staff_number: String,
    employee_name: String,
    kra_pin: String,
    gross_pay: Decimal,
    taxable_pay: Decimal,
    tax: Decimal,
    personal_relief: Decimal,
    insurance_relief: Decimal,
    paye_payable: Decimal,
}

/// PAYE return (P10): per-employee PAYE for the period, aggregated across the
/// (non-draft) pay runs whose pay date falls in the period.
async fn paye_p10(
    engine: &ErpEngine,
    entity_id: Uuid,
    params: ReportParameters,
) -> ErpResult<PayeP10Report> {
    let (period_from, period_to) = resolve_period(&params);

    let rows = sqlx::query_as::<_, PayeP10Row>(
        "SELECT e.staff_number, e.full_name AS employee_name, e.kra_pin,
                SUM((ps.deductions->>'gross_salary')::numeric)    AS gross_pay,
                SUM((ps.deductions->>'taxable_income')::numeric)  AS taxable_pay,
                SUM((ps.deductions->>'paye')::numeric)            AS tax,
                SUM((ps.deductions->>'personal_relief')::numeric) AS personal_relief,
                SUM((ps.deductions->>'insurance_relief')::numeric) AS insurance_relief,
                SUM((ps.deductions->>'net_paye')::numeric)        AS paye_payable
         FROM payslips ps
         JOIN pay_runs pr ON pr.id = ps.pay_run_id
         JOIN employees e ON e.id = ps.employee_id
         WHERE pr.entity_id = $1 AND pr.status <> 'draft' AND pr.pay_date BETWEEN $2 AND $3
         GROUP BY e.staff_number, e.full_name, e.kra_pin
         ORDER BY e.full_name",
    )
    .bind(entity_id)
    .bind(period_from)
    .bind(period_to)
    .fetch_all(engine.pool())
    .await?;

    let lines: Vec<PayeP10Line> = rows
        .into_iter()
        .map(|r| PayeP10Line {
            staff_number: r.staff_number,
            employee_name: r.employee_name,
            kra_pin: r.kra_pin,
            gross_pay: r.gross_pay,
            taxable_pay: r.taxable_pay,
            tax: r.tax,
            personal_relief: r.personal_relief,
            insurance_relief: r.insurance_relief,
            paye_payable: r.paye_payable,
        })
        .collect();

    Ok(PayeP10Report {
        period_from,
        period_to,
        total_gross: lines.iter().map(|l| l.gross_pay).sum(),
        total_taxable: lines.iter().map(|l| l.taxable_pay).sum(),
        total_paye: lines.iter().map(|l| l.tax).sum(),
        total_relief: lines.iter().map(|l| l.personal_relief + l.insurance_relief).sum(),
        total_payable: lines.iter().map(|l| l.paye_payable).sum(),
        lines,
    })
}

#[derive(Debug, sqlx::FromRow)]
struct WhtRow {
    date: NaiveDate,
    document_number: String,
    vendor_name: String,
    kra_pin: Option<String>,
    wht_category: Option<String>,
    base_amount: Decimal,
    wht_amount: Decimal,
}

/// Withholding tax withheld from suppliers — one line per (non-draft) bill that
/// carried WHT in the period.
async fn wht_report(
    engine: &ErpEngine,
    entity_id: Uuid,
    params: ReportParameters,
) -> ErpResult<WhtReport> {
    let (period_from, period_to) = resolve_period(&params);

    let rows = sqlx::query_as::<_, WhtRow>(
        "SELECT b.issue_date AS date, b.number AS document_number, v.name AS vendor_name,
                v.kra_pin, v.wht_category, b.subtotal AS base_amount, b.wht_amount
         FROM bills b
         JOIN vendors v ON v.id = b.vendor_id
         WHERE b.entity_id = $1 AND b.status <> 'draft' AND b.wht_amount > 0
           AND b.issue_date BETWEEN $2 AND $3
         ORDER BY b.issue_date, b.number",
    )
    .bind(entity_id)
    .bind(period_from)
    .bind(period_to)
    .fetch_all(engine.pool())
    .await?;

    let lines: Vec<WhtLine> = rows
        .into_iter()
        .map(|r| WhtLine {
            date: r.date,
            document_number: r.document_number,
            vendor_name: r.vendor_name,
            kra_pin: r.kra_pin,
            wht_category: r.wht_category,
            base_amount: r.base_amount,
            wht_amount: r.wht_amount,
        })
        .collect();

    Ok(WhtReport {
        period_from,
        period_to,
        total_base: lines.iter().map(|l| l.base_amount).sum(),
        total_wht: lines.iter().map(|l| l.wht_amount).sum(),
        lines,
    })
}

#[derive(Debug, sqlx::FromRow)]
struct VatBandRow {
    treatment: String,
    taxable_amount: Decimal,
    vat_amount: Decimal,
    document_count: i64,
}

/// VAT summary by rate band: sales (output) from invoice lines and purchases
/// (input) from bill lines, grouped by VAT treatment, with the net VAT position.
async fn vat_detail(
    engine: &ErpEngine,
    entity_id: Uuid,
    params: ReportParameters,
) -> ErpResult<VatDetailReport> {
    let (period_from, period_to) = resolve_period(&params);

    let output_rows = sqlx::query_as::<_, VatBandRow>(
        "SELECT il.vat_treatment AS treatment,
                COALESCE(SUM(il.line_total), 0) AS taxable_amount,
                COALESCE(SUM(il.vat_amount), 0) AS vat_amount,
                COUNT(DISTINCT i.id) AS document_count
         FROM invoice_lines il
         JOIN invoices i ON i.id = il.invoice_id
         WHERE i.entity_id = $1 AND i.status NOT IN ('draft', 'voided')
           AND i.issue_date BETWEEN $2 AND $3
         GROUP BY il.vat_treatment
         ORDER BY il.vat_treatment",
    )
    .bind(entity_id)
    .bind(period_from)
    .bind(period_to)
    .fetch_all(engine.pool())
    .await?;

    let input_rows = sqlx::query_as::<_, VatBandRow>(
        "SELECT bl.vat_treatment AS treatment,
                COALESCE(SUM(bl.line_total), 0) AS taxable_amount,
                COALESCE(SUM(bl.vat_amount), 0) AS vat_amount,
                COUNT(DISTINCT b.id) AS document_count
         FROM bill_lines bl
         JOIN bills b ON b.id = bl.bill_id
         WHERE b.entity_id = $1 AND b.status NOT IN ('draft', 'cancelled', 'voided')
           AND b.issue_date BETWEEN $2 AND $3
         GROUP BY bl.vat_treatment
         ORDER BY bl.vat_treatment",
    )
    .bind(entity_id)
    .bind(period_from)
    .bind(period_to)
    .fetch_all(engine.pool())
    .await?;

    let to_band = |r: VatBandRow| VatBand {
        treatment: r.treatment,
        taxable_amount: r.taxable_amount,
        vat_amount: r.vat_amount,
        document_count: r.document_count as u32,
    };
    let output: Vec<VatBand> = output_rows.into_iter().map(to_band).collect();
    let input: Vec<VatBand> = input_rows.into_iter().map(to_band).collect();

    let total_output_taxable = output.iter().map(|b| b.taxable_amount).sum();
    let total_output_vat: Decimal = output.iter().map(|b| b.vat_amount).sum();
    let total_input_taxable = input.iter().map(|b| b.taxable_amount).sum();
    let total_input_vat: Decimal = input.iter().map(|b| b.vat_amount).sum();
    let net_vat = total_output_vat - total_input_vat;

    Ok(VatDetailReport {
        period_from,
        period_to,
        output,
        input,
        total_output_taxable,
        total_output_vat,
        total_input_taxable,
        total_input_vat,
        net_vat,
        is_payable: net_vat > Decimal::ZERO,
    })
}

#[derive(Debug, sqlx::FromRow)]
struct PartyRankingRow {
    party_id: Uuid,
    party_name: String,
    document_count: i64,
    amount: Decimal,
}

/// Income by customer (invoices) or expense by vendor (bills): net, ex-VAT
/// amounts grouped per party for the period and ranked high to low. Drafts and
/// voided/cancelled documents are excluded.
async fn party_ranking(
    engine: &ErpEngine,
    entity_id: Uuid,
    params: ReportParameters,
    kind: PartyKind,
) -> ErpResult<PartyRankingReport> {
    let (period_from, period_to) = resolve_period(&params);

    // subtotal is the net (ex-VAT) value — the P&L-relevant figure.
    let sql = match kind {
        PartyKind::Customer => {
            "SELECT i.customer_id AS party_id, c.name AS party_name,
                    COUNT(*) AS document_count, COALESCE(SUM(i.subtotal), 0) AS amount
             FROM invoices i
             JOIN customers c ON c.id = i.customer_id
             WHERE i.entity_id = $1 AND i.status NOT IN ('draft', 'voided')
               AND i.issue_date BETWEEN $2 AND $3
             GROUP BY i.customer_id, c.name
             ORDER BY amount DESC"
        }
        PartyKind::Vendor => {
            "SELECT b.vendor_id AS party_id, v.name AS party_name,
                    COUNT(*) AS document_count, COALESCE(SUM(b.subtotal), 0) AS amount
             FROM bills b
             JOIN vendors v ON v.id = b.vendor_id
             WHERE b.entity_id = $1 AND b.status NOT IN ('draft', 'cancelled', 'voided')
               AND b.issue_date BETWEEN $2 AND $3
             GROUP BY b.vendor_id, v.name
             ORDER BY amount DESC"
        }
    };

    let rows = sqlx::query_as::<_, PartyRankingRow>(sql)
        .bind(entity_id)
        .bind(period_from)
        .bind(period_to)
        .fetch_all(engine.pool())
        .await?;

    let total: Decimal = rows.iter().map(|r| r.amount).sum();
    let hundred = Decimal::from(100);
    let lines: Vec<PartyRankingLine> = rows
        .into_iter()
        .map(|r| PartyRankingLine {
            percent: if total.is_zero() { Decimal::ZERO } else { (r.amount * hundred) / total },
            party_id: r.party_id,
            party_name: r.party_name,
            document_count: r.document_count as u32,
            amount: r.amount,
        })
        .collect();

    Ok(PartyRankingReport {
        party_kind: match kind {
            PartyKind::Customer => "customer".to_string(),
            PartyKind::Vendor => "vendor".to_string(),
        },
        period_from,
        period_to,
        lines,
        total,
    })
}

#[derive(Debug, sqlx::FromRow)]
struct InventoryValuationLineRow {
    sku: String,
    description: String,
    uom: String,
    on_hand: Decimal,
    unit_cost: Decimal,
    total_value: Decimal,
    costing_method: String,
}

/// Current inventory valuation from the running stock figures.
async fn inventory_valuation(
    engine: &ErpEngine,
    entity_id: Uuid,
    params: ReportParameters,
) -> ErpResult<InventoryValuationReport> {
    let as_at = params.as_at.unwrap_or_else(|| Utc::now().date_naive());

    let rows = sqlx::query_as::<_, InventoryValuationLineRow>(
        "SELECT sku, description, uom, on_hand, unit_cost, total_value, costing_method
         FROM inventory_items
         WHERE entity_id = $1 AND is_active = true
         ORDER BY sku",
    )
    .bind(entity_id)
    .fetch_all(engine.pool())
    .await?;

    let lines: Vec<InventoryValuationLine> = rows
        .into_iter()
        .map(|r| InventoryValuationLine {
            sku: r.sku,
            description: r.description,
            uom: r.uom,
            on_hand: r.on_hand,
            unit_cost: r.unit_cost,
            total_value: r.total_value,
            costing_method: r.costing_method,
        })
        .collect();

    Ok(InventoryValuationReport {
        as_at,
        total_value: lines.iter().map(|l| l.total_value).sum(),
        item_count: lines.len() as u32,
        lines,
    })
}

#[derive(Debug, sqlx::FromRow)]
struct FixedAssetLineRow {
    asset_number: String,
    description: String,
    category: String,
    acquisition_date: NaiveDate,
    cost: Decimal,
    accumulated_depreciation: Decimal,
    net_book_value: Decimal,
    status: String,
}

/// Fixed-asset register: cost, accumulated depreciation and net book value of
/// every non-disposed asset.
async fn fixed_asset_register(
    engine: &ErpEngine,
    entity_id: Uuid,
    params: ReportParameters,
) -> ErpResult<FixedAssetRegisterReport> {
    let as_at = params.as_at.unwrap_or_else(|| Utc::now().date_naive());

    let rows = sqlx::query_as::<_, FixedAssetLineRow>(
        "SELECT asset_number, description, category, acquisition_date, cost,
                accumulated_depreciation, net_book_value, status
         FROM fixed_assets
         WHERE entity_id = $1 AND status <> 'disposed'
         ORDER BY category, asset_number",
    )
    .bind(entity_id)
    .fetch_all(engine.pool())
    .await?;

    let lines: Vec<FixedAssetLine> = rows
        .into_iter()
        .map(|r| FixedAssetLine {
            asset_number: r.asset_number,
            description: r.description,
            category: r.category,
            acquisition_date: r.acquisition_date,
            cost: r.cost,
            accumulated_depreciation: r.accumulated_depreciation,
            net_book_value: r.net_book_value,
            status: r.status,
        })
        .collect();

    Ok(FixedAssetRegisterReport {
        as_at,
        total_cost: lines.iter().map(|l| l.cost).sum(),
        total_accumulated_depreciation: lines.iter().map(|l| l.accumulated_depreciation).sum(),
        total_net_book_value: lines.iter().map(|l| l.net_book_value).sum(),
        lines,
    })
}

#[derive(Debug, sqlx::FromRow)]
struct BankAccountRow {
    id: Uuid,
    name: String,
    bank_name: String,
    gl_account: String,
}

#[derive(Debug, sqlx::FromRow)]
struct BankFeedStatsRow {
    matched_count: i64,
    unmatched_count: i64,
    unreconciled_amount: Decimal,
}

/// Bank reconciliation summary: statement balance vs GL balance per bank account
/// as at a date, with matched/unmatched feed lines explaining the difference.
async fn bank_recon_summary(
    engine: &ErpEngine,
    entity_id: Uuid,
    params: ReportParameters,
) -> ErpResult<BankReconSummaryReport> {
    let as_at = params.as_at.unwrap_or_else(|| Utc::now().date_naive());

    // One account, or all of them.
    let accounts = if let Some(bank_id) = params.bank_account_id {
        sqlx::query_as::<_, BankAccountRow>(
            "SELECT id, name, bank_name, gl_account FROM bank_accounts WHERE entity_id = $1 AND id = $2",
        )
        .bind(entity_id)
        .bind(bank_id)
        .fetch_all(engine.pool())
        .await?
    } else {
        sqlx::query_as::<_, BankAccountRow>(
            "SELECT id, name, bank_name, gl_account FROM bank_accounts WHERE entity_id = $1 ORDER BY name",
        )
        .bind(entity_id)
        .fetch_all(engine.pool())
        .await?
    };

    let mut lines = Vec::with_capacity(accounts.len());
    for a in accounts {
        // Bank's own running balance on the most recent feed line up to the date.
        let statement_balance: Decimal = sqlx::query_scalar(
            "SELECT running_bal FROM imported_transactions
             WHERE entity_id = $1 AND bank_account = $2 AND value_date <= $3
             ORDER BY value_date DESC, created_at DESC
             LIMIT 1",
        )
        .bind(entity_id)
        .bind(a.id)
        .bind(as_at)
        .fetch_optional(engine.pool())
        .await?
        .unwrap_or(Decimal::ZERO);

        // GL balance of the control account (debit positive) up to the date.
        let gl_balance: Decimal = sqlx::query_scalar(
            "SELECT COALESCE(SUM(COALESCE(functional_debit, 0) - COALESCE(functional_credit, 0)), 0)
             FROM journal_lines
             WHERE entity_id = $1 AND account_code = $2 AND entry_date <= $3",
        )
        .bind(entity_id)
        .bind(&a.gl_account)
        .bind(as_at)
        .fetch_one(engine.pool())
        .await?;

        // Matched/unmatched feed lines and the net of the unmatched ones.
        let stats = sqlx::query_as::<_, BankFeedStatsRow>(
            "SELECT
                 COUNT(*) FILTER (WHERE journal_entry_id IS NOT NULL) AS matched_count,
                 COUNT(*) FILTER (WHERE journal_entry_id IS NULL)     AS unmatched_count,
                 COALESCE(SUM(CASE WHEN journal_entry_id IS NULL
                                   THEN COALESCE(credit, 0) - COALESCE(debit, 0) ELSE 0 END), 0)
                     AS unreconciled_amount
             FROM imported_transactions
             WHERE entity_id = $1 AND bank_account = $2 AND value_date <= $3",
        )
        .bind(entity_id)
        .bind(a.id)
        .bind(as_at)
        .fetch_one(engine.pool())
        .await?;

        let difference = statement_balance - gl_balance;
        let is_reconciled = (difference - stats.unreconciled_amount).abs() < dec_cent();

        lines.push(BankReconLine {
            bank_account_id: a.id,
            account_name: a.name,
            bank_name: a.bank_name,
            gl_account: a.gl_account,
            statement_balance,
            gl_balance,
            matched_count: stats.matched_count as u32,
            unmatched_count: stats.unmatched_count as u32,
            unreconciled_amount: stats.unreconciled_amount,
            difference,
            is_reconciled,
        });
    }

    Ok(BankReconSummaryReport { as_at, accounts: lines })
}

/// 0.01 tolerance for balance comparisons.
fn dec_cent() -> Decimal {
    Decimal::new(1, 2)
}

#[derive(Debug, sqlx::FromRow)]
struct BudgetVsActualRow {
    account_code: String,
    account_name: String,
    account_type: String,
    actual: Decimal,
    budget: Decimal,
}

/// Budget vs Actual for P&L accounts over the period. Actual is the ledger
/// movement in the account's natural sign (revenue credit-positive, expense
/// debit-positive); budget is the sum of budget entries for fiscal periods that
/// fall fully within the range. Includes any account with a budget or activity.
async fn budget_vs_actual(
    engine: &ErpEngine,
    entity_id: Uuid,
    params: ReportParameters,
) -> ErpResult<BudgetVsActualReport> {
    let (period_from, period_to) = resolve_period(&params);

    let rows = sqlx::query_as::<_, BudgetVsActualRow>(
        r#"
        WITH actual AS (
            SELECT a.code AS account_code, a.name AS account_name, a.account_type,
                   CASE WHEN a.account_type IN ('Revenue', 'ContraRevenue')
                        THEN COALESCE(SUM(jl.functional_credit), 0) - COALESCE(SUM(jl.functional_debit), 0)
                        ELSE COALESCE(SUM(jl.functional_debit), 0) - COALESCE(SUM(jl.functional_credit), 0)
                   END AS actual
            FROM accounts a
            LEFT JOIN journal_lines jl
                   ON jl.account_code = a.code AND jl.entity_id = $1
                  AND jl.entry_date BETWEEN $2 AND $3
            WHERE a.entity_id = $1
              AND a.account_type IN ('Revenue', 'ContraRevenue', 'Expense', 'ContraExpense')
            GROUP BY a.code, a.name, a.account_type
        ),
        budget AS (
            SELECT be.account_code, COALESCE(SUM(be.amount), 0) AS budget
            FROM budget_entries be
            JOIN fiscal_periods fp ON fp.id = be.period_id
            WHERE be.entity_id = $1 AND fp.start_date >= $2 AND fp.end_date <= $3
            GROUP BY be.account_code
        )
        SELECT actual.account_code, actual.account_name, actual.account_type,
               actual.actual AS actual, COALESCE(budget.budget, 0) AS budget
        FROM actual
        LEFT JOIN budget ON budget.account_code = actual.account_code
        WHERE actual.actual <> 0 OR COALESCE(budget.budget, 0) <> 0
        ORDER BY actual.account_code
        "#,
    )
    .bind(entity_id)
    .bind(period_from)
    .bind(period_to)
    .fetch_all(engine.pool())
    .await?;

    let hundred = Decimal::from(100);
    let lines: Vec<BudgetVsActualLine> = rows
        .into_iter()
        .map(|r| {
            let variance = r.actual - r.budget;
            BudgetVsActualLine {
                variance_pct: if r.budget.is_zero() { None } else { Some((variance * hundred) / r.budget) },
                account_code: r.account_code,
                account_name: r.account_name,
                account_type: r.account_type,
                actual: r.actual,
                budget: r.budget,
                variance,
            }
        })
        .collect();

    Ok(BudgetVsActualReport {
        period_from,
        period_to,
        total_actual: lines.iter().map(|l| l.actual).sum(),
        total_budget: lines.iter().map(|l| l.budget).sum(),
        total_variance: lines.iter().map(|l| l.variance).sum(),
        lines,
    })
}

#[derive(Debug, sqlx::FromRow)]
struct DimensionalRow {
    value_code: String,
    debit: Decimal,
    credit: Decimal,
}

/// Dimensional analysis: ledger movement grouped by the values of one dimension
/// type over the period (Option A — scans the date-bounded lines and reads the
/// JSONB dimension key). Values are resolved to names from dimension_values.
async fn dimensional_analysis(
    engine: &ErpEngine,
    entity_id: Uuid,
    params: ReportParameters,
) -> ErpResult<DimensionalAnalysisReport> {
    let (period_from, period_to) = resolve_period(&params);
    let dimension_type = params.dimension_type.clone().ok_or_else(|| {
        crate::error::ErpError::ValidationFailed {
            message: "A dimension type must be selected for this report".to_string(),
        }
    })?;

    let rows = sqlx::query_as::<_, DimensionalRow>(
        "SELECT COALESCE(NULLIF(dimensions->>$2, ''), '(unassigned)') AS value_code,
                COALESCE(SUM(functional_debit), 0)  AS debit,
                COALESCE(SUM(functional_credit), 0) AS credit
         FROM journal_lines
         WHERE entity_id = $1 AND entry_date BETWEEN $3 AND $4
         GROUP BY value_code
         ORDER BY value_code",
    )
    .bind(entity_id)
    .bind(&dimension_type)
    .bind(period_from)
    .bind(period_to)
    .fetch_all(engine.pool())
    .await?;

    // Resolve value codes to names.
    let names: std::collections::HashMap<String, String> = sqlx::query_as::<_, (String, String)>(
        "SELECT code, name FROM dimension_values WHERE entity_id = $1 AND type_code = $2",
    )
    .bind(entity_id)
    .bind(&dimension_type)
    .fetch_all(engine.pool())
    .await
    .unwrap_or_default()
    .into_iter()
    .collect();

    let lines: Vec<DimensionalLine> = rows
        .into_iter()
        .map(|r| DimensionalLine {
            value_name: names.get(&r.value_code).cloned().unwrap_or_else(|| r.value_code.clone()),
            net: r.debit - r.credit,
            value_code: r.value_code,
            debit: r.debit,
            credit: r.credit,
        })
        .collect();

    Ok(DimensionalAnalysisReport {
        dimension_type,
        period_from,
        period_to,
        total_debit: lines.iter().map(|l| l.debit).sum(),
        total_credit: lines.iter().map(|l| l.credit).sum(),
        total_net: lines.iter().map(|l| l.net).sum(),
        lines,
    })
}

#[derive(Debug, sqlx::FromRow)]
struct EquityChangeRow {
    account_code: String,
    account_name: String,
    opening: Decimal,
    closing: Decimal,
}

/// Statement of Changes in Equity.
async fn equity_changes(
    engine: &ErpEngine,
    entity_id: Uuid,
    params: ReportParameters,
) -> ErpResult<EquityChangesReport> {
    let (period_from, period_to) = resolve_period(&params);
    let opening_cutoff = period_from.pred_opt().unwrap_or(period_from);

    // Equity accounts are credit-natured: balance = credit - debit.
    let rows = sqlx::query_as::<_, EquityChangeRow>(
        "SELECT a.code AS account_code, a.name AS account_name,
                COALESCE(SUM(CASE WHEN jl.entry_date <= $2 THEN COALESCE(jl.functional_credit,0) - COALESCE(jl.functional_debit,0) ELSE 0 END), 0) AS opening,
                COALESCE(SUM(CASE WHEN jl.entry_date <= $3 THEN COALESCE(jl.functional_credit,0) - COALESCE(jl.functional_debit,0) ELSE 0 END), 0) AS closing
         FROM accounts a
         LEFT JOIN journal_lines jl ON jl.account_code = a.code AND jl.entity_id = $1
         WHERE a.entity_id = $1 AND a.account_type IN ('Equity', 'ContraEquity')
         GROUP BY a.code, a.name
         ORDER BY a.code",
    )
    .bind(entity_id).bind(opening_cutoff).bind(period_to)
    .fetch_all(engine.pool()).await?;

    // Net profit for the period = credit - debit over all P&L accounts.
    let profit_for_period: Decimal = sqlx::query_scalar(
        "SELECT COALESCE(SUM(COALESCE(jl.functional_credit,0) - COALESCE(jl.functional_debit,0)), 0)
         FROM journal_lines jl JOIN accounts a ON a.code = jl.account_code AND a.entity_id = $1
         WHERE jl.entity_id = $1 AND jl.entry_date BETWEEN $2 AND $3
           AND a.account_type IN ('Revenue','ContraRevenue','Expense','ContraExpense')",
    )
    .bind(entity_id).bind(period_from).bind(period_to)
    .fetch_one(engine.pool()).await.unwrap_or(Decimal::ZERO);

    let lines: Vec<EquityChangeLine> = rows.into_iter()
        .filter(|r| r.opening != Decimal::ZERO || r.closing != Decimal::ZERO)
        .map(|r| EquityChangeLine { account_code: r.account_code, account_name: r.account_name, opening: r.opening, movement: r.closing - r.opening, closing: r.closing })
        .collect();
    let opening_total: Decimal = lines.iter().map(|l| l.opening).sum();
    let booked_closing: Decimal = lines.iter().map(|l| l.closing).sum();

    Ok(EquityChangesReport {
        period_from,
        period_to,
        opening_total,
        profit_for_period,
        closing_total: booked_closing + profit_for_period,
        lines,
    })
}

#[derive(Debug, sqlx::FromRow)]
struct CashFlowContraRow {
    account_code: String,
    account_name: String,
    net: Decimal,
}

/// Direct-method cash flow: cash receipts/payments grouped by contra account.
/// Cash accounts are the GL accounts behind the entity's bank accounts.
async fn cash_flow_direct(
    engine: &ErpEngine,
    entity_id: Uuid,
    params: ReportParameters,
) -> ErpResult<CashFlowDirectReport> {
    let (period_from, period_to) = resolve_period(&params);
    let opening_cutoff = period_from.pred_opt().unwrap_or(period_from);

    let cash_accounts: Vec<String> = sqlx::query_scalar("SELECT gl_account FROM bank_accounts WHERE entity_id = $1")
        .bind(entity_id).fetch_all(engine.pool()).await.unwrap_or_default();

    if cash_accounts.is_empty() {
        return Ok(CashFlowDirectReport { period_from, period_to, receipts: vec![], payments: vec![], total_receipts: Decimal::ZERO, total_payments: Decimal::ZERO, net_change: Decimal::ZERO, opening_cash: Decimal::ZERO, closing_cash: Decimal::ZERO });
    }

    let cash_balance = |cutoff: NaiveDate| {
        let cash = cash_accounts.clone();
        async move {
            sqlx::query_scalar::<_, Decimal>(
                "SELECT COALESCE(SUM(COALESCE(functional_debit,0) - COALESCE(functional_credit,0)), 0)
                 FROM journal_lines WHERE entity_id = $1 AND account_code = ANY($2) AND entry_date <= $3",
            )
            .bind(entity_id).bind(&cash).bind(cutoff)
            .fetch_one(engine.pool()).await.unwrap_or(Decimal::ZERO)
        }
    };
    let opening_cash = cash_balance(opening_cutoff).await;
    let closing_cash = cash_balance(period_to).await;

    // Contra movements: non-cash lines in cash-touching entries within the period.
    let contra = sqlx::query_as::<_, CashFlowContraRow>(
        "SELECT a.code AS account_code, a.name AS account_name,
                COALESCE(SUM(COALESCE(jl.functional_credit,0) - COALESCE(jl.functional_debit,0)), 0) AS net
         FROM journal_lines jl JOIN accounts a ON a.code = jl.account_code AND a.entity_id = $1
         WHERE jl.entity_id = $1 AND jl.entry_date BETWEEN $2 AND $3
           AND jl.account_code <> ALL($4)
           AND jl.entry_id IN (SELECT entry_id FROM journal_lines WHERE entity_id = $1 AND account_code = ANY($4))
         GROUP BY a.code, a.name
         ORDER BY a.code",
    )
    .bind(entity_id).bind(period_from).bind(period_to).bind(&cash_accounts)
    .fetch_all(engine.pool()).await.unwrap_or_default();

    let mut receipts = Vec::new();
    let mut payments = Vec::new();
    for r in contra {
        if r.net > Decimal::ZERO {
            receipts.push(CashFlowDirectLine { account_code: r.account_code, account_name: r.account_name, amount: r.net });
        } else if r.net < Decimal::ZERO {
            payments.push(CashFlowDirectLine { account_code: r.account_code, account_name: r.account_name, amount: -r.net });
        }
    }
    let total_receipts: Decimal = receipts.iter().map(|l| l.amount).sum();
    let total_payments: Decimal = payments.iter().map(|l| l.amount).sum();

    Ok(CashFlowDirectReport {
        period_from,
        period_to,
        net_change: total_receipts - total_payments,
        opening_cash,
        closing_cash,
        total_receipts,
        total_payments,
        receipts,
        payments,
    })
}

/// GL detail report.
async fn gl_detail(engine: &ErpEngine, entity_id: Uuid, params: ReportParameters) -> ErpResult<GlDetailReport> {
    let account_code = params.account_code.unwrap_or_default();
    let today = Utc::now().date_naive();
    let period_from = params.period_from.unwrap_or(NaiveDate::from_ymd_opt(today.year(), 1, 1).unwrap());
    let period_to = params.period_to.unwrap_or(today);

    let account_name = sqlx::query_scalar::<_, String>(
        "SELECT name FROM accounts WHERE entity_id = $1 AND code = $2",
    )
    .bind(entity_id)
    .bind(&account_code)
    .fetch_optional(engine.pool())
    .await?
    .unwrap_or_else(|| account_code.clone());

    // Opening balance = all posted movement on this account strictly BEFORE the
    // period start. Without this the running balance (and closing balance) would
    // not tie back to the trial balance for any account with prior history.
    let opening_balance = sqlx::query_scalar::<_, Decimal>(
        r#"SELECT COALESCE(SUM(COALESCE(jl.functional_debit, 0) - COALESCE(jl.functional_credit, 0)), 0)
           FROM journal_lines jl
           WHERE jl.entity_id = $1
             AND jl.account_code = $2 AND jl.entry_date < $3"#,
    )
    .bind(entity_id)
    .bind(&account_code)
    .bind(period_from)
    .fetch_one(engine.pool())
    .await?;

    let rows = sqlx::query_as::<_, GlDetailQueryRow>(
        r#"SELECT
               je.id as entry_id, je.date, je.number as journal_number,
               je.description, je.reference, je.source, je.source_id,
               COALESCE(jl.functional_debit, 0) as debit,
               COALESCE(jl.functional_credit, 0) as credit
           FROM journal_lines jl
           JOIN journal_entries je ON je.id = jl.entry_id
           WHERE je.entity_id = $1 AND je.status = 'posted'
           AND jl.account_code = $2
           AND je.date >= $3 AND je.date <= $4
           ORDER BY je.date, je.number"#,
    )
    .bind(entity_id)
    .bind(&account_code)
    .bind(period_from)
    .bind(period_to)
    .fetch_all(engine.pool())
    .await?;

    let mut balance = opening_balance;
    let lines: Vec<GlDetailLine> = rows
        .iter()
        .map(|r| {
            balance += r.debit - r.credit;
            GlDetailLine {
                date: r.date,
                entry_id: r.entry_id,
                journal_number: r.journal_number.clone(),
                description: r.description.clone(),
                reference: r.reference.clone(),
                source: r.source.clone(),
                source_id: r.source_id,
                debit: r.debit,
                credit: r.credit,
                balance,
            }
        })
        .collect();

    Ok(GlDetailReport {
        account_code,
        account_name,
        period_from,
        period_to,
        opening_balance,
        lines,
        closing_balance: balance,
    })
}

#[derive(Debug, sqlx::FromRow)]
struct GlDetailQueryRow {
    entry_id: Uuid,
    date: NaiveDate,
    journal_number: String,
    description: String,
    reference: String,
    source: String,
    source_id: Option<Uuid>,
    debit: Decimal,
    credit: Decimal,
}

#[derive(Debug, sqlx::FromRow)]
struct AccountMovementRow {
    debit: Decimal,
    credit: Decimal,
}

/// Net posted movement (debit, credit) on one account over a period.
async fn account_movement(
    engine: &ErpEngine,
    entity_id: Uuid,
    account_code: &str,
    from: NaiveDate,
    to: NaiveDate,
) -> ErpResult<(Decimal, Decimal)> {
    let m = sqlx::query_as::<_, AccountMovementRow>(
        r#"SELECT COALESCE(SUM(jl.functional_debit), 0) as debit,
                  COALESCE(SUM(jl.functional_credit), 0) as credit
           FROM journal_lines jl
           WHERE jl.entity_id = $1
             AND jl.account_code = $2 AND jl.entry_date >= $3 AND jl.entry_date <= $4"#,
    )
    .bind(entity_id)
    .bind(account_code)
    .bind(from)
    .bind(to)
    .fetch_one(engine.pool())
    .await?;
    Ok((m.debit, m.credit))
}

/// VAT return (KRA) for a period: output VAT (net credit on the VAT-output
/// account) less input VAT (net debit on the VAT-input account), netting to the
/// amount payable to — or creditable from — KRA. The control accounts come from
/// the entity's posting setup, so the figures tie directly to the ledger.
async fn vat_return(engine: &ErpEngine, entity_id: Uuid, params: ReportParameters) -> ErpResult<VatReturnReport> {
    let today = Utc::now().date_naive();
    let period_from = params.period_from.unwrap_or(NaiveDate::from_ymd_opt(today.year(), 1, 1).unwrap());
    let period_to = params.period_to.unwrap_or(today);

    let posting = engine.posting_for(entity_id).await?;
    let vat_output_account = posting.vat_output.clone();
    let vat_input_account = posting.vat_input.clone();

    let (out_debit, out_credit) =
        account_movement(engine, entity_id, &vat_output_account, period_from, period_to).await?;
    let (in_debit, in_credit) =
        account_movement(engine, entity_id, &vat_input_account, period_from, period_to).await?;

    // Output VAT is credit-natured; input VAT is debit-natured. Net the contra
    // direction so refunds/adjustments are reflected correctly.
    let output_vat = out_credit - out_debit;
    let input_vat = in_debit - in_credit;
    let net_vat = output_vat - input_vat;

    Ok(VatReturnReport {
        period_from,
        period_to,
        output_vat,
        input_vat,
        net_vat,
        is_payable: net_vat > Decimal::ZERO,
        vat_output_account,
        vat_input_account,
    })
}

