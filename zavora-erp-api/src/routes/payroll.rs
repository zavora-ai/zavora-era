use axum::{extract::{Path, State}, Json};
use std::sync::Arc;
use uuid::Uuid;

use crate::AppState;
use crate::middleware::auth::{AuthContext, require_role, ROLES_CREATE, ROLES_APPROVE, ROLES_POST_JOURNAL};
use super::err_response;
use zavora_erp_core::payroll::*;
use zavora_erp_core::services::payroll as svc;
use zavora_erp_core::AgentOrUserId;

pub async fn run(
    ctx: AuthContext,
    State(state): State<Arc<AppState>>,
    Json(req): Json<RunPayrollRequest>,
) -> Result<Json<serde_json::Value>, impl axum::response::IntoResponse> {
    require_role(ROLES_CREATE, &ctx, "run payroll").map_err(err_response)?;
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
    require_role(ROLES_APPROVE, &ctx, "approve pay run").map_err(err_response)?;
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
    require_role(ROLES_POST_JOURNAL, &ctx, "post pay run").map_err(err_response)?;
    let actor = AgentOrUserId::User(ctx.user_id);
    match svc::post_pay_run(&state.engine, id, ctx.entity_id, &actor).await {
        Ok(je_id) => Ok(Json(serde_json::json!({ "journal_entry_id": je_id }))),
        Err(e) => Err(err_response(e)),
    }
}

pub async fn mark_paid(
    ctx: AuthContext,
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, impl axum::response::IntoResponse> {
    require_role(ROLES_POST_JOURNAL, &ctx, "mark pay run paid").map_err(err_response)?;
    let actor = AgentOrUserId::User(ctx.user_id);
    match svc::mark_pay_run_paid(&state.engine, id, ctx.entity_id, &actor).await {
        Ok(()) => Ok(Json(serde_json::json!({ "status": "paid" }))),
        Err(e) => Err(err_response(e)),
    }
}
