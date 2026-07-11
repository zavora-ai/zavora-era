use std::sync::Arc;

use axum::{
    extract::{Path, State},
    Json,
};

use crate::middleware::auth::AuthContext;
use crate::routes::err_response;
use crate::AppState;
use zavora_erp_core::services::projects as svc;

/// GET /projects — list the entity's projects (with budget lines + tasks).
pub async fn list(ctx: AuthContext, State(state): State<Arc<AppState>>) -> Json<serde_json::Value> {
    let rows = svc::list_projects(&state.engine, ctx.entity_id).await.unwrap_or_default();
    Json(serde_json::to_value(rows).unwrap_or_default())
}

/// GET /projects/{id} — one project.
pub async fn get_one(
    ctx: AuthContext,
    State(state): State<Arc<AppState>>,
    Path(id): Path<uuid::Uuid>,
) -> Result<Json<serde_json::Value>, impl axum::response::IntoResponse> {
    match svc::get_project(&state.engine, ctx.entity_id, id).await {
        Ok(p) => Ok(Json(serde_json::to_value(p).unwrap_or_default())),
        Err(e) => Err(err_response(e)),
    }
}

/// POST /projects — create a project (auto-creates its PROJECT dimension value).
pub async fn create(
    ctx: AuthContext,
    State(state): State<Arc<AppState>>,
    Json(req): Json<svc::CreateProjectRequest>,
) -> Result<Json<serde_json::Value>, impl axum::response::IntoResponse> {
    match svc::create_project(&state.engine, ctx.entity_id, req).await {
        Ok(id) => Ok(Json(serde_json::json!({ "id": id }))),
        Err(e) => Err(err_response(e)),
    }
}

/// PUT /projects/{id} — update a project + replace budget lines/tasks.
pub async fn update(
    ctx: AuthContext,
    State(state): State<Arc<AppState>>,
    Path(id): Path<uuid::Uuid>,
    Json(req): Json<svc::CreateProjectRequest>,
) -> Result<Json<serde_json::Value>, impl axum::response::IntoResponse> {
    match svc::update_project(&state.engine, ctx.entity_id, id, req).await {
        Ok(()) => Ok(Json(serde_json::json!({ "status": "updated" }))),
        Err(e) => Err(err_response(e)),
    }
}

/// GET /projects/{id}/summary — budget-vs-actual + profitability from the GL.
pub async fn summary(
    ctx: AuthContext,
    State(state): State<Arc<AppState>>,
    Path(id): Path<uuid::Uuid>,
) -> Result<Json<serde_json::Value>, impl axum::response::IntoResponse> {
    match svc::project_summary(&state.engine, ctx.entity_id, id).await {
        Ok(s) => Ok(Json(serde_json::to_value(s).unwrap_or_default())),
        Err(e) => Err(err_response(e)),
    }
}
