use axum::{extract::{Path, Query, State}, Json};
use std::sync::Arc;
use uuid::Uuid;

use crate::AppState;
use super::err_response;
use crate::middleware::auth::{AuthContext};
use zavora_erp_core::transactions::*;
use zavora_erp_core::services::transactions as svc;

#[derive(serde::Deserialize)]
pub struct QueueQuery {
    pub status: Option<String>,
    pub bank_account_id: Option<Uuid>,
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}

pub async fn list(
    ctx: AuthContext,
    State(state): State<Arc<AppState>>,
    Query(q): Query<QueueQuery>,
) -> Result<Json<serde_json::Value>, impl axum::response::IntoResponse> {
    let query = TransactionQueueQuery {
        entity_id: ctx.entity_id,
        bank_account_id: q.bank_account_id,
        status: q.status.as_deref().and_then(CategoryStatus::from_db_str),
        date_from: None,
        date_to: None,
        search: None,
        limit: q.limit,
        offset: q.offset,
    };
    let rows = match svc::get_queue(&state.engine, query).await {
        Ok(rows) => rows,
        Err(e) => return Err(err_response(e)),
    };

    // Resolve account code -> name once for assigned-account display.
    let names: std::collections::HashMap<String, String> = sqlx::query_as::<_, (String, String)>(
        "SELECT code, name FROM accounts WHERE entity_id = $1",
    )
    .bind(ctx.entity_id)
    .fetch_all(state.engine.pool())
    .await
    .unwrap_or_default()
    .into_iter()
    .collect();

    // Resolve bank-account id -> currency so each line displays in the
    // statement's own currency (a USD account's amounts are USD, not KES).
    let currencies: std::collections::HashMap<uuid::Uuid, String> =
        sqlx::query_as::<_, (uuid::Uuid, String)>(
            "SELECT id, currency FROM bank_accounts WHERE entity_id = $1",
        )
        .bind(ctx.entity_id)
        .fetch_all(state.engine.pool())
        .await
        .unwrap_or_default()
        .into_iter()
        .map(|(id, c)| (id, c.trim().to_string()))
        .collect();

    // Map the raw DB rows onto the contract the UI consumes (status/amount/date,
    // signed amount = credit − debit, resolved account names).
    let out: Vec<serde_json::Value> = rows
        .into_iter()
        .map(|r| {
            use rust_decimal::prelude::ToPrimitive;
            let amount = (r.credit.unwrap_or_default() - r.debit.unwrap_or_default())
                .to_f64()
                .unwrap_or(0.0);
            let assigned_name = r.assigned_account.as_ref().and_then(|c| names.get(c).cloned());
            let currency = currencies.get(&r.bank_account).cloned().unwrap_or_else(|| "KES".to_string());
            serde_json::json!({
                "id": r.id,
                "entity_id": r.entity_id,
                "bank_account_id": r.bank_account,
                "currency": currency,
                "date": r.value_date,
                "description": r.description,
                "reference": r.reference,
                "amount": amount,
                "status": r.category_status,
                "suggestion": r.suggestion,
                "assigned_account_code": r.assigned_account,
                "assigned_account_name": assigned_name,
                "created_at": r.created_at,
            })
        })
        .collect();

    Ok(Json(serde_json::Value::Array(out)))
}

pub async fn categorise(
    ctx: AuthContext,
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
    Json(req): Json<CategoriseRequest>,
) -> Result<Json<serde_json::Value>, impl axum::response::IntoResponse> {
    let mut cat_req = req;
    cat_req.transaction_id = id;
    cat_req.categorised_by = zavora_erp_core::AgentOrUserId::User(ctx.user_id);
    match svc::categorise(&state.engine, ctx.entity_id, cat_req).await {
        Ok(()) => Ok(Json(serde_json::json!({ "status": "posted" }))),
        Err(e) => Err(err_response(e)),
    }
}

pub async fn split(
    ctx: AuthContext,
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
    Json(req): Json<SplitRequest>,
) -> Result<Json<serde_json::Value>, impl axum::response::IntoResponse> {
    let mut split_req = req;
    split_req.transaction_id = id;
    match svc::split_transaction(&state.engine, ctx.entity_id, split_req).await {
        Ok(ids) => Ok(Json(serde_json::json!({ "status": "split", "child_ids": ids }))),
        Err(e) => Err(err_response(e)),
    }
}

pub async fn merge(
    ctx: AuthContext,
    State(state): State<Arc<AppState>>,
    Json(req): Json<MergeRequest>,
) -> Result<Json<serde_json::Value>, impl axum::response::IntoResponse> {
    match svc::merge_transactions(&state.engine, ctx.entity_id, req).await {
        Ok(()) => Ok(Json(serde_json::json!({ "status": "merged" }))),
        Err(e) => Err(err_response(e)),
    }
}

pub async fn exclude(
    ctx: AuthContext,
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
    Json(_req): Json<ExcludeRequest>,
) -> Result<Json<serde_json::Value>, impl axum::response::IntoResponse> {
    sqlx::query("UPDATE imported_transactions SET category_status = 'excluded' WHERE id = $1 AND entity_id = $2")
        .bind(id)
        .bind(ctx.entity_id)
        .execute(state.engine.pool())
        .await
        .ok();
    Ok::<_, axum::response::Response>(Json(serde_json::json!({ "status": "excluded" })))
}
