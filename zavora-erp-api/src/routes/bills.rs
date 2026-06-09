use axum::{extract::{Path, State}, Json};
use std::sync::Arc;
use uuid::Uuid;

use crate::AppState;
use super::err_response;
use zavora_erp_core::ap::*;
use zavora_erp_core::services::bills as svc;
use zavora_erp_core::AgentOrUserId;

pub async fn create(
    State(state): State<Arc<AppState>>,
    Json(req): Json<CreateBillRequest>,
) -> Result<Json<serde_json::Value>, impl axum::response::IntoResponse> {
    let actor = AgentOrUserId::Agent("api".to_string());
    match svc::create_bill(&state.engine, req, &actor).await {
        Ok(bill) => Ok(Json(serde_json::to_value(bill).unwrap_or_default())),
        Err(e) => Err(err_response(e)),
    }
}

pub async fn approve(
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, impl axum::response::IntoResponse> {
    let req = ApproveBillRequest {
        bill_id: id,
        approved_by: Uuid::new_v4(), // TODO: from auth context
    };
    match svc::approve_bill(&state.engine, req).await {
        Ok(()) => Ok(Json(serde_json::json!({ "status": "approved" }))),
        Err(e) => Err(err_response(e)),
    }
}
