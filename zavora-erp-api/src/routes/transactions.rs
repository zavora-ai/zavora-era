use axum::{extract::{Path, Query, State}, Json};
use std::sync::Arc;
use uuid::Uuid;

use crate::AppState;
use super::err_response;
use crate::middleware::auth::{require_role, AuthContext, ROLES_CREATE};
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
    match svc::get_queue(&state.engine, query).await {
        Ok(rows) => Ok(Json(serde_json::to_value(rows).unwrap_or_default())),
        Err(e) => Err(err_response(e)),
    }
}

pub async fn categorise(
    ctx: AuthContext,
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
    Json(req): Json<CategoriseRequest>,
) -> Result<Json<serde_json::Value>, impl axum::response::IntoResponse> {
    require_role(ROLES_CREATE, &ctx, "categorise transaction").map_err(err_response)?;
    let mut cat_req = req;
    cat_req.transaction_id = id;
    match svc::categorise(&state.engine, ctx.entity_id, cat_req).await {
        Ok(()) => Ok(Json(serde_json::json!({ "status": "categorised" }))),
        Err(e) => Err(err_response(e)),
    }
}

pub async fn split(
    ctx: AuthContext,
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
    Json(req): Json<SplitRequest>,
) -> Result<Json<serde_json::Value>, impl axum::response::IntoResponse> {
    require_role(ROLES_CREATE, &ctx, "split transaction").map_err(err_response)?;
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
    require_role(ROLES_CREATE, &ctx, "merge transactions").map_err(err_response)?;
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
    if let Err(e) = require_role(ROLES_CREATE, &ctx, "exclude transaction") { return Err(err_response(e)); }
    sqlx::query("UPDATE imported_transactions SET category_status = 'excluded' WHERE id = $1 AND entity_id = $2")
        .bind(id)
        .bind(ctx.entity_id)
        .execute(state.engine.pool())
        .await
        .ok();
    Ok(Json(serde_json::json!({ "status": "excluded" })))
}
