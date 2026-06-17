use axum::{extract::State, Json};
use std::sync::Arc;

use crate::AppState;
use crate::middleware::auth::AuthContext;
use super::err_response;
use zavora_erp_core::reporting::*;

pub async fn generate(
    ctx: AuthContext,
    State(state): State<Arc<AppState>>,
    Json(mut req): Json<ReportRequest>,
) -> Result<Json<serde_json::Value>, impl axum::response::IntoResponse> {
    // Force the report to the caller's tenant — never trust a client-supplied
    // entity_id (prevents cross-tenant report disclosure).
    req.entity_id = ctx.entity_id;
    match state.engine.run_report(req).await {
        Ok(data) => Ok(Json(serde_json::to_value(data).unwrap_or_default())),
        Err(e) => Err(err_response(e)),
    }
}

pub async fn export(
    ctx: AuthContext,
    State(state): State<Arc<AppState>>,
    Json(mut req): Json<ReportRequest>,
) -> Result<Json<serde_json::Value>, impl axum::response::IntoResponse> {
    req.entity_id = ctx.entity_id;
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
