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

pub async fn export(
    State(state): State<Arc<AppState>>,
    Json(req): Json<ReportRequest>,
) -> Result<Json<serde_json::Value>, impl axum::response::IntoResponse> {
    // Generate report then format for export
    match state.engine.run_report(req).await {
        Ok(data) => {
            // TODO: actual PDF/CSV generation
            Ok(Json(serde_json::json!({
                "format": "json",
                "title": data.title,
                "message": "PDF/CSV export coming soon. Report data available in JSON.",
                "data": serde_json::to_value(data.content).unwrap_or_default(),
            })))
        }
        Err(e) => Err(err_response(e)),
    }
}
