use axum::{extract::{Path, State}, Json};
use std::sync::Arc;
use uuid::Uuid;

use crate::AppState;
use crate::middleware::auth::{AuthContext};
use super::err_response;
use zavora_erp_core::period::*;
use zavora_erp_core::services::periods as svc;
use zavora_erp_core::services::period_close;

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
    // Actor is taken from the verified JWT, never the request body (audit integrity).
    let close_type = match body.get("close_type").and_then(|v| v.as_str()) {
        Some(s) if s.eq_ignore_ascii_case("hard") => PeriodCloseType::Hard,
        _ => PeriodCloseType::Soft,
    };
    let close_req = ClosePeriodRequest {
        period_id: id,
        close_type,
        closed_by: zavora_erp_core::AgentOrUserId::User(ctx.user_id),
        // Hard-close checklist override — explicit opt-in, recorded in audit.
        force: body.get("force").and_then(|v| v.as_bool()).unwrap_or(false),
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

/// POST /periods/year-end-close — execute the year-end closing procedure for a
/// fiscal year. Requires every period of the year to be hard-closed first. Posts
/// the closing entry into the year's last period and the opening-balance entry
/// into the next year's first period, atomically. Idempotent: refuses if the year
/// has already been closed.
///
/// Body: `{ "fiscal_year": 2025 }`. The actor is taken from the verified JWT.
pub async fn year_end_close(
    ctx: AuthContext,
    State(state): State<Arc<AppState>>,
    Json(body): Json<serde_json::Value>,
) -> Result<Json<serde_json::Value>, impl axum::response::IntoResponse> {
    let Some(fiscal_year) = body.get("fiscal_year").and_then(|v| v.as_i64()) else {
        return Err(err_response(zavora_erp_core::ErpError::ValidationFailed {
            message: "Missing or invalid 'fiscal_year' (expected an integer year)".to_string(),
        }));
    };
    let req = period_close::YearEndCloseRequest {
        fiscal_year: fiscal_year as i32,
        executed_by: zavora_erp_core::AgentOrUserId::User(ctx.user_id),
    };
    match period_close::execute_year_end_close(&state.engine, ctx.entity_id, req).await {
        Ok(result) => Ok(Json(serde_json::to_value(result).unwrap_or_default())),
        Err(e) => Err(err_response(e)),
    }
}
