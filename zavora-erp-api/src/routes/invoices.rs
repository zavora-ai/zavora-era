use axum::{extract::{Path, State}, Json};
use std::sync::Arc;
use uuid::Uuid;

use crate::AppState;
use super::err_response;
use zavora_erp_core::invoicing::*;
use zavora_erp_core::services::invoicing as svc;
use zavora_erp_core::AgentOrUserId;

pub async fn list(
    State(state): State<Arc<AppState>>,
) -> Result<Json<serde_json::Value>, impl axum::response::IntoResponse> {
    let rows = sqlx::query_as::<_, InvoiceRow>(
        "SELECT * FROM invoices WHERE entity_id = $1 ORDER BY created_at DESC",
    )
    .bind(state.engine.entity_id())
    .fetch_all(state.engine.pool())
    .await;
    match rows {
        Ok(r) => Ok(Json(serde_json::to_value(r).unwrap_or_default())),
        Err(e) => Err(err_response(zavora_erp_core::ErpError::Database(e))),
    }
}

pub async fn get_one(
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, impl axum::response::IntoResponse> {
    let invoice = sqlx::query_as::<_, InvoiceRow>(
        "SELECT * FROM invoices WHERE id = $1 AND entity_id = $2",
    )
    .bind(id).bind(state.engine.entity_id())
    .fetch_optional(state.engine.pool()).await;

    let lines = sqlx::query_as::<_, InvoiceLineRow>(
        "SELECT * FROM invoice_lines WHERE invoice_id = $1",
    )
    .bind(id)
    .fetch_all(state.engine.pool()).await.unwrap_or_default();

    match invoice {
        Ok(Some(inv)) => Ok(Json(serde_json::json!({
            "invoice": serde_json::to_value(inv).unwrap_or_default(),
            "lines": serde_json::to_value(lines).unwrap_or_default(),
        }))),
        Ok(None) => Err(err_response(zavora_erp_core::ErpError::NotFound { entity_type: "Invoice".into(), id })),
        Err(e) => Err(err_response(zavora_erp_core::ErpError::Database(e))),
    }
}

pub async fn create(
    State(state): State<Arc<AppState>>,
    Json(req): Json<CreateInvoiceRequest>,
) -> Result<Json<serde_json::Value>, impl axum::response::IntoResponse> {
    let actor = AgentOrUserId::Agent("api".to_string());
    match svc::create_invoice(&state.engine, req, &actor).await {
        Ok(invoice) => Ok(Json(serde_json::to_value(invoice).unwrap_or_default())),
        Err(e) => Err(err_response(e)),
    }
}

pub async fn post_invoice(
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, impl axum::response::IntoResponse> {
    let actor = AgentOrUserId::Agent("api".to_string());
    match svc::post_invoice(&state.engine, id, &actor).await {
        Ok(je_id) => Ok(Json(serde_json::json!({ "journal_entry_id": je_id }))),
        Err(e) => Err(err_response(e)),
    }
}

pub async fn send(
    State(_state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
    Json(_req): Json<SendInvoiceRequest>,
) -> Json<serde_json::Value> {
    Json(serde_json::json!({ "status": "queued", "invoice_id": id }))
}

pub async fn create_credit_note(
    State(_state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
    Json(req): Json<zavora_erp_core::invoicing::CreateCreditNoteRequest>,
) -> Json<serde_json::Value> {
    // TODO: wire to credit note service
    Json(serde_json::json!({ "status": "created", "invoice_id": id, "reason": req.reason }))
}

pub async fn list_recurring(
    State(state): State<Arc<AppState>>,
) -> Result<Json<serde_json::Value>, impl axum::response::IntoResponse> {
    let rows = sqlx::query_as::<_, RecurringInvoiceRow>(
        "SELECT * FROM recurring_invoices WHERE entity_id = $1 ORDER BY next_run",
    )
    .bind(state.engine.entity_id())
    .fetch_all(state.engine.pool())
    .await;
    match rows {
        Ok(r) => Ok(Json(serde_json::to_value(r).unwrap_or_default())),
        Err(e) => Err(err_response(zavora_erp_core::ErpError::Database(e))),
    }
}

pub async fn create_recurring(
    State(_state): State<Arc<AppState>>,
    Json(req): Json<CreateRecurringInvoiceRequest>,
) -> Json<serde_json::Value> {
    Json(serde_json::json!({ "status": "created", "customer_id": req.customer_id }))
}
