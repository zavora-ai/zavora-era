use std::sync::Arc;

use axum::{
    extract::{Path, State},
    Json,
};

use crate::middleware::auth::AuthContext;
use crate::routes::err_response;
use crate::AppState;
use zavora_erp_core::services::warehousing as svc;

/// GET /warehouses — list the entity's warehouses (own + 3PL).
pub async fn list(ctx: AuthContext, State(state): State<Arc<AppState>>) -> Json<serde_json::Value> {
    let rows = svc::list_warehouses(&state.engine, ctx.entity_id).await.unwrap_or_default();
    Json(serde_json::to_value(rows).unwrap_or_default())
}

/// POST /warehouses — create a warehouse (own or 3PL).
pub async fn create(
    ctx: AuthContext,
    State(state): State<Arc<AppState>>,
    Json(req): Json<svc::CreateWarehouseRequest>,
) -> Result<Json<serde_json::Value>, impl axum::response::IntoResponse> {
    match svc::create_warehouse(&state.engine, ctx.entity_id, req).await {
        Ok(w) => Ok(Json(serde_json::to_value(w).unwrap_or_default())),
        Err(e) => Err(err_response(e)),
    }
}

/// PUT /warehouses/{id} — update name/provider/location/active.
pub async fn update(
    ctx: AuthContext,
    State(state): State<Arc<AppState>>,
    Path(id): Path<uuid::Uuid>,
    Json(req): Json<svc::UpdateWarehouseRequest>,
) -> Result<Json<serde_json::Value>, impl axum::response::IntoResponse> {
    match svc::update_warehouse(&state.engine, ctx.entity_id, id, req).await {
        Ok(()) => Ok(Json(serde_json::json!({ "status": "updated" }))),
        Err(e) => Err(err_response(e)),
    }
}

/// POST /warehouses/transfer — move stock between two warehouses.
pub async fn transfer(
    ctx: AuthContext,
    State(state): State<Arc<AppState>>,
    Json(req): Json<svc::TransferRequest>,
) -> Result<Json<serde_json::Value>, impl axum::response::IntoResponse> {
    match svc::transfer_stock(&state.engine, ctx.entity_id, req, ctx.user_id).await {
        Ok(()) => Ok(Json(serde_json::json!({ "status": "transferred" }))),
        Err(e) => Err(err_response(e)),
    }
}

/// GET /warehouses/{id}/stock — items held in a warehouse.
pub async fn stock_in_warehouse(
    ctx: AuthContext,
    State(state): State<Arc<AppState>>,
    Path(id): Path<uuid::Uuid>,
) -> Json<serde_json::Value> {
    let rows = svc::stock_in_warehouse(&state.engine, ctx.entity_id, id).await.unwrap_or_default();
    Json(serde_json::to_value(rows).unwrap_or_default())
}

/// GET /inventory/{item_id}/warehouse-stock — where an item's stock sits.
pub async fn item_stock(
    ctx: AuthContext,
    State(state): State<Arc<AppState>>,
    Path(item_id): Path<uuid::Uuid>,
) -> Json<serde_json::Value> {
    let rows = svc::stock_for_item(&state.engine, ctx.entity_id, item_id).await.unwrap_or_default();
    Json(serde_json::to_value(rows).unwrap_or_default())
}
