use axum::{extract::{Path, Query, State}, Json};
use std::sync::Arc;
use uuid::Uuid;

use crate::AppState;
use super::err_response;
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
    State(state): State<Arc<AppState>>,
    Query(q): Query<QueueQuery>,
) -> Result<Json<serde_json::Value>, impl axum::response::IntoResponse> {
    let query = TransactionQueueQuery {
        entity_id: state.engine.entity_id(),
        bank_account_id: q.bank_account_id,
        status: None, // TODO: parse status string
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
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
    Json(req): Json<CategoriseRequest>,
) -> Result<Json<serde_json::Value>, impl axum::response::IntoResponse> {
    let mut cat_req = req;
    cat_req.transaction_id = id;
    match svc::categorise(&state.engine, cat_req).await {
        Ok(()) => Ok(Json(serde_json::json!({ "status": "categorised" }))),
        Err(e) => Err(err_response(e)),
    }
}

pub async fn split(
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
    Json(req): Json<SplitRequest>,
) -> Result<Json<serde_json::Value>, impl axum::response::IntoResponse> {
    let mut split_req = req;
    split_req.transaction_id = id;
    match svc::split_transaction(&state.engine, split_req).await {
        Ok(ids) => Ok(Json(serde_json::json!({ "status": "split", "child_ids": ids }))),
        Err(e) => Err(err_response(e)),
    }
}

pub async fn merge(
    State(state): State<Arc<AppState>>,
    Json(req): Json<MergeRequest>,
) -> Result<Json<serde_json::Value>, impl axum::response::IntoResponse> {
    match svc::merge_transactions(&state.engine, req).await {
        Ok(()) => Ok(Json(serde_json::json!({ "status": "merged" }))),
        Err(e) => Err(err_response(e)),
    }
}

pub async fn exclude(
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
    Json(req): Json<ExcludeRequest>,
) -> Json<serde_json::Value> {
    sqlx::query("UPDATE imported_transactions SET category_status = 'excluded' WHERE id = $1 AND entity_id = $2")
        .bind(id)
        .bind(state.engine.entity_id())
        .execute(state.engine.pool())
        .await
        .ok();
    Json(serde_json::json!({ "status": "excluded" }))
}
