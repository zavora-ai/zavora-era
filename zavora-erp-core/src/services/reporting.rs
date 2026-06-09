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

/// Cash flow statement stub.
async fn cash_flow(_engine: &ErpEngine, params: ReportParameters) -> ErpResult<CashFlowReport> {
    let today = Utc::now().date_naive();
    Ok(CashFlowReport {
        period_from: params.period_from.unwrap_or(today),
        period_to: params.period_to.unwrap_or(today),
        operating_activities: CashFlowSection { lines: Vec::new(), total: Decimal::ZERO },
        investing_activities: CashFlowSection { lines: Vec::new(), total: Decimal::ZERO },
        financing_activities: CashFlowSection { lines: Vec::new(), total: Decimal::ZERO },
        net_change: Decimal::ZERO,
        opening_cash: Decimal::ZERO,
        closing_cash: Decimal::ZERO,
    })
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
