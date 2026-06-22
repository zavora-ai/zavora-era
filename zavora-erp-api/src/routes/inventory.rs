use axum::{extract::State, Json};
use std::sync::Arc;

use crate::AppState;
use super::err_response;
use crate::middleware::auth::{require_role, AuthContext, ROLES_CREATE, ROLES_POST_JOURNAL};
use zavora_erp_core::inventory::*;
use zavora_erp_core::services::inventory as svc;
use zavora_erp_core::AgentOrUserId;

pub async fn list(
    ctx: AuthContext,
    State(state): State<Arc<AppState>>,
) -> Result<Json<serde_json::Value>, impl axum::response::IntoResponse> {
    let rows = sqlx::query_as::<_, InventoryItemRow>(
        "SELECT * FROM inventory_items WHERE entity_id = $1 AND is_active = true ORDER BY sku",
    )
    .bind(ctx.entity_id)
    .fetch_all(state.engine.pool())
    .await;
    match rows {
        Ok(r) => Ok(Json(serde_json::to_value(r).unwrap_or_default())),
        Err(e) => Err(err_response(zavora_erp_core::ErpError::Database(e))),
    }
}

pub async fn create(
    ctx: AuthContext,
    State(_state): State<Arc<AppState>>,
    Json(_req): Json<serde_json::Value>,
) -> Result<Json<serde_json::Value>, impl axum::response::IntoResponse> {
    if let Err(e) = require_role(ROLES_CREATE, &ctx, "create inventory item") { return Err(err_response(e)); }
    Ok(Json(serde_json::json!({ "status": "todo" })))
}

pub async fn receive(
    ctx: AuthContext,
    State(state): State<Arc<AppState>>,
    Json(req): Json<ReceiveInventoryRequest>,
) -> Result<Json<serde_json::Value>, impl axum::response::IntoResponse> {
    require_role(ROLES_CREATE, &ctx, "receive inventory").map_err(err_response)?;
    let actor = AgentOrUserId::User(ctx.user_id);
    match svc::receive_inventory(&state.engine, ctx.entity_id, req, &actor).await {
        Ok(id) => Ok(Json(serde_json::json!({ "movement_id": id }))),
        Err(e) => Err(err_response(e)),
    }
}

pub async fn issue(
    ctx: AuthContext,
    State(state): State<Arc<AppState>>,
    Json(req): Json<IssueInventoryRequest>,
) -> Result<Json<serde_json::Value>, impl axum::response::IntoResponse> {
    require_role(ROLES_CREATE, &ctx, "issue inventory").map_err(err_response)?;
    let actor = AgentOrUserId::User(ctx.user_id);
    match svc::issue_inventory(&state.engine, ctx.entity_id, req, &actor).await {
        Ok(result) => Ok(Json(serde_json::json!({ "movement_id": result.movement_id }))),
        Err(e) => Err(err_response(e)),
    }
}

/// POST /inventory/adjust — stock-take adjustment to a counted quantity.
pub async fn adjust(
    ctx: AuthContext,
    State(state): State<Arc<AppState>>,
    Json(req): Json<svc::AdjustInventoryRequest>,
) -> Result<Json<serde_json::Value>, impl axum::response::IntoResponse> {
    require_role(ROLES_POST_JOURNAL, &ctx, "adjust inventory").map_err(err_response)?;
    let actor = AgentOrUserId::User(ctx.user_id);
    match svc::adjust_inventory(&state.engine, ctx.entity_id, req, actor).await {
        Ok(item_id) => Ok(Json(serde_json::json!({ "item_id": item_id }))),
        Err(e) => Err(err_response(e)),
    }
}
