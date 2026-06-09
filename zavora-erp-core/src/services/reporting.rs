use chrono::{Datelike, NaiveDate, Utc};
use rust_decimal::Decimal;
use uuid::Uuid;

use crate::engine::ErpEngine;
use crate::error::ErpResult;
use crate::reporting::*;

/// Generate a report based on the request type.
pub async fn generate_report(engine: &ErpEngine, req: ReportRequest) -> ErpResult<ReportData> {
    let now = Utc::now();

    let content = match req.report_type {
        ReportType::TrialBalance => {
            let report = trial_balance(engine, req.parameters).await?;
            ReportContent::TrialBalance(report)
        }
        ReportType::BalanceSheet => {
            let report = balance_sheet(engine, req.parameters).await?;
            ReportContent::BalanceSheet(report)
        }
        ReportType::ProfitAndLoss => {
            let report = profit_and_loss(engine, req.parameters).await?;
            ReportContent::ProfitAndLoss(report)
        }
        ReportType::CashFlow => {
            let report = cash_flow(engine, req.parameters).await?;
            ReportContent::CashFlow(report)
        }
        ReportType::ArAgeing => {
            let report = ar_ageing(engine, req.parameters).await?;
            ReportContent::ArAgeing(report)
        }
        ReportType::ApAgeing => {
            let report = ap_ageing(engine, req.parameters).await?;
            ReportContent::ApAgeing(report)
        }
        ReportType::GlDetail => {
            let report = gl_detail(engine, req.parameters).await?;
            ReportContent::GlDetail(report)
        }
        _ => {
            ReportContent::Generic(serde_json::json!({"message": "Report type not yet implemented"}))
        }
    };

    Ok(ReportData {
        report_type: req.report_type,
        generated_at: now,
        entity_id: req.entity_id,
        title: "Report".to_string(),
        subtitle: None,
        content,
    })
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
async fn trial_balance(engine: &ErpEngine, params: ReportParameters) -> ErpResult<TrialBalanceReport> {
    let as_at = params.as_at.unwrap_or_else(|| Utc::now().date_naive());

    let lines = sqlx::query_as::<_, TrialBalanceQueryRow>(
        r#"SELECT 
               a.code as account_code,
               a.name as account_name,
               COALESCE(SUM(CASE WHEN je.date <= $2 THEN jl.functional_debit ELSE 0 END), 0) as total_debit,
               COALESCE(SUM(CASE WHEN je.date <= $2 THEN jl.functional_credit ELSE 0 END), 0) as total_credit
           FROM accounts a
           LEFT JOIN journal_lines jl ON jl.account_code = a.code
           LEFT JOIN journal_entries je ON je.id = jl.entry_id AND je.entity_id = a.entity_id AND je.status = 'posted'
           WHERE a.entity_id = $1 AND a.is_active = true
           GROUP BY a.code, a.name
           HAVING COALESCE(SUM(jl.functional_debit), 0) != 0 OR COALESCE(SUM(jl.functional_credit), 0) != 0
           ORDER BY a.code"#,
    )
    .bind(engine.entity_id())
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

    let total_debits = report_lines.iter().map(|l| l.closing_debit).sum();
    let total_credits = report_lines.iter().map(|l| l.closing_credit).sum();

    Ok(TrialBalanceReport {
        as_at,
        lines: report_lines,
        total_debits,
        total_credits,
    })
}

#[derive(Debug, sqlx::FromRow)]
struct TrialBalanceQueryRow {
    account_code: String,
    account_name: String,
    total_debit: Decimal,
    total_credit: Decimal,
}

/// Balance sheet report.
async fn balance_sheet(engine: &ErpEngine, params: ReportParameters) -> ErpResult<BalanceSheetReport> {
    let as_at = params.as_at.unwrap_or_else(|| Utc::now().date_naive());

    // Query balances grouped by account type
    let rows = sqlx::query_as::<_, BalanceSheetQueryRow>(
        r#"SELECT 
               a.code, a.name, a.account_type,
               COALESCE(SUM(COALESCE(jl.functional_debit, 0) - COALESCE(jl.functional_credit, 0)), 0) as balance
           FROM accounts a
           LEFT JOIN journal_lines jl ON jl.account_code = a.code
           LEFT JOIN journal_entries je ON je.id = jl.entry_id AND je.entity_id = a.entity_id AND je.status = 'posted' AND je.date <= $2
           WHERE a.entity_id = $1 AND a.is_active = true
           AND a.account_type IN ('asset', 'contra_asset', 'liability', 'contra_liability', 'equity')
           GROUP BY a.code, a.name, a.account_type
           HAVING COALESCE(SUM(COALESCE(jl.functional_debit, 0) - COALESCE(jl.functional_credit, 0)), 0) != 0
           ORDER BY a.code"#,
    )
    .bind(engine.entity_id())
    .bind(as_at)
    .fetch_all(engine.pool())
    .await?;

    let mut assets = Vec::new();
    let mut liabilities = Vec::new();
    let mut equity = Vec::new();

    for row in &rows {
        let line = BalanceSheetLine {
            account_code: row.code.clone(),
            account_name: row.name.clone(),
            amount: row.balance,
            comparative: None,
        };
        match row.account_type.as_str() {
            "asset" | "contra_asset" => assets.push(line),
            "liability" | "contra_liability" => liabilities.push(line),
            "equity" => equity.push(line),
            _ => {}
        }
    }

    let total_assets: Decimal = assets.iter().map(|l| l.amount).sum();
    let total_liabilities: Decimal = liabilities.iter().map(|l| l.amount.abs()).sum();
    let total_equity: Decimal = equity.iter().map(|l| l.amount.abs()).sum();

    Ok(BalanceSheetReport {
        as_at,
        assets: vec![BalanceSheetSection { name: "Assets".to_string(), lines: assets, total: total_assets }],
        liabilities: vec![BalanceSheetSection { name: "Liabilities".to_string(), lines: liabilities, total: total_liabilities }],
        equity: vec![BalanceSheetSection { name: "Equity".to_string(), lines: equity, total: total_equity }],
        total_assets,
        total_liabilities,
        total_equity,
    })
}

#[derive(Debug, sqlx::FromRow)]
struct BalanceSheetQueryRow {
    code: String,
    name: String,
    account_type: String,
    balance: Decimal,
}

/// Profit & Loss report.
async fn profit_and_loss(engine: &ErpEngine, params: ReportParameters) -> ErpResult<ProfitAndLossReport> {
    let today = Utc::now().date_naive();
    let period_from = params.period_from.unwrap_or(NaiveDate::from_ymd_opt(today.year(), 1, 1).unwrap());
    let period_to = params.period_to.unwrap_or(today);

    let rows = sqlx::query_as::<_, PnlQueryRow>(
        r#"SELECT 
               a.code, a.name, a.account_type,
               COALESCE(SUM(COALESCE(jl.functional_debit, 0) - COALESCE(jl.functional_credit, 0)), 0) as balance
           FROM accounts a
           LEFT JOIN journal_lines jl ON jl.account_code = a.code
           LEFT JOIN journal_entries je ON je.id = jl.entry_id AND je.entity_id = a.entity_id AND je.status = 'posted'
               AND je.date >= $2 AND je.date <= $3
           WHERE a.entity_id = $1 AND a.is_active = true
           AND a.account_type IN ('revenue', 'contra_revenue', 'expense', 'contra_expense')
           GROUP BY a.code, a.name, a.account_type
           HAVING COALESCE(SUM(COALESCE(jl.functional_debit, 0) - COALESCE(jl.functional_credit, 0)), 0) != 0
           ORDER BY a.code"#,
    )
    .bind(engine.entity_id())
    .bind(period_from)
    .bind(period_to)
    .fetch_all(engine.pool())
    .await?;

    let mut revenue_lines = Vec::new();
    let mut cogs_lines = Vec::new();
    let mut opex_lines = Vec::new();
    let mut other_lines = Vec::new();

    for row in &rows {
        let line = PnlLine {
            account_code: row.code.clone(),
            account_name: row.name.clone(),
            amount: row.balance.abs(),
            comparative: None,
        };
        let code_num: u32 = row.code.parse().unwrap_or(0);
        match row.account_type.as_str() {
            "revenue" | "contra_revenue" => revenue_lines.push(line),
            "expense" | "contra_expense" => {
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
async fn cash_flow(engine: &ErpEngine, params: ReportParameters) -> ErpResult<CashFlowReport> {
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
    .bind(engine.entity_id())
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
    .bind(engine.entity_id())
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
    .bind(engine.entity_id())
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
    .bind(engine.entity_id())
    .bind(period_from)
    .bind(period_to)
    .fetch_one(engine.pool())
    .await
    .unwrap_or(Decimal::ZERO);

    // --- Changes in working capital ---
    // Change in AR (increase = cash outflow, decrease = cash inflow)
    let ar_change = working_capital_change(engine, "1200", "1299", period_from, period_to).await?;

    // Change in AP (increase = cash inflow, decrease = cash outflow)
    let ap_change = working_capital_change(engine, "3000", "3099", period_from, period_to).await?;

    // Change in Inventory (increase = cash outflow)
    let inventory_change = working_capital_change(engine, "1300", "1399", period_from, period_to).await?;

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
    .bind(engine.entity_id())
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
    let loan_movements = working_capital_change(engine, "3200", "3999", period_from, period_to).await?;
    let equity_movements = working_capital_change(engine, "4000", "4999", period_from, period_to).await?;

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
    .bind(engine.entity_id())
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
    .bind(engine.entity_id())
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
async fn ar_ageing(engine: &ErpEngine, params: ReportParameters) -> ErpResult<AgeingReport> {
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
    .bind(engine.entity_id())
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
async fn ap_ageing(engine: &ErpEngine, params: ReportParameters) -> ErpResult<AgeingReport> {
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
    .bind(engine.entity_id())
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

/// GL detail report.
async fn gl_detail(engine: &ErpEngine, params: ReportParameters) -> ErpResult<GlDetailReport> {
    let account_code = params.account_code.unwrap_or_default();
    let today = Utc::now().date_naive();
    let period_from = params.period_from.unwrap_or(NaiveDate::from_ymd_opt(today.year(), 1, 1).unwrap());
    let period_to = params.period_to.unwrap_or(today);

    let account_name = sqlx::query_scalar::<_, String>(
        "SELECT name FROM accounts WHERE entity_id = $1 AND code = $2",
    )
    .bind(engine.entity_id())
    .bind(&account_code)
    .fetch_optional(engine.pool())
    .await?
    .unwrap_or_else(|| account_code.clone());

    let rows = sqlx::query_as::<_, GlDetailQueryRow>(
        r#"SELECT 
               je.date, je.number as journal_number, je.description, je.reference,
               COALESCE(jl.functional_debit, 0) as debit,
               COALESCE(jl.functional_credit, 0) as credit
           FROM journal_lines jl
           JOIN journal_entries je ON je.id = jl.entry_id
           WHERE je.entity_id = $1 AND je.status = 'posted'
           AND jl.account_code = $2
           AND je.date >= $3 AND je.date <= $4
           ORDER BY je.date, je.number"#,
    )
    .bind(engine.entity_id())
    .bind(&account_code)
    .bind(period_from)
    .bind(period_to)
    .fetch_all(engine.pool())
    .await?;

    let mut balance = Decimal::ZERO;
    let lines: Vec<GlDetailLine> = rows
        .iter()
        .map(|r| {
            balance += r.debit - r.credit;
            GlDetailLine {
                date: r.date,
                journal_number: r.journal_number.clone(),
                description: r.description.clone(),
                reference: r.reference.clone(),
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
        opening_balance: Decimal::ZERO, // TODO: compute from prior periods
        lines,
        closing_balance: balance,
    })
}

#[derive(Debug, sqlx::FromRow)]
struct GlDetailQueryRow {
    date: NaiveDate,
    journal_number: String,
    description: String,
    reference: String,
    debit: Decimal,
    credit: Decimal,
}
