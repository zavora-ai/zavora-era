use axum::{extract::{Path, State}, Json};
use std::sync::Arc;
use uuid::Uuid;

use crate::AppState;
use super::err_response;
use zavora_erp_core::payroll::*;
use zavora_erp_core::services::payroll as svc;
use zavora_erp_core::AgentOrUserId;

pub async fn run(
    State(state): State<Arc<AppState>>,
    Json(req): Json<RunPayrollRequest>,
) -> Result<Json<serde_json::Value>, impl axum::response::IntoResponse> {
    match svc::run_payroll(&state.engine, req).await {
        Ok(pay_run) => Ok(Json(serde_json::to_value(pay_run).unwrap_or_default())),
        Err(e) => Err(err_response(e)),
    }
}

pub async fn approve(
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, impl axum::response::IntoResponse> {
    let req = ApprovePayRunRequest {
        pay_run_id: id,
        approved_by: AgentOrUserId::Agent("api".to_string()),
    };
    match svc::approve_pay_run(&state.engine, req).await {
        Ok(()) => Ok(Json(serde_json::json!({ "status": "approved" }))),
        Err(e) => Err(err_response(e)),
    }
}

pub async fn post_run(
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, impl axum::response::IntoResponse> {
    let actor = AgentOrUserId::Agent("api".to_string());
    match svc::post_pay_run(&state.engine, id, &actor).await {
        Ok(je_id) => Ok(Json(serde_json::json!({ "journal_entry_id": je_id }))),
        Err(e) => Err(err_response(e)),
    }
}
