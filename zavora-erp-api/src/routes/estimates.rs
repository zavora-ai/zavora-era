use axum::{extract::{Path, State}, Json};
use std::sync::Arc;
use uuid::Uuid;

use crate::AppState;
use super::err_response;

pub async fn list(
    State(state): State<Arc<AppState>>,
) -> Result<Json<serde_json::Value>, impl axum::response::IntoResponse> {
    let rows = sqlx::query_as::<_, zavora_erp_core::invoicing::EstimateRow>(
        "SELECT * FROM estimates WHERE entity_id = $1 ORDER BY created_at DESC",
    )
    .bind(state.engine.entity_id())
    .fetch_all(state.engine.pool())
    .await;
    match rows {
        Ok(r) => Ok(Json(serde_json::to_value(r).unwrap_or_default())),
        Err(e) => Err(err_response(zavora_erp_core::ErpError::Database(e))),
    }
}

pub async fn get_one(
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, impl axum::response::IntoResponse> {
    let row = sqlx::query_as::<_, zavora_erp_core::invoicing::EstimateRow>(
        "SELECT * FROM estimates WHERE id = $1 AND entity_id = $2",
    )
    .bind(id)
    .bind(state.engine.entity_id())
    .fetch_optional(state.engine.pool())
    .await;
    match row {
        Ok(Some(r)) => Ok(Json(serde_json::to_value(r).unwrap_or_default())),
        Ok(None) => Err(err_response(zavora_erp_core::ErpError::NotFound { entity_type: "Estimate".into(), id })),
        Err(e) => Err(err_response(zavora_erp_core::ErpError::Database(e))),
    }
}

pub async fn create(
    State(_state): State<Arc<AppState>>,
    Json(req): Json<zavora_erp_core::invoicing::CreateEstimateRequest>,
) -> Json<serde_json::Value> {
    // TODO: wire to service
    Json(serde_json::json!({ "status": "created", "customer_id": req.customer_id }))
}

pub async fn convert(
    State(_state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
) -> Json<serde_json::Value> {
    // TODO: wire to estimate->invoice conversion service
    Json(serde_json::json!({ "status": "converted", "estimate_id": id }))
}
