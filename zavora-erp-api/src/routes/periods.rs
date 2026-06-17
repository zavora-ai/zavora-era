use axum::{extract::{Path, State}, Json};
use std::sync::Arc;
use uuid::Uuid;

use crate::AppState;
use crate::middleware::auth::{AuthContext, require_role, ROLES_CLOSE_PERIOD, ROLES_MANAGE};
use super::err_response;
use zavora_erp_core::period::*;
use zavora_erp_core::services::periods as svc;

pub async fn list(
    ctx: AuthContext,
    State(state): State<Arc<AppState>>,
) -> Result<Json<serde_json::Value>, impl axum::response::IntoResponse> {
    match svc::list_periods(&state.engine, ctx.entity_id).await {
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
    match svc::generate_periods(&state.engine, ctx.entity_id, req).await {
        Ok(periods) => Ok(Json(serde_json::to_value(periods).unwrap_or_default())),
        Err(e) => Err(err_response(e)),
    }
}

pub async fn close(
    ctx: AuthContext,
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
    Json(body): Json<serde_json::Value>,
) -> Result<Json<serde_json::Value>, impl axum::response::IntoResponse> {
    require_role(ROLES_CLOSE_PERIOD, &ctx, "close fiscal period").map_err(err_response)?;
    // Actor is taken from the verified JWT, never the request body (audit integrity).
    let close_type = match body.get("close_type").and_then(|v| v.as_str()) {
        Some(s) if s.eq_ignore_ascii_case("hard") => PeriodCloseType::Hard,
        _ => PeriodCloseType::Soft,
    };
    let close_req = ClosePeriodRequest {
        period_id: id,
        close_type,
        closed_by: zavora_erp_core::AgentOrUserId::User(ctx.user_id),
    };
    match svc::close_period(&state.engine, ctx.entity_id, close_req).await {
        Ok(period) => Ok(Json(serde_json::to_value(period).unwrap_or_default())),
        Err(e) => Err(err_response(e)),
    }
}

pub async fn reopen(
    ctx: AuthContext,
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
    Json(body): Json<serde_json::Value>,
) -> Result<Json<serde_json::Value>, impl axum::response::IntoResponse> {
    require_role(ROLES_CLOSE_PERIOD, &ctx, "reopen fiscal period").map_err(err_response)?;
    let reason = body
        .get("reason")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let reopen_req = ReopenPeriodRequest {
        period_id: id,
        reopened_by: zavora_erp_core::AgentOrUserId::User(ctx.user_id),
        reason,
    };
    match svc::reopen_period(&state.engine, ctx.entity_id, reopen_req).await {
        Ok(period) => Ok(Json(serde_json::to_value(period).unwrap_or_default())),
        Err(e) => Err(err_response(e)),
    }
}
