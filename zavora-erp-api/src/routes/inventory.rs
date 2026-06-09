use axum::{extract::State, Json};
use std::sync::Arc;

use crate::AppState;
use super::err_response;
use zavora_erp_core::inventory::*;
use zavora_erp_core::services::inventory as svc;
use zavora_erp_core::AgentOrUserId;

pub async fn list(
    State(state): State<Arc<AppState>>,
) -> Result<Json<serde_json::Value>, impl axum::response::IntoResponse> {
    let rows = sqlx::query_as::<_, InventoryItemRow>(
        "SELECT * FROM inventory_items WHERE entity_id = $1 AND is_active = true ORDER BY sku",
    )
    .bind(state.engine.entity_id())
    .fetch_all(state.engine.pool())
    .await;
    match rows {
        Ok(r) => Ok(Json(serde_json::to_value(r).unwrap_or_default())),
        Err(e) => Err(err_response(zavora_erp_core::ErpError::Database(e))),
    }
}

pub async fn create(
    State(_state): State<Arc<AppState>>,
    Json(_req): Json<serde_json::Value>,
) -> Json<serde_json::Value> {
    Json(serde_json::json!({ "status": "todo" }))
}

pub async fn receive(
    State(state): State<Arc<AppState>>,
    Json(req): Json<ReceiveInventoryRequest>,
) -> Result<Json<serde_json::Value>, impl axum::response::IntoResponse> {
    let actor = AgentOrUserId::Agent("api".to_string());
    match svc::receive_inventory(&state.engine, req, &actor).await {
        Ok(id) => Ok(Json(serde_json::json!({ "movement_id": id }))),
        Err(e) => Err(err_response(e)),
    }
}

pub async fn issue(
    State(state): State<Arc<AppState>>,
    Json(req): Json<IssueInventoryRequest>,
) -> Result<Json<serde_json::Value>, impl axum::response::IntoResponse> {
    let actor = AgentOrUserId::Agent("api".to_string());
    match svc::issue_inventory(&state.engine, req, &actor).await {
        Ok(id) => Ok(Json(serde_json::json!({ "movement_id": id }))),
        Err(e) => Err(err_response(e)),
    }
}
