use axum::{extract::{Path, State}, Json};
use std::sync::Arc;
use uuid::Uuid;

use crate::AppState;
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
    State(state): State<Arc<AppState>>,
    Json(req): Json<GeneratePeriodsRequest>,
) -> Result<Json<serde_json::Value>, impl axum::response::IntoResponse> {
    match svc::generate_periods(&state.engine, req).await {
        Ok(periods) => Ok(Json(serde_json::to_value(periods).unwrap_or_default())),
        Err(e) => Err(err_response(e)),
    }
}

pub async fn close(
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
    Json(req): Json<ClosePeriodRequest>,
) -> Result<Json<serde_json::Value>, impl axum::response::IntoResponse> {
    let mut close_req = req;
    close_req.period_id = id;
    match svc::close_period(&state.engine, close_req).await {
        Ok(period) => Ok(Json(serde_json::to_value(period).unwrap_or_default())),
        Err(e) => Err(err_response(e)),
    }
}
