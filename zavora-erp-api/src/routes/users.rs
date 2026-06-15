use axum::{extract::State, http::HeaderMap, Json};
use std::sync::Arc;
use uuid::Uuid;

use crate::AppState;
use crate::middleware::auth::{require_role, AuthContext, ROLES_MANAGE};
use super::err_response;
use zavora_erp_core::rbac::{CreateUserRequest, EraUserRow};
use zavora_erp_core::ErpError;

#[derive(serde::Deserialize)]
pub struct LoginRequest {
    pub email: String,
}

/// POST /auth/login — resolve a user's stored identity by email.
///
/// Identity is established by an upstream gateway / JWT layer in production; this
/// endpoint lets the SPA resolve the user record (id, entity, role) so it can attach
/// the `X-User-Id` / `X-Entity-Id` / `X-User-Role` headers the API expects. It is not
/// a credential check and intentionally requires no auth.
pub async fn login(
    State(state): State<Arc<AppState>>,
    Json(req): Json<LoginRequest>,
) -> Result<Json<serde_json::Value>, impl axum::response::IntoResponse> {
    let row = sqlx::query_as::<_, EraUserRow>(
        "SELECT * FROM era_users WHERE entity_id = $1 AND lower(email) = lower($2) AND is_active = true",
    )
    .bind(state.engine.entity_id())
    .bind(&req.email)
    .fetch_optional(state.engine.pool())
    .await
    .map_err(|e| err_response(ErpError::Database(e)))?;

    let Some(user) = row else {
        return Err(err_response(ErpError::NotFound {
            entity_type: "User".to_string(),
            id: Uuid::nil(),
        }));
    };

    let _ = sqlx::query("UPDATE era_users SET last_login = NOW() WHERE id = $1")
        .bind(user.id)
        .execute(state.engine.pool())
        .await;

    Ok(Json(serde_json::json!({
        "user_id": user.id,
        "entity_id": user.entity_id,
        "role": user.role,
        "display_name": user.display_name,
        "email": user.email,
    })))
}

/// GET /users — list users for the entity (Owner/Admin only).
pub async fn list(
    ctx: AuthContext,
    State(state): State<Arc<AppState>>,
) -> Result<Json<serde_json::Value>, impl axum::response::IntoResponse> {
    require_role(ROLES_MANAGE, &ctx, "list users").map_err(err_response)?;

    let rows = sqlx::query_as::<_, EraUserRow>(
        "SELECT * FROM era_users WHERE entity_id = $1 ORDER BY created_at",
    )
    .bind(state.engine.entity_id())
    .fetch_all(state.engine.pool())
    .await;

    match rows {
        Ok(r) => Ok(Json(serde_json::to_value(r).unwrap_or_default())),
        Err(e) => Err(err_response(ErpError::Database(e))),
    }
}

/// POST /users — create/invite a user.
///
/// Bootstrap: when no active users exist for the entity, the first user can be created
/// without authentication (this becomes the initial Owner/Admin). Afterwards, creating
/// users requires an Owner or Admin role (read from identity headers).
pub async fn create(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(req): Json<CreateUserRequest>,
) -> Result<Json<serde_json::Value>, impl axum::response::IntoResponse> {
    let active_users = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM era_users WHERE entity_id = $1 AND is_active = true",
    )
    .bind(state.engine.entity_id())
    .fetch_one(state.engine.pool())
    .await
    .map_err(|e| err_response(ErpError::Database(e)))?;

    let invited_by = if active_users == 0 {
        // Bootstrap: first user, no auth required.
        None
    } else {
        let role = headers
            .get("x-user-role")
            .and_then(|v| v.to_str().ok())
            .map(|s| s.to_lowercase());
        let allowed = matches!(role.as_deref(), Some("owner") | Some("admin"));
        if !allowed {
            return Err(err_response(ErpError::PermissionDenied {
                action: "create user".to_string(),
                required_role: "Owner, Admin".to_string(),
            }));
        }
        headers
            .get("x-user-id")
            .and_then(|v| v.to_str().ok())
            .and_then(|s| Uuid::parse_str(s).ok())
    };

    // Canonical role string (e.g. "Owner") for storage.
    let role_str = serde_json::to_value(&req.role)
        .ok()
        .and_then(|v| v.as_str().map(String::from))
        .unwrap_or_else(|| "Viewer".to_string());

    let id = Uuid::new_v4();
    let result = sqlx::query(
        "INSERT INTO era_users (id, entity_id, email, display_name, role, invited_by) VALUES ($1, $2, $3, $4, $5, $6)",
    )
    .bind(id)
    .bind(state.engine.entity_id())
    .bind(&req.email)
    .bind(&req.display_name)
    .bind(&role_str)
    .bind(invited_by)
    .execute(state.engine.pool())
    .await;

    match result {
        Ok(_) => Ok(Json(serde_json::json!({
            "id": id,
            "email": req.email,
            "display_name": req.display_name,
            "role": role_str,
        }))),
        Err(e) => Err(err_response(ErpError::Database(e))),
    }
}
