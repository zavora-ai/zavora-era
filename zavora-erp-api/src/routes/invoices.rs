use axum::{extract::{Path, State}, Json};
use std::sync::Arc;
use uuid::Uuid;

use crate::AppState;
use super::err_response;
use zavora_erp_core::invoicing::*;
use zavora_erp_core::services::invoicing as svc;
use zavora_erp_core::AgentOrUserId;

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
    // TODO: integrate with notification service for email/WhatsApp/SMS delivery
    Json(serde_json::json!({ "status": "queued", "invoice_id": id }))
}
