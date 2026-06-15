use axum::{extract::{Path, State}, Json};
use std::sync::Arc;
use uuid::Uuid;

use crate::AppState;
use crate::middleware::auth::{AuthContext, require_role, ROLES_CLOSE_PERIOD, ROLES_MANAGE};
use super::err_response;
use zavora_erp_core::period::*;
use zavora_erp_core::services::periods as svc;

pub async fn list(
    State(state): State<Arc<AppState>>,
) -> Result<Json<serde_json::Value>, impl axum::response::IntoResponse> {
    match svc::list_periods(&state.engine).await {
        Ok(periods) => Ok(Json(serde_json::to_value(periods).unwrap_or_default())),
        Err(e) => Err(err_response(e)),
    }
}

pub async fn generate(
    ctx: AuthContext,
    State(state): State<Arc<AppState>>,
    Json(req): Json<GeneratePeriodsRequest>,
) -> Result<Json<serde_json::Value>, impl axum::response::IntoResponse> {
    require_role(ROLES_MANAGE, &ctx, "generate fiscal periods").map_err(err_response)?;
    match svc::generate_periods(&state.engine, req).await {
        Ok(periods) => Ok(Json(serde_json::to_value(periods).unwrap_or_default())),
        Err(e) => Err(err_response(e)),
    }
}

pub async fn close(
    ctx: AuthContext,
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
    Json(req): Json<ClosePeriodRequest>,
) -> Result<Json<serde_json::Value>, impl axum::response::IntoResponse> {
    require_role(ROLES_CLOSE_PERIOD, &ctx, "close fiscal period").map_err(err_response)?;
    let mut close_req = req;
    close_req.period_id = id;
    match svc::close_period(&state.engine, close_req).await {
        Ok(period) => Ok(Json(serde_json::to_value(period).unwrap_or_default())),
        Err(e) => Err(err_response(e)),
    }
}

pub async fn reopen(
    ctx: AuthContext,
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
    Json(req): Json<ReopenPeriodRequest>,
) -> Result<Json<serde_json::Value>, impl axum::response::IntoResponse> {
    require_role(ROLES_CLOSE_PERIOD, &ctx, "reopen fiscal period").map_err(err_response)?;
    let mut reopen_req = req;
    reopen_req.period_id = id;
    match svc::reopen_period(&state.engine, reopen_req).await {
        Ok(period) => Ok(Json(serde_json::to_value(period).unwrap_or_default())),
        Err(e) => Err(err_response(e)),
    }
}
