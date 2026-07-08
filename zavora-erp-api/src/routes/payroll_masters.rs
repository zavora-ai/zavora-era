//! Payroll master-data & configuration routes: earning types, deduction types,
//! departments, effective-dated statutory config, employee recurring items, and
//! loans. Back-office HR/Finance only (writes gated by `ROLES_HR_MANAGE`).

use std::sync::Arc;

use axum::{extract::{Path, Query, State}, Json};
use serde::Deserialize;
use uuid::Uuid;

use crate::AppState;
use super::err_response;
use crate::middleware::auth::{AuthContext};
use zavora_erp_core::payroll::*;
use zavora_erp_core::services::{payroll_config, payroll_masters as masters};
use zavora_erp_core::ErpError;

type ApiResult = Result<Json<serde_json::Value>, axum::response::Response>;
fn er(e: ErpError) -> axum::response::Response {
    use axum::response::IntoResponse;
    err_response(e).into_response()
}

#[derive(Deserialize)]
pub struct EmployeeQuery {
    pub employee_id: Uuid,
}

#[derive(Deserialize)]
pub struct ActivePatch {
    pub active: bool,
}

// ─── Earning types ───────────────────────────────────────────────────────────

pub async fn list_earning_types(ctx: AuthContext, State(state): State<Arc<AppState>>) -> ApiResult {
    masters::seed_default_types(&state.engine, ctx.entity_id).await.map_err(er)?;
    let rows = masters::list_earning_types(&state.engine, ctx.entity_id).await.map_err(er)?;
    Ok(Json(serde_json::to_value(rows).unwrap_or_default()))
}

pub async fn create_earning_type(ctx: AuthContext, State(state): State<Arc<AppState>>, Json(req): Json<CreateEarningTypeRequest>) -> ApiResult {
    let id = masters::create_earning_type(&state.engine, ctx.entity_id, req).await.map_err(er)?;
    Ok(Json(serde_json::json!({ "id": id })))
}

pub async fn set_earning_type_active(ctx: AuthContext, State(state): State<Arc<AppState>>, Path(id): Path<Uuid>, Json(p): Json<ActivePatch>) -> ApiResult {
    masters::set_earning_type_active(&state.engine, ctx.entity_id, id, p.active).await.map_err(er)?;
    Ok(Json(serde_json::json!({ "status": "ok" })))
}

// ─── Deduction types ─────────────────────────────────────────────────────────

pub async fn list_deduction_types(ctx: AuthContext, State(state): State<Arc<AppState>>) -> ApiResult {
    masters::seed_default_types(&state.engine, ctx.entity_id).await.map_err(er)?;
    let rows = masters::list_deduction_types(&state.engine, ctx.entity_id).await.map_err(er)?;
    Ok(Json(serde_json::to_value(rows).unwrap_or_default()))
}

pub async fn create_deduction_type(ctx: AuthContext, State(state): State<Arc<AppState>>, Json(req): Json<CreateDeductionTypeRequest>) -> ApiResult {
    let id = masters::create_deduction_type(&state.engine, ctx.entity_id, req).await.map_err(er)?;
    Ok(Json(serde_json::json!({ "id": id })))
}

pub async fn set_deduction_type_active(ctx: AuthContext, State(state): State<Arc<AppState>>, Path(id): Path<Uuid>, Json(p): Json<ActivePatch>) -> ApiResult {
    masters::set_deduction_type_active(&state.engine, ctx.entity_id, id, p.active).await.map_err(er)?;
    Ok(Json(serde_json::json!({ "status": "ok" })))
}

// ─── Departments ─────────────────────────────────────────────────────────────

pub async fn list_departments(ctx: AuthContext, State(state): State<Arc<AppState>>) -> ApiResult {
    let rows = masters::list_departments(&state.engine, ctx.entity_id).await.map_err(er)?;
    Ok(Json(serde_json::to_value(rows).unwrap_or_default()))
}

pub async fn create_department(ctx: AuthContext, State(state): State<Arc<AppState>>, Json(req): Json<CreateDepartmentRequest>) -> ApiResult {
    let id = masters::create_department(&state.engine, ctx.entity_id, req).await.map_err(er)?;
    Ok(Json(serde_json::json!({ "id": id })))
}

// ─── Statutory config (effective-dated) ──────────────────────────────────────

pub async fn list_statutory(ctx: AuthContext, State(state): State<Arc<AppState>>) -> ApiResult {
    payroll_config::ensure_seeded(&state.engine, ctx.entity_id).await.map_err(er)?;
    let rows = payroll_config::list(&state.engine, ctx.entity_id).await.map_err(er)?;
    Ok(Json(serde_json::to_value(rows).unwrap_or_default()))
}

#[derive(Deserialize)]
pub struct UpsertStatutoryRequest {
    pub effective_from: chrono::NaiveDate,
    pub config: StatutoryConfig,
}

pub async fn upsert_statutory(ctx: AuthContext, State(state): State<Arc<AppState>>, Json(req): Json<UpsertStatutoryRequest>) -> ApiResult {
    payroll_config::upsert(&state.engine, ctx.entity_id, req.effective_from, req.config, Some(ctx.user_id)).await.map_err(er)?;
    Ok(Json(serde_json::json!({ "status": "ok" })))
}

// ─── Employee recurring items ────────────────────────────────────────────────

pub async fn list_recurring(ctx: AuthContext, State(state): State<Arc<AppState>>, Query(q): Query<EmployeeQuery>) -> ApiResult {
    let rows = masters::list_recurring_items(&state.engine, ctx.entity_id, q.employee_id).await.map_err(er)?;
    Ok(Json(serde_json::to_value(rows).unwrap_or_default()))
}

pub async fn create_recurring(ctx: AuthContext, State(state): State<Arc<AppState>>, Json(req): Json<CreateRecurringItemRequest>) -> ApiResult {
    let id = masters::create_recurring_item(&state.engine, ctx.entity_id, req).await.map_err(er)?;
    Ok(Json(serde_json::json!({ "id": id })))
}

pub async fn delete_recurring(ctx: AuthContext, State(state): State<Arc<AppState>>, Path(id): Path<Uuid>) -> ApiResult {
    masters::delete_recurring_item(&state.engine, ctx.entity_id, id).await.map_err(er)?;
    Ok(Json(serde_json::json!({ "status": "deleted" })))
}

// ─── Loans ───────────────────────────────────────────────────────────────────

pub async fn list_loans(ctx: AuthContext, State(state): State<Arc<AppState>>, Query(q): Query<EmployeeQuery>) -> ApiResult {
    let rows = masters::list_loans(&state.engine, ctx.entity_id, q.employee_id).await.map_err(er)?;
    Ok(Json(serde_json::to_value(rows).unwrap_or_default()))
}

pub async fn create_loan(ctx: AuthContext, State(state): State<Arc<AppState>>, Json(req): Json<CreateLoanRequest>) -> ApiResult {
    let id = masters::create_loan(&state.engine, ctx.entity_id, req).await.map_err(er)?;
    Ok(Json(serde_json::json!({ "id": id })))
}
