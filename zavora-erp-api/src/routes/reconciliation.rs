use axum::{extract::{Query, State}, Json};
use rust_decimal::Decimal;
use std::sync::Arc;
use uuid::Uuid;

use crate::AppState;
use crate::middleware::auth::{AuthContext, require_role, ROLES_POST_JOURNAL};
use super::err_response;

async fn bank_gl_account(state: &AppState, entity_id: Uuid, bank_account_id: Uuid) -> Option<String> {
    sqlx::query_scalar::<_, String>("SELECT gl_account FROM bank_accounts WHERE id = $1 AND entity_id = $2")
        .bind(bank_account_id)
        .bind(entity_id)
        .fetch_optional(state.engine.pool())
        .await
        .ok()
        .flatten()
}

#[derive(serde::Deserialize)]
pub struct ComputeRequest {
    pub bank_account_id: Uuid,
    pub statement_date: chrono::NaiveDate,
}

/// POST /bank/reconciliations/compute — GL balance, already-cleared balance, and
/// the uncleared entries to be reconciled against the statement.
pub async fn compute(
    ctx: AuthContext,
    State(state): State<Arc<AppState>>,
    Json(req): Json<ComputeRequest>,
) -> Result<Json<serde_json::Value>, impl axum::response::IntoResponse> {
    let Some(gl) = bank_gl_account(&state, ctx.entity_id, req.bank_account_id).await else {
        return Err(err_response(zavora_erp_core::ErpError::NotFound { entity_type: "BankAccount".into(), id: req.bank_account_id }));
    };

    let gl_balance: Decimal = sqlx::query_scalar(
        "SELECT COALESCE(SUM(COALESCE(functional_debit,0) - COALESCE(functional_credit,0)), 0)
         FROM journal_lines WHERE entity_id = $1 AND account_code = $2 AND entry_date <= $3",
    )
    .bind(ctx.entity_id).bind(&gl).bind(req.statement_date)
    .fetch_one(state.engine.pool()).await.unwrap_or(Decimal::ZERO);

    let prior_cleared: Decimal = sqlx::query_scalar(
        "SELECT COALESCE(SUM(COALESCE(jl.functional_debit,0) - COALESCE(jl.functional_credit,0)), 0)
         FROM journal_lines jl JOIN journal_entries je ON je.id = jl.entry_id
         WHERE jl.entity_id = $1 AND jl.account_code = $2 AND jl.entry_date <= $3 AND je.reconciled = true",
    )
    .bind(ctx.entity_id).bind(&gl).bind(req.statement_date)
    .fetch_one(state.engine.pool()).await.unwrap_or(Decimal::ZERO);

    let uncleared = sqlx::query_as::<_, (Uuid, chrono::NaiveDate, String, String, Decimal)>(
        "SELECT je.id, je.date, je.number, je.reference,
                COALESCE(SUM(COALESCE(jl.functional_debit,0) - COALESCE(jl.functional_credit,0)), 0) AS amount
         FROM journal_lines jl JOIN journal_entries je ON je.id = jl.entry_id
         WHERE jl.entity_id = $1 AND jl.account_code = $2 AND jl.entry_date <= $3 AND je.reconciled = false
         GROUP BY je.id, je.date, je.number, je.reference
         ORDER BY je.date, je.number",
    )
    .bind(ctx.entity_id).bind(&gl).bind(req.statement_date)
    .fetch_all(state.engine.pool()).await.unwrap_or_default();

    let items: Vec<_> = uncleared.into_iter().map(|(id, date, number, reference, amount)| {
        serde_json::json!({ "journal_entry_id": id, "date": date, "number": number, "reference": reference, "amount": amount })
    }).collect();

    Ok(Json(serde_json::json!({ "gl_balance": gl_balance, "prior_cleared": prior_cleared, "uncleared": items })))
}

#[derive(serde::Deserialize)]
pub struct CompleteRequest {
    pub bank_account_id: Uuid,
    pub statement_date: chrono::NaiveDate,
    pub statement_closing_balance: Decimal,
    pub cleared_entry_ids: Vec<Uuid>,
}

