use axum::{extract::{Path, State}, Json};
use std::sync::Arc;
use uuid::Uuid;

use crate::AppState;
use super::err_response;
use zavora_erp_core::bank::*;
use zavora_erp_core::services::bank as svc;
use zavora_erp_core::AgentOrUserId;

pub async fn list_accounts(
    State(state): State<Arc<AppState>>,
) -> Result<Json<serde_json::Value>, impl axum::response::IntoResponse> {
    let rows = sqlx::query_as::<_, BankAccountRow>(
        "SELECT * FROM bank_accounts WHERE entity_id = $1 AND is_active = true ORDER BY name",
    )
    .bind(state.engine.entity_id())
    .fetch_all(state.engine.pool())
    .await;
    match rows {
        Ok(r) => Ok(Json(serde_json::to_value(r).unwrap_or_default())),
        Err(e) => Err(err_response(zavora_erp_core::ErpError::Database(e))),
    }
}

pub async fn create_account(
    State(state): State<Arc<AppState>>,
    Json(req): Json<CreateBankAccountRequest>,
) -> Result<Json<serde_json::Value>, impl axum::response::IntoResponse> {
    let id = uuid::Uuid::new_v4();
    let currency = req.currency.unwrap_or_else(|| "KES".to_string());
    let gl = req.gl_account.unwrap_or_else(|| "1020".to_string());
    let result = sqlx::query(
        "INSERT INTO bank_accounts (id, entity_id, name, bank_name, account_number, currency, gl_account, feed_provider, feed_enabled) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)"
    )
    .bind(id).bind(state.engine.entity_id())
    .bind(&req.name).bind(&req.bank_name).bind(&req.account_number)
    .bind(&currency).bind(&gl)
    .bind(req.feed_provider.as_ref().map(|f| serde_json::to_string(f).unwrap_or_default()))
    .bind(req.feed_provider.is_some())
    .execute(state.engine.pool()).await;
    match result {
        Ok(_) => Ok(Json(serde_json::json!({ "id": id }))),
        Err(e) => Err(err_response(zavora_erp_core::ErpError::Database(e))),
    }
}

pub async fn import_statement(
    State(_state): State<Arc<AppState>>,
) -> Json<serde_json::Value> {
    // TODO: file upload parsing (CSV/MT940/OFX)
    Json(serde_json::json!({ "status": "import_endpoint_ready", "message": "Upload CSV/MT940/OFX file" }))
}

/// DELETE /bank-accounts/{id} — soft-delete a bank account (sets is_active = false).
pub async fn delete_account(
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, impl axum::response::IntoResponse> {
    let result = sqlx::query(
        "UPDATE bank_accounts SET is_active = false WHERE id = $1 AND entity_id = $2",
    )
    .bind(id)
    .bind(state.engine.entity_id())
    .execute(state.engine.pool())
    .await;
    match result {
        Ok(_) => Ok(Json(serde_json::json!({ "status": "deleted", "id": id }))),
        Err(e) => Err(err_response(zavora_erp_core::ErpError::Database(e))),
    }
}

pub async fn reconcile(
    State(state): State<Arc<AppState>>,
    Path(statement_id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, impl axum::response::IntoResponse> {
    match svc::match_bank_lines(&state.engine, statement_id).await {
        Ok(report) => Ok(Json(serde_json::to_value(report).unwrap_or_default())),
        Err(e) => Err(err_response(e)),
    }
}

pub async fn confirm_match(
    State(state): State<Arc<AppState>>,
    Json(req): Json<ConfirmMatchRequest>,
) -> Result<Json<serde_json::Value>, impl axum::response::IntoResponse> {
    match svc::confirm_match(&state.engine, req).await {
        Ok(()) => Ok(Json(serde_json::json!({ "status": "confirmed" }))),
        Err(e) => Err(err_response(e)),
    }
}
