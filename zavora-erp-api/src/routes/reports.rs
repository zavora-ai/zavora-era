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
) -> Result<axum::response::Response, axum::response::Response> {
    use axum::response::IntoResponse;
    req.entity_id = ctx.entity_id;
    let report_type = req.report_type.clone();
    let data = state
        .engine
        .run_report(req)
        .await
        .map_err(|e| err_response(e).into_response())?;
    let csv = zavora_erp_core::services::reporting::export_to_csv(&data)
        .map_err(|e| err_response(e).into_response())?;
    let filename = format!("{:?}-{}.csv", report_type, data.generated_at.format("%Y%m%d"));
    Ok((
        [
            (axum::http::header::CONTENT_TYPE, "text/csv; charset=utf-8".to_string()),
            (axum::http::header::CONTENT_DISPOSITION, format!("attachment; filename=\"{filename}\"")),
        ],
        csv,
    )
        .into_response())
}
