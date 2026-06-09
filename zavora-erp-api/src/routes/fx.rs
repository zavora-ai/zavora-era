use axum::{extract::State, Json};
use std::sync::Arc;

use crate::AppState;
use super::err_response;
use zavora_erp_core::fx::*;
use zavora_erp_core::services::fx as svc;

pub async fn list(
    State(state): State<Arc<AppState>>,
) -> Result<Json<serde_json::Value>, impl axum::response::IntoResponse> {
    let rows = sqlx::query_as::<_, ExchangeRateRow>(
        "SELECT * FROM exchange_rates WHERE entity_id = $1 ORDER BY rate_date DESC LIMIT 100",
    )
    .bind(state.engine.entity_id())
    .fetch_all(state.engine.pool())
    .await;
    match rows {
        Ok(r) => Ok(Json(serde_json::to_value(r).unwrap_or_default())),
        Err(e) => Err(err_response(zavora_erp_core::ErpError::Database(e))),
    }
}

pub async fn upsert(
    State(state): State<Arc<AppState>>,
    Json(req): Json<UpsertRateRequest>,
) -> Result<Json<serde_json::Value>, impl axum::response::IntoResponse> {
    match svc::upsert_rate(&state.engine, req).await {
        Ok(rate) => Ok(Json(serde_json::to_value(rate).unwrap_or_default())),
        Err(e) => Err(err_response(e)),
    }
}

pub async fn revaluation(
    State(_state): State<Arc<AppState>>,
) -> Json<serde_json::Value> {
    // TODO: implement FX revaluation
    Json(serde_json::json!({ "status": "todo", "message": "FX revaluation not yet implemented" }))
}