/// POST /bank/reconciliations/complete — mark the selected entries cleared and
/// lock the rec, but only if cleared balance == statement closing balance.
pub async fn complete(
    ctx: AuthContext,
    State(state): State<Arc<AppState>>,
    Json(req): Json<CompleteRequest>,
) -> Result<Json<serde_json::Value>, impl axum::response::IntoResponse> {
    require_role(ROLES_POST_JOURNAL, &ctx, "complete bank reconciliation").map_err(err_response)?;
    let Some(gl) = bank_gl_account(&state, ctx.entity_id, req.bank_account_id).await else {
        return Err(err_response(zavora_erp_core::ErpError::NotFound { entity_type: "BankAccount".into(), id: req.bank_account_id }));
    };

    let prior_cleared: Decimal = sqlx::query_scalar(
        "SELECT COALESCE(SUM(COALESCE(jl.functional_debit,0) - COALESCE(jl.functional_credit,0)), 0)
         FROM journal_lines jl JOIN journal_entries je ON je.id = jl.entry_id
         WHERE jl.entity_id = $1 AND jl.account_code = $2 AND jl.entry_date <= $3 AND je.reconciled = true",
    )
    .bind(ctx.entity_id).bind(&gl).bind(req.statement_date)
    .fetch_one(state.engine.pool()).await.unwrap_or(Decimal::ZERO);

    let newly_cleared: Decimal = sqlx::query_scalar(
        "SELECT COALESCE(SUM(COALESCE(functional_debit,0) - COALESCE(functional_credit,0)), 0)
         FROM journal_lines WHERE entity_id = $1 AND account_code = $2 AND entry_id = ANY($3)",
    )
    .bind(ctx.entity_id).bind(&gl).bind(&req.cleared_entry_ids)
    .fetch_one(state.engine.pool()).await.unwrap_or(Decimal::ZERO);

    let total_cleared = prior_cleared + newly_cleared;
    let difference = req.statement_closing_balance - total_cleared;
    if difference.abs() >= Decimal::new(1, 2) {
        return Err(err_response(zavora_erp_core::ErpError::ValidationFailed {
            message: format!(
                "Reconciliation does not balance — cleared {} vs statement {} (difference {}). Tick the items that appear on the statement.",
                total_cleared, req.statement_closing_balance, difference
            ),
        }));
    }

    // Mark the newly-cleared entries reconciled (allowed by the relaxed trigger).
    if !req.cleared_entry_ids.is_empty() {
        sqlx::query("UPDATE journal_entries SET reconciled = true, reconciled_at = NOW() WHERE id = ANY($1) AND entity_id = $2")
            .bind(&req.cleared_entry_ids).bind(ctx.entity_id)
            .execute(state.engine.pool()).await
            .map_err(|e| err_response(zavora_erp_core::ErpError::Database(e)))?;
    }

    let gl_balance: Decimal = sqlx::query_scalar(
        "SELECT COALESCE(SUM(COALESCE(functional_debit,0) - COALESCE(functional_credit,0)), 0)
         FROM journal_lines WHERE entity_id = $1 AND account_code = $2 AND entry_date <= $3",
    )
    .bind(ctx.entity_id).bind(&gl).bind(req.statement_date)
    .fetch_one(state.engine.pool()).await.unwrap_or(Decimal::ZERO);

    let id = sqlx::query_scalar::<_, Uuid>(
        "INSERT INTO bank_reconciliations (entity_id, bank_account_id, statement_date, statement_closing_balance, gl_balance, cleared_balance, difference, completed_by)
         VALUES ($1,$2,$3,$4,$5,$6,$7,$8) RETURNING id",
    )
    .bind(ctx.entity_id).bind(req.bank_account_id).bind(req.statement_date)
    .bind(req.statement_closing_balance).bind(gl_balance).bind(total_cleared).bind(difference).bind(ctx.user_id)
    .fetch_one(state.engine.pool()).await
    .map_err(|e| err_response(zavora_erp_core::ErpError::Database(e)))?;

    Ok(Json(serde_json::json!({ "id": id, "cleared_balance": total_cleared, "gl_balance": gl_balance, "outstanding": gl_balance - total_cleared })))
}

#[derive(serde::Deserialize)]
pub struct ListQuery {
    pub bank_account_id: Option<Uuid>,
}

/// GET /bank/reconciliations — completed reconciliations.
pub async fn list(
    ctx: AuthContext,
    State(state): State<Arc<AppState>>,
    Query(q): Query<ListQuery>,
) -> Json<serde_json::Value> {
    let rows = sqlx::query_as::<_, (Uuid, Uuid, chrono::NaiveDate, Decimal, Decimal, Decimal, chrono::DateTime<chrono::Utc>)>(
        "SELECT id, bank_account_id, statement_date, statement_closing_balance, gl_balance, cleared_balance, completed_at
         FROM bank_reconciliations
         WHERE entity_id = $1 AND ($2::uuid IS NULL OR bank_account_id = $2)
         ORDER BY statement_date DESC, completed_at DESC",
    )
    .bind(ctx.entity_id).bind(q.bank_account_id)
    .fetch_all(state.engine.pool()).await.unwrap_or_default();
    let items: Vec<_> = rows.into_iter().map(|(id, bank_account_id, statement_date, scb, glb, cb, completed_at)| {
        serde_json::json!({ "id": id, "bank_account_id": bank_account_id, "statement_date": statement_date,
            "statement_closing_balance": scb, "gl_balance": glb, "cleared_balance": cb, "completed_at": completed_at })
    }).collect();
    Json(serde_json::to_value(items).unwrap_or_default())
}
