use axum::{extract::{Path, State}, Json};
use std::sync::Arc;
use uuid::Uuid;

use crate::AppState;
use super::err_response;
use zavora_erp_core::catalog::*;
use zavora_erp_core::services::catalog as svc;
use zavora_erp_core::AgentOrUserId;

pub async fn list_products(
    State(state): State<Arc<AppState>>,
) -> Result<Json<serde_json::Value>, impl axum::response::IntoResponse> {
    let rows = sqlx::query_as::<_, ProductRow>(
        "SELECT * FROM products WHERE entity_id = $1 AND is_active = true ORDER BY name",
    )
    .bind(state.engine.entity_id())
    .fetch_all(state.engine.pool())
    .await;
    match rows {
        Ok(r) => Ok(Json(serde_json::to_value(r).unwrap_or_default())),
        Err(e) => Err(err_response(zavora_erp_core::ErpError::Database(e))),
    }
}

pub async fn get_product(
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, impl axum::response::IntoResponse> {
    let row = sqlx::query_as::<_, ProductRow>(
        "SELECT * FROM products WHERE id = $1 AND entity_id = $2",
    )
    .bind(id).bind(state.engine.entity_id())
    .fetch_optional(state.engine.pool()).await;
    match row {
        Ok(Some(r)) => Ok(Json(serde_json::to_value(r).unwrap_or_default())),
        Ok(None) => Err(err_response(zavora_erp_core::ErpError::NotFound { entity_type: "Product".into(), id })),
        Err(e) => Err(err_response(zavora_erp_core::ErpError::Database(e))),
    }
}

pub async fn create_product(
    State(state): State<Arc<AppState>>,
    Json(req): Json<CreateProductRequest>,
) -> Result<Json<serde_json::Value>, impl axum::response::IntoResponse> {
    let actor = AgentOrUserId::Agent("api".to_string());
    match svc::create_product(&state.engine, req, &actor).await {
        Ok(id) => Ok(Json(serde_json::json!({ "id": id }))),
        Err(e) => Err(err_response(e)),
    }
}

pub async fn update_product(
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
    Json(patch): Json<serde_json::Value>,
) -> Json<serde_json::Value> {
    if let Some(name) = patch.get("name").and_then(|v| v.as_str()) {
        sqlx::query("UPDATE products SET name = $1 WHERE id = $2 AND entity_id = $3")
            .bind(name).bind(id).bind(state.engine.entity_id())
            .execute(state.engine.pool()).await.ok();
    }
    Json(serde_json::json!({ "id": id, "updated": true }))
}
