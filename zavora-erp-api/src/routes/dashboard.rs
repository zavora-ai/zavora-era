use axum::{extract::State, Json};
use std::sync::Arc;

use crate::AppState;
use super::err_response;

pub async fn summary(
    State(state): State<Arc<AppState>>,
) -> Result<Json<serde_json::Value>, impl axum::response::IntoResponse> {
    match state.engine.dashboard_summary(state.engine.entity_id()).await {
        Ok(summary) => Ok(Json(serde_json::to_value(summary).unwrap_or_default())),
        Err(e) => Err(err_response(e)),
    }
}
