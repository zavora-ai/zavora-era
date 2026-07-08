use axum::{extract::{Path, State}, Json};
use std::sync::Arc;
use uuid::Uuid;

use crate::AppState;
use crate::middleware::auth::{AuthContext, require_permission};
use super::err_response;
use zavora_erp_core::payroll::*;
use zavora_erp_core::services::payroll as svc;
use zavora_erp_core::services::payroll_masters as masters;
use zavora_erp_core::AgentOrUserId;

pub async fn run(
    ctx: AuthContext,
    State(state): State<Arc<AppState>>,
    Json(req): Json<RunPayrollRequest>,
) -> Result<Json<serde_json::Value>, impl axum::response::IntoResponse> {
    match svc::run_payroll(&state.engine, ctx.entity_id, req).await {
        Ok(pay_run) => Ok(Json(serde_json::to_value(pay_run).unwrap_or_default())),
        Err(e) => Err(err_response(e)),
    }
}

pub async fn approve(
    ctx: AuthContext,
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, impl axum::response::IntoResponse> {
    let req = ApprovePayRunRequest {
        pay_run_id: id,
        approved_by: AgentOrUserId::User(ctx.user_id),
    };
    match svc::approve_pay_run(&state.engine, ctx.entity_id, req).await {
        Ok(()) => Ok(Json(serde_json::json!({ "status": "approved" }))),
        Err(e) => Err(err_response(e)),
    }
}

pub async fn post_run(
    ctx: AuthContext,
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, impl axum::response::IntoResponse> {
    let actor = AgentOrUserId::User(ctx.user_id);
    match svc::post_pay_run(&state.engine, ctx.entity_id, id, &actor).await {
        Ok(je_id) => Ok(Json(serde_json::json!({ "journal_entry_id": je_id }))),
        Err(e) => Err(err_response(e)),
    }
}

pub async fn mark_paid(
    ctx: AuthContext,
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, impl axum::response::IntoResponse> {
    let actor = AgentOrUserId::User(ctx.user_id);
    match svc::mark_pay_run_paid(&state.engine, ctx.entity_id, id, &actor).await {
        Ok(()) => Ok(Json(serde_json::json!({ "status": "paid" }))),
        Err(e) => Err(err_response(e)),
    }
}

/// GET /payroll/{run_id}/payslips/{employee_id}/pdf — back-office payslip PDF.
pub async fn payslip_pdf(
    ctx: AuthContext,
    State(state): State<Arc<AppState>>,
    Path((run_id, employee_id)): Path<(Uuid, Uuid)>,
) -> Result<axum::response::Response, axum::response::Response> {
    use axum::response::IntoResponse;
    require_permission(&state, &ctx, "pay_run.read").await.map_err(|e| err_response(e).into_response())?;
    match svc::payslip_pdf(&state.engine, ctx.entity_id, run_id, employee_id).await {
        Ok(bytes) => Ok(pdf_response(bytes, "payslip.pdf")),
        Err(e) => Err(err_response(e).into_response()),
    }
}

fn pdf_response(bytes: Vec<u8>, filename: &str) -> axum::response::Response {
    use axum::response::IntoResponse;
    (
        [
            (axum::http::header::CONTENT_TYPE, "application/pdf".to_string()),
            (axum::http::header::CONTENT_DISPOSITION, format!("inline; filename=\"{filename}\"")),
        ],
        bytes,
    ).into_response()
}

/// GET /payroll — pay run history.
pub async fn list(
    ctx: AuthContext,
    State(state): State<Arc<AppState>>,
) -> Result<Json<serde_json::Value>, impl axum::response::IntoResponse> {
    require_permission(&state, &ctx, "pay_run.read").await.map_err(err_response)?;
    match svc::list_pay_runs(&state.engine, ctx.entity_id).await {
        Ok(rows) => Ok(Json(serde_json::to_value(rows).unwrap_or_default())),
        Err(e) => Err(err_response(e)),
    }
}

/// GET /payroll/{id} — run detail (header + payslips).
pub async fn detail(
    ctx: AuthContext,
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, impl axum::response::IntoResponse> {
    require_permission(&state, &ctx, "pay_run.read").await.map_err(err_response)?;
    match svc::load_pay_run(&state.engine, ctx.entity_id, id).await {
        Ok(run) => Ok(Json(serde_json::to_value(run).unwrap_or_default())),
        Err(e) => Err(err_response(e)),
    }
}

/// POST /payroll/{id}/recompute — recompute a draft (picks up inputs).
pub async fn recompute(
    ctx: AuthContext,
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, impl axum::response::IntoResponse> {
    match svc::recompute_pay_run(&state.engine, ctx.entity_id, id).await {
        Ok(run) => Ok(Json(serde_json::to_value(run).unwrap_or_default())),
        Err(e) => Err(err_response(e)),
    }
}

/// DELETE /payroll/{id} — delete a draft run.
pub async fn delete_draft(
    ctx: AuthContext,
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, impl axum::response::IntoResponse> {
    match svc::delete_draft_pay_run(&state.engine, ctx.entity_id, id).await {
        Ok(()) => Ok(Json(serde_json::json!({ "status": "deleted" }))),
        Err(e) => Err(err_response(e)),
    }
}

/// GET /payroll/{id}/inputs — per-run variable inputs.
pub async fn list_inputs(
    ctx: AuthContext,
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, impl axum::response::IntoResponse> {
    require_permission(&state, &ctx, "pay_run.read").await.map_err(err_response)?;
    match masters::list_run_inputs(&state.engine, ctx.entity_id, id).await {
        Ok(rows) => Ok(Json(serde_json::to_value(rows).unwrap_or_default())),
        Err(e) => Err(err_response(e)),
    }
}

/// POST /payroll/{id}/inputs — add a per-run earning/deduction input.
pub async fn add_input(
    ctx: AuthContext,
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
    Json(req): Json<CreatePayRunInputRequest>,
) -> Result<Json<serde_json::Value>, impl axum::response::IntoResponse> {
    match masters::add_run_input(&state.engine, ctx.entity_id, id, req).await {
        Ok(input_id) => Ok(Json(serde_json::json!({ "id": input_id }))),
        Err(e) => Err(err_response(e)),
    }
}

/// DELETE /payroll/{id}/inputs/{input_id} — remove a per-run input.
pub async fn delete_input(
    ctx: AuthContext,
    State(state): State<Arc<AppState>>,
    Path((_id, input_id)): Path<(Uuid, Uuid)>,
) -> Result<Json<serde_json::Value>, impl axum::response::IntoResponse> {
    match masters::delete_run_input(&state.engine, ctx.entity_id, input_id).await {
        Ok(()) => Ok(Json(serde_json::json!({ "status": "deleted" }))),
        Err(e) => Err(err_response(e)),
    }
}
