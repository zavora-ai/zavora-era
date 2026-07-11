use std::sync::Arc;

use axum::{
    extract::{Path, State},
    Json,
};
use zavora_erp_core::types::AgentOrUserId;

use crate::middleware::auth::AuthContext;
use crate::routes::err_response;
use crate::AppState;
use zavora_erp_core::services::manufacturing as svc;

// ─── Bills of Materials ──────────────────────────────────────────────────────

/// GET /boms — list the entity's bills of materials.
pub async fn list_boms(ctx: AuthContext, State(state): State<Arc<AppState>>) -> Json<serde_json::Value> {
    let rows = svc::list_boms(&state.engine, ctx.entity_id).await.unwrap_or_default();
    Json(serde_json::to_value(rows).unwrap_or_default())
}

/// GET /boms/{id} — one BOM with its component lines.
pub async fn get_bom(
    ctx: AuthContext,
    State(state): State<Arc<AppState>>,
    Path(id): Path<uuid::Uuid>,
) -> Result<Json<serde_json::Value>, impl axum::response::IntoResponse> {
    match svc::get_bom(&state.engine, ctx.entity_id, id).await {
        Ok(b) => Ok(Json(serde_json::to_value(b).unwrap_or_default())),
        Err(e) => Err(err_response(e)),
    }
}

/// POST /boms — create a bill of materials for a finished-good product.
pub async fn create_bom(
    ctx: AuthContext,
    State(state): State<Arc<AppState>>,
    Json(req): Json<svc::CreateBomRequest>,
) -> Result<Json<serde_json::Value>, impl axum::response::IntoResponse> {
    match svc::create_bom(&state.engine, ctx.entity_id, req).await {
        Ok(id) => Ok(Json(serde_json::json!({ "id": id }))),
        Err(e) => Err(err_response(e)),
    }
}

/// PUT /boms/{id} — replace a BOM's lines + settings.
pub async fn update_bom(
    ctx: AuthContext,
    State(state): State<Arc<AppState>>,
    Path(id): Path<uuid::Uuid>,
    Json(req): Json<svc::CreateBomRequest>,
) -> Result<Json<serde_json::Value>, impl axum::response::IntoResponse> {
    match svc::update_bom(&state.engine, ctx.entity_id, id, req).await {
        Ok(()) => Ok(Json(serde_json::json!({ "status": "updated" }))),
        Err(e) => Err(err_response(e)),
    }
}

// ─── Work orders ─────────────────────────────────────────────────────────────

/// GET /work-orders — list production runs.
pub async fn list_work_orders(ctx: AuthContext, State(state): State<Arc<AppState>>) -> Json<serde_json::Value> {
    let rows = svc::list_work_orders(&state.engine, ctx.entity_id).await.unwrap_or_default();
    Json(serde_json::to_value(rows).unwrap_or_default())
}

/// GET /work-orders/{id} — one work order with its consumptions.
pub async fn get_work_order(
    ctx: AuthContext,
    State(state): State<Arc<AppState>>,
    Path(id): Path<uuid::Uuid>,
) -> Result<Json<serde_json::Value>, impl axum::response::IntoResponse> {
    match svc::get_work_order(&state.engine, ctx.entity_id, id).await {
        Ok(w) => Ok(Json(serde_json::to_value(w).unwrap_or_default())),
        Err(e) => Err(err_response(e)),
    }
}

/// POST /work-orders — create a draft work order.
pub async fn create_work_order(
    ctx: AuthContext,
    State(state): State<Arc<AppState>>,
    Json(req): Json<svc::CreateWorkOrderRequest>,
) -> Result<Json<serde_json::Value>, impl axum::response::IntoResponse> {
    let actor = AgentOrUserId::User(ctx.user_id);
    match svc::create_work_order(&state.engine, ctx.entity_id, req, &actor).await {
        Ok(id) => Ok(Json(serde_json::json!({ "id": id }))),
        Err(e) => Err(err_response(e)),
    }
}

/// POST /work-orders/{id}/start — issue components into WIP.
pub async fn start_work_order(
    ctx: AuthContext,
    State(state): State<Arc<AppState>>,
    Path(id): Path<uuid::Uuid>,
) -> Result<Json<serde_json::Value>, impl axum::response::IntoResponse> {
    let actor = AgentOrUserId::User(ctx.user_id);
    match svc::start_work_order(&state.engine, ctx.entity_id, id, actor).await {
        Ok(w) => Ok(Json(serde_json::to_value(w).unwrap_or_default())),
        Err(e) => Err(err_response(e)),
    }
}

/// POST /work-orders/{id}/complete — receive finished goods out of WIP.
pub async fn complete_work_order(
    ctx: AuthContext,
    State(state): State<Arc<AppState>>,
    Path(id): Path<uuid::Uuid>,
) -> Result<Json<serde_json::Value>, impl axum::response::IntoResponse> {
    let actor = AgentOrUserId::User(ctx.user_id);
    match svc::complete_work_order(&state.engine, ctx.entity_id, id, actor).await {
        Ok(w) => Ok(Json(serde_json::to_value(w).unwrap_or_default())),
        Err(e) => Err(err_response(e)),
    }
}

/// POST /work-orders/{id}/cancel — cancel a draft work order.
pub async fn cancel_work_order(
    ctx: AuthContext,
    State(state): State<Arc<AppState>>,
    Path(id): Path<uuid::Uuid>,
) -> Result<Json<serde_json::Value>, impl axum::response::IntoResponse> {
    match svc::cancel_work_order(&state.engine, ctx.entity_id, id).await {
        Ok(()) => Ok(Json(serde_json::json!({ "status": "cancelled" }))),
        Err(e) => Err(err_response(e)),
    }
}
