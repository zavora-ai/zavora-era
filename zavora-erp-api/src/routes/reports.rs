use axum::{extract::State, Json};
use std::sync::Arc;

use crate::AppState;
use super::err_response;
use zavora_erp_core::reporting::*;

pub async fn generate(
    State(state): State<Arc<AppState>>,
    Json(req): Json<ReportRequest>,
) -> Result<Json<serde_json::Value>, impl axum::response::IntoResponse> {
    match state.engine.run_report(req).await {
        Ok(data) => Ok(Json(serde_json::to_value(data).unwrap_or_default())),
        Err(e) => Err(err_response(e)),
    }
}
