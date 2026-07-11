//! Projects v1 — job/project accounting for NGOs (grants/funds) and construction
//! (job costing).
//!
//! A project is a first-class record backed by a **`PROJECT` GL dimension
//! value** (created here on project create/update). Because invoices, bills and
//! journals already carry `dimensions` ({type_code: value_code}) that propagate
//! to `journal_lines` at posting, tagging a document to a project makes its cost
//! or revenue roll up through the **real ledger** — so budget-vs-actual and
//! profitability are actuals, not a parallel silo.

use chrono::{DateTime, NaiveDate, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

use crate::engine::ErpEngine;
use crate::error::{ErpError, ErpResult};

type PgTx<'a> = sqlx::Transaction<'a, sqlx::Postgres>;

/// The dimension type code every project is tagged under.
pub const PROJECT_DIMENSION: &str = "PROJECT";

// ─── Models ──────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, FromRow)]
pub struct ProjectRow {
    pub id: Uuid,
    pub entity_id: Uuid,
    pub code: String,
    pub name: String,
    pub client_id: Option<Uuid>,
    pub donor: Option<String>,
    pub manager: Option<String>,
    pub status: String,
    pub billing_method: String,
    pub budget_amount: Decimal,
    pub currency: String,
    pub start_date: Option<NaiveDate>,
    pub end_date: Option<NaiveDate>,
    pub notes: Option<String>,
    pub is_active: bool,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, FromRow)]
pub struct ProjectBudgetLine {
    pub id: Uuid,
    pub project_id: Uuid,
    pub category: String,
    pub account_code: Option<String>,
    pub amount: Decimal,
    pub notes: Option<String>,
}

#[derive(Debug, Clone, Serialize, FromRow)]
pub struct ProjectTask {
    pub id: Uuid,
    pub project_id: Uuid,
    pub name: String,
    pub budget_hours: Decimal,
    pub budget_amount: Decimal,
    pub status: String,
    pub sort_order: i32,
}

#[derive(Debug, Clone, Serialize)]
pub struct Project {
    #[serde(flatten)]
    pub row: ProjectRow,
    pub client_name: Option<String>,
    pub budget_lines: Vec<ProjectBudgetLine>,
    pub tasks: Vec<ProjectTask>,
}

