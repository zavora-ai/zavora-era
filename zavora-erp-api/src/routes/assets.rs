use axum::{extract::State, Json};
use std::sync::Arc;

use crate::AppState;
use super::err_response;
use zavora_erp_core::assets::*;
use zavora_erp_core::services::assets as svc;
use zavora_erp_core::AgentOrUserId;

pub async fn list(
    State(state): State<Arc<AppState>>,
) -> Result<Json<serde_json::Value>, impl axum::response::IntoResponse> {
    let rows = sqlx::query_as::<_, FixedAssetRow>(
        "SELECT * FROM fixed_assets WHERE entity_id = $1 ORDER BY asset_number",
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
    State(state): State<Arc<AppState>>,
    Json(req): Json<CreateAssetRequest>,
) -> Result<Json<serde_json::Value>, impl axum::response::IntoResponse> {
    let actor = AgentOrUserId::Agent("api".to_string());
    match svc::create_asset(&state.engine, req, &actor).await {
        Ok(id) => Ok(Json(serde_json::json!({ "id": id }))),
        Err(e) => Err(err_response(e)),
    }
}

pub async fn run_depreciation(
    State(_state): State<Arc<AppState>>,
) -> Json<serde_json::Value> {
    // TODO: implement depreciation run across all active assets
    Json(serde_json::json!({ "status": "todo", "message": "Depreciation run not yet implemented" }))
}
