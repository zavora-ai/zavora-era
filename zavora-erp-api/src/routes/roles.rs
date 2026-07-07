//! Read-only role listing for admin UIs (Phase 2). Returns the assignable roles
//! available to a tenant — the built-in system roles plus any per-tenant custom
//! roles — so the Users invite/edit dropdowns include `HrManager` and custom
//! roles instead of a hard-coded list. Full role administration (create/edit
//! permissions) lands in Phase 3.

use std::sync::Arc;

use axum::extract::{Path, State};
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde::Deserialize;
use uuid::Uuid;

use crate::middleware::auth::{require_permission, AuthContext};
use crate::AppState;
use zavora_erp_core::rbac::RoleRow;
use zavora_erp_core::services::rbac as svc;
use zavora_erp_core::ErpError;

fn er(e: ErpError) -> Response {
    super::err_response(e).into_response()
}

/// GET /api/v1/roles — assignable roles for the caller's tenant (system + custom).
/// Requires the `admin.manage` permission (used to populate user role dropdowns).
pub async fn list(
    ctx: AuthContext,
    State(state): State<Arc<AppState>>,
) -> Result<Json<serde_json::Value>, Response> {
    require_permission(&state, &ctx, "role.read").await.map_err(er)?;

    let rows = sqlx::query_as::<_, RoleRow>(
        "SELECT id, entity_id, key, name, description, is_system, is_assignable, created_at, updated_at \
         FROM roles \
         WHERE is_assignable = true AND (entity_id IS NULL OR entity_id = $1) \
         ORDER BY is_system DESC, name",
    )
    .bind(ctx.entity_id)
    .fetch_all(state.engine.pool())
    .await
    .map_err(|e| er(ErpError::Database(e)))?;

    Ok(Json(serde_json::to_value(rows).unwrap_or_default()))
}

/// GET /api/v1/permissions — the full permission catalog (for the matrix editor).
pub async fn list_permissions(
    ctx: AuthContext,
    State(state): State<Arc<AppState>>,
) -> Result<Json<serde_json::Value>, Response> {
    require_permission(&state, &ctx, "role.read").await.map_err(er)?;
    let perms = svc::list_permissions(&state.engine).await.map_err(er)?;
    Ok(Json(serde_json::to_value(perms).unwrap_or_default()))
}

/// GET /api/v1/roles/{id} — a role + its permission keys.
pub async fn detail(
    ctx: AuthContext,
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, Response> {
    require_permission(&state, &ctx, "role.read").await.map_err(er)?;
    let (role, perms) = svc::get_role_with_perms(&state.engine, ctx.entity_id, id).await.map_err(er)?;
    Ok(Json(serde_json::json!({ "role": role, "permissions": perms })))
}

#[derive(Deserialize)]
pub struct CreateRoleRequest {
    pub name: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub permissions: Vec<String>,
}

/// POST /api/v1/roles — create a per-tenant custom role.
pub async fn create(
    ctx: AuthContext,
    State(state): State<Arc<AppState>>,
    Json(req): Json<CreateRoleRequest>,
) -> Result<Json<serde_json::Value>, Response> {
    require_permission(&state, &ctx, "role.create").await.map_err(er)?;
    let id = svc::create_custom_role(
        &state.engine,
        ctx.entity_id,
        &req.name,
        req.description.as_deref(),
        &req.permissions,
    )
    .await
    .map_err(er)?;
    state.permissions.clear(); // effective permissions may have changed
    Ok(Json(serde_json::json!({ "id": id })))
}

#[derive(Deserialize)]
pub struct UpdateRoleRequest {
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    /// When present, replaces the role's permission set entirely.
    #[serde(default)]
    pub permissions: Option<Vec<String>>,
}

/// PUT /api/v1/roles/{id} — edit a custom role (system roles are read-only).
pub async fn update(
    ctx: AuthContext,
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
    Json(req): Json<UpdateRoleRequest>,
) -> Result<Json<serde_json::Value>, Response> {
    require_permission(&state, &ctx, "role.update").await.map_err(er)?;
    svc::update_custom_role(
        &state.engine,
        ctx.entity_id,
        id,
        req.name.as_deref(),
        req.description.as_deref(),
        req.permissions.as_deref(),
    )
    .await
    .map_err(er)?;
    state.permissions.clear();
    Ok(Json(serde_json::json!({ "status": "ok" })))
}

/// DELETE /api/v1/roles/{id} — delete a custom role (blocked if in use).
pub async fn delete(
    ctx: AuthContext,
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, Response> {
    require_permission(&state, &ctx, "role.delete").await.map_err(er)?;
    svc::delete_custom_role(&state.engine, ctx.entity_id, id).await.map_err(er)?;
    state.permissions.clear();
    Ok(Json(serde_json::json!({ "status": "deleted" })))
}