// ─── Requests ────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Deserialize)]
pub struct BudgetLineInput {
    pub category: String,
    #[serde(default)]
    pub account_code: Option<String>,
    #[serde(default)]
    pub amount: Decimal,
    #[serde(default)]
    pub notes: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct TaskInput {
    pub name: String,
    #[serde(default)]
    pub budget_hours: Decimal,
    #[serde(default)]
    pub budget_amount: Decimal,
    #[serde(default)]
    pub status: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CreateProjectRequest {
    pub code: String,
    pub name: String,
    #[serde(default)]
    pub client_id: Option<Uuid>,
    #[serde(default)]
    pub donor: Option<String>,
    #[serde(default)]
    pub manager: Option<String>,
    #[serde(default)]
    pub status: Option<String>,
    #[serde(default)]
    pub billing_method: Option<String>,
    #[serde(default)]
    pub budget_amount: Decimal,
    #[serde(default)]
    pub currency: Option<String>,
    #[serde(default)]
    pub start_date: Option<NaiveDate>,
    #[serde(default)]
    pub end_date: Option<NaiveDate>,
    #[serde(default)]
    pub notes: Option<String>,
    #[serde(default)]
    pub budget_lines: Vec<BudgetLineInput>,
    #[serde(default)]
    pub tasks: Vec<TaskInput>,
}

// ─── Summary (budget vs actual + profitability) ──────────────────────────────

#[derive(Debug, Clone, Serialize)]
pub struct AccountActual {
    pub account_code: String,
    pub account_name: String,
    pub account_type: String,
    /// Net movement on the account for this project (debit − credit).
    pub amount: Decimal,
}

#[derive(Debug, Clone, Serialize)]
pub struct BudgetVsActualLine {
    pub category: String,
    pub account_code: Option<String>,
    pub budgeted: Decimal,
    pub actual: Decimal,
    pub variance: Decimal,
}

#[derive(Debug, Clone, Serialize)]
pub struct ProjectSummary {
    pub budget_total: Decimal,
    /// Revenue tagged to the project (Σ credit−debit on Revenue accounts).
    pub revenue: Decimal,
    /// Cost tagged to the project (Σ debit−credit on Expense accounts).
    pub cost: Decimal,
    /// revenue − cost.
    pub margin: Decimal,
    /// Cost as a % of the total budget (0 when no budget).
    pub budget_used_pct: Decimal,
    pub budget_vs_actual: Vec<BudgetVsActualLine>,
    pub actuals_by_account: Vec<AccountActual>,
}

// ─── Dimension bootstrap ─────────────────────────────────────────────────────

async fn ensure_project_dimension_tx(tx: &mut PgTx<'_>, entity_id: Uuid, code: &str, name: &str) -> ErpResult<()> {
    sqlx::query(
        "INSERT INTO dimension_types (entity_id, code, name) VALUES ($1, $2, 'Project')
         ON CONFLICT (entity_id, code) DO NOTHING",
    )
    .bind(entity_id)
    .bind(PROJECT_DIMENSION)
    .execute(&mut **tx)
    .await?;
    sqlx::query(
        "INSERT INTO dimension_values (entity_id, type_code, code, name) VALUES ($1, $2, $3, $4)
         ON CONFLICT (entity_id, type_code, code) DO UPDATE SET name = EXCLUDED.name, is_active = true",
    )
    .bind(entity_id)
    .bind(PROJECT_DIMENSION)
    .bind(code)
    .bind(name)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

// ─── CRUD ────────────────────────────────────────────────────────────────────

fn validate(req: &CreateProjectRequest) -> ErpResult<()> {
    if req.code.trim().is_empty() {
        return Err(ErpError::ValidationFailed { message: "Project code is required.".into() });
    }
    if req.name.trim().is_empty() {
        return Err(ErpError::ValidationFailed { message: "Project name is required.".into() });
    }
    Ok(())
}

pub async fn create_project(engine: &ErpEngine, entity_id: Uuid, req: CreateProjectRequest) -> ErpResult<Uuid> {
    validate(&req)?;
    let code = req.code.trim().to_string();
    let exists: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM projects WHERE entity_id = $1 AND code = $2)")
        .bind(entity_id)
        .bind(&code)
        .fetch_one(engine.pool())
        .await?;
    if exists {
        return Err(ErpError::Duplicate { message: format!("A project with code '{code}' already exists.") });
    }

    let id = Uuid::new_v4();
    let mut tx = engine.pool().begin().await?;
    sqlx::query(
        r#"INSERT INTO projects
           (id, entity_id, code, name, client_id, donor, manager, status, billing_method,
            budget_amount, currency, start_date, end_date, notes, is_active, created_at)
           VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14,true,$15)"#,
    )
    .bind(id)
    .bind(entity_id)
    .bind(&code)
    .bind(req.name.trim())
    .bind(req.client_id)
    .bind(req.donor.as_deref())
    .bind(req.manager.as_deref())
    .bind(req.status.as_deref().unwrap_or("active"))
    .bind(req.billing_method.as_deref().unwrap_or("time_and_materials"))
    .bind(req.budget_amount)
    .bind(req.currency.as_deref().unwrap_or("KES"))
    .bind(req.start_date)
    .bind(req.end_date)
    .bind(req.notes.as_deref())
    .bind(Utc::now())
    .execute(&mut *tx)
    .await?;
    replace_budget_lines(&mut tx, id, &req.budget_lines).await?;
    replace_tasks(&mut tx, id, &req.tasks).await?;
    ensure_project_dimension_tx(&mut tx, entity_id, &code, req.name.trim()).await?;
    tx.commit().await?;
    Ok(id)
}

pub async fn update_project(engine: &ErpEngine, entity_id: Uuid, id: Uuid, req: CreateProjectRequest) -> ErpResult<()> {
    validate(&req)?;
    let existing = sqlx::query_as::<_, ProjectRow>("SELECT * FROM projects WHERE id = $1 AND entity_id = $2")
        .bind(id)
        .bind(entity_id)
        .fetch_optional(engine.pool())
        .await?
        .ok_or_else(|| ErpError::NotFound { entity_type: "Project".into(), id })?;

    let mut tx = engine.pool().begin().await?;
    // The code is the dimension value key — keep it stable across edits.
    sqlx::query(
        r#"UPDATE projects SET name=$1, client_id=$2, donor=$3, manager=$4, status=$5,
           billing_method=$6, budget_amount=$7, currency=$8, start_date=$9, end_date=$10, notes=$11
           WHERE id=$12 AND entity_id=$13"#,
    )
    .bind(req.name.trim())
    .bind(req.client_id)
    .bind(req.donor.as_deref())
    .bind(req.manager.as_deref())
    .bind(req.status.as_deref().unwrap_or("active"))
    .bind(req.billing_method.as_deref().unwrap_or("time_and_materials"))
    .bind(req.budget_amount)
    .bind(req.currency.as_deref().unwrap_or("KES"))
    .bind(req.start_date)
    .bind(req.end_date)
    .bind(req.notes.as_deref())
    .bind(id)
    .bind(entity_id)
    .execute(&mut *tx)
    .await?;
    replace_budget_lines(&mut tx, id, &req.budget_lines).await?;
    replace_tasks(&mut tx, id, &req.tasks).await?;
    ensure_project_dimension_tx(&mut tx, entity_id, &existing.code, req.name.trim()).await?;
    tx.commit().await?;
    Ok(())
}

async fn replace_budget_lines(tx: &mut PgTx<'_>, project_id: Uuid, lines: &[BudgetLineInput]) -> ErpResult<()> {
    sqlx::query("DELETE FROM project_budget_lines WHERE project_id = $1").bind(project_id).execute(&mut **tx).await?;
    for l in lines {
        if l.category.trim().is_empty() {
            continue;
        }
        sqlx::query(
            "INSERT INTO project_budget_lines (id, project_id, category, account_code, amount, notes) VALUES ($1,$2,$3,$4,$5,$6)",
        )
        .bind(Uuid::new_v4())
        .bind(project_id)
        .bind(l.category.trim())
        .bind(l.account_code.as_deref().filter(|s| !s.trim().is_empty()))
        .bind(l.amount)
        .bind(l.notes.as_deref())
        .execute(&mut **tx)
        .await?;
    }
    Ok(())
}

async fn replace_tasks(tx: &mut PgTx<'_>, project_id: Uuid, tasks: &[TaskInput]) -> ErpResult<()> {
    sqlx::query("DELETE FROM project_tasks WHERE project_id = $1").bind(project_id).execute(&mut **tx).await?;
    for (i, t) in tasks.iter().enumerate() {
        if t.name.trim().is_empty() {
            continue;
        }
        sqlx::query(
            "INSERT INTO project_tasks (id, project_id, name, budget_hours, budget_amount, status, sort_order) VALUES ($1,$2,$3,$4,$5,$6,$7)",
        )
        .bind(Uuid::new_v4())
        .bind(project_id)
        .bind(t.name.trim())
        .bind(t.budget_hours)
        .bind(t.budget_amount)
        .bind(t.status.as_deref().unwrap_or("open"))
        .bind(i as i32)
        .execute(&mut **tx)
        .await?;
    }
    Ok(())
}

pub async fn list_projects(engine: &ErpEngine, entity_id: Uuid) -> ErpResult<Vec<Project>> {
    let rows = sqlx::query_as::<_, ProjectRow>("SELECT * FROM projects WHERE entity_id = $1 ORDER BY created_at DESC")
        .bind(entity_id)
        .fetch_all(engine.pool())
        .await?;
    let mut out = Vec::with_capacity(rows.len());
    for row in rows {
        out.push(hydrate(engine, entity_id, row).await?);
    }
    Ok(out)
}

pub async fn get_project(engine: &ErpEngine, entity_id: Uuid, id: Uuid) -> ErpResult<Project> {
    let row = sqlx::query_as::<_, ProjectRow>("SELECT * FROM projects WHERE id = $1 AND entity_id = $2")
        .bind(id)
        .bind(entity_id)
        .fetch_optional(engine.pool())
        .await?
        .ok_or_else(|| ErpError::NotFound { entity_type: "Project".into(), id })?;
    hydrate(engine, entity_id, row).await
}

async fn hydrate(engine: &ErpEngine, entity_id: Uuid, row: ProjectRow) -> ErpResult<Project> {
    let client_name: Option<String> = match row.client_id {
        Some(cid) => sqlx::query_scalar("SELECT name FROM customers WHERE id = $1 AND entity_id = $2")
            .bind(cid)
            .bind(entity_id)
            .fetch_optional(engine.pool())
            .await?,
        None => None,
    };
    let budget_lines = sqlx::query_as::<_, ProjectBudgetLine>("SELECT * FROM project_budget_lines WHERE project_id = $1 ORDER BY category")
        .bind(row.id)
        .fetch_all(engine.pool())
        .await?;
    let tasks = sqlx::query_as::<_, ProjectTask>("SELECT * FROM project_tasks WHERE project_id = $1 ORDER BY sort_order, name")
        .bind(row.id)
        .fetch_all(engine.pool())
        .await?;
    Ok(Project { row, client_name, budget_lines, tasks })
}

// ─── Summary: budget vs actual + profitability from the GL ───────────────────

pub async fn project_summary(engine: &ErpEngine, entity_id: Uuid, id: Uuid) -> ErpResult<ProjectSummary> {
    let project = get_project(engine, entity_id, id).await?;

    // Actuals: net movement per account for postings tagged to this project's
    // PROJECT dimension. `dimensions` is JSONB {type_code: value_code}.
    let rows = sqlx::query_as::<_, (String, String, String, Decimal, Decimal)>(
        r#"SELECT a.code, a.name, a.account_type,
                  COALESCE(SUM(jl.debit),0)::numeric AS dr,
                  COALESCE(SUM(jl.credit),0)::numeric AS cr
           FROM journal_lines jl
           JOIN journal_entries je ON jl.entry_id = je.id
           JOIN accounts a ON a.entity_id = je.entity_id AND a.code = jl.account_code
           WHERE je.entity_id = $1 AND je.status = 'posted' AND jl.dimensions->>'PROJECT' = $2
           GROUP BY a.code, a.name, a.account_type
           ORDER BY a.code"#,
    )
    .bind(entity_id)
    .bind(&project.row.code)
    .fetch_all(engine.pool())
    .await?;

    let mut revenue = Decimal::ZERO;
    let mut cost = Decimal::ZERO;
    let mut actuals_by_account = Vec::new();
    // account_code -> cost actual (debit − credit), for budget-vs-actual matching.
    let mut cost_by_account: std::collections::HashMap<String, Decimal> = std::collections::HashMap::new();
    for (code, name, atype, dr, cr) in rows {
        let net = dr - cr; // debit-positive
        match atype.as_str() {
            "Revenue" | "ContraRevenue" => revenue += cr - dr,
            "Expense" | "ContraExpense" => {
                cost += net;
                cost_by_account.insert(code.clone(), net);
            }
            _ => {}
        }
        actuals_by_account.push(AccountActual { account_code: code, account_name: name, account_type: atype, amount: net });
    }

    let budget_total: Decimal = if project.budget_lines.is_empty() {
        project.row.budget_amount
    } else {
        project.budget_lines.iter().map(|l| l.amount).sum()
    };

    let budget_vs_actual = project
        .budget_lines
        .iter()
        .map(|l| {
            let actual = l
                .account_code
                .as_deref()
                .and_then(|c| cost_by_account.get(c).copied())
                .unwrap_or(Decimal::ZERO);
            BudgetVsActualLine {
                category: l.category.clone(),
                account_code: l.account_code.clone(),
                budgeted: l.amount,
                actual,
                variance: l.amount - actual,
            }
        })
        .collect();

    let budget_used_pct = if budget_total > Decimal::ZERO {
        (cost / budget_total * Decimal::from(100)).round_dp(1)
    } else {
        Decimal::ZERO
    };

    Ok(ProjectSummary {
        budget_total,
        revenue,
        cost,
        margin: revenue - cost,
        budget_used_pct,
        budget_vs_actual,
        actuals_by_account,
    })
}
