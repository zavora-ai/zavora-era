use axum::extract::State;
use axum::response::{IntoResponse, Response};
use axum::Json;
use std::sync::Arc;
use uuid::Uuid;

use crate::AppState;
use crate::middleware::auth::{jwt_config, require_role, served_entity, AuthContext, ROLES_MANAGE};
use zavora_erp_core::auth::{self, TokenPair};
use zavora_erp_core::rbac::{CreateUserRequest, EraUserRow};
use zavora_erp_core::ErpError;

/// Map an `ErpError` to a concrete HTTP `Response`.
fn er(e: ErpError) -> Response {
    super::err_response(e).into_response()
}

/// Minimal projection of `era_users` needed to authenticate.
#[derive(sqlx::FromRow)]
struct AuthUserRow {
    id: Uuid,
    entity_id: Uuid,
    email: String,
    display_name: String,
    role: String,
    is_active: bool,
    password_hash: Option<String>,
}

#[derive(serde::Deserialize)]
pub struct LoginRequest {
    pub email: String,
    pub password: String,
}

fn token_response(user: &AuthUserRow, pair: &TokenPair) -> serde_json::Value {
    serde_json::json!({
        "access_token": pair.access_token,
        "refresh_token": pair.refresh_token,
        "token_type": "Bearer",
        "expires_in": pair.expires_in,
        "user": {
            "user_id": user.id,
            "entity_id": user.entity_id,
            "role": user.role,
            "display_name": user.display_name,
            "email": user.email,
        }
    })
}

/// Persist a freshly issued refresh token so it can later be revoked.
async fn store_refresh_token(
    pool: &sqlx::PgPool,
    pair: &TokenPair,
    user: &AuthUserRow,
) -> Result<(), ErpError> {
    sqlx::query(
        "INSERT INTO refresh_tokens (jti, user_id, entity_id, expires_at) VALUES ($1, $2, $3, $4)",
    )
    .bind(pair.refresh_jti)
    .bind(user.id)
    .bind(user.entity_id)
    .bind(pair.refresh_expires_at)
    .execute(pool)
    .await
    .map_err(ErpError::Database)?;
    Ok(())
}

async fn fetch_auth_user(
    pool: &sqlx::PgPool,
    by_email: Option<&str>,
    by_id: Option<Uuid>,
) -> Result<Option<AuthUserRow>, ErpError> {
    let base = "SELECT id, entity_id, email, display_name, role, is_active, password_hash \
                FROM era_users WHERE entity_id = $1 AND ";
    let sql = if by_email.is_some() {
        format!("{base} lower(email) = lower($2)")
    } else {
        format!("{base} id = $2")
    };
    let mut q = sqlx::query_as::<_, AuthUserRow>(&sql).bind(served_entity());
    q = match by_email {
        Some(email) => q.bind(email.to_string()),
        None => q.bind(by_id.unwrap()),
    };
    q.fetch_optional(pool).await.map_err(ErpError::Database)
}

/// POST /auth/login — verify email + password and issue a JWT token pair.
pub async fn login(
    State(state): State<Arc<AppState>>,
    Json(req): Json<LoginRequest>,
) -> Result<Json<serde_json::Value>, Response> {
    // Uniform error for unknown user / bad password / inactive (no enumeration).
    let invalid = || er(ErpError::Unauthorized {
        message: "Invalid email or password".to_string(),
    });

    let user = fetch_auth_user(state.engine.pool(), Some(&req.email), None)
        .await
        .map_err(er)?
        .ok_or_else(invalid)?;

    if !user.is_active {
        return Err(invalid());
    }
    let Some(hash) = user.password_hash.as_deref() else {
        return Err(er(ErpError::Unauthorized {
            message: "Account has no password set; complete registration first".to_string(),
        }));
    };
    if !auth::verify_password(&req.password, hash) {
        return Err(invalid());
    }

    let pair = auth::issue_token_pair(jwt_config(), user.id, user.entity_id, &user.role)
        .map_err(er)?;
    store_refresh_token(state.engine.pool(), &pair, &user).await.map_err(er)?;

    let _ = sqlx::query("UPDATE era_users SET last_login = NOW() WHERE id = $1")
        .bind(user.id)
        .execute(state.engine.pool())
        .await;

    Ok(Json(token_response(&user, &pair)))
}

#[derive(serde::Deserialize)]
pub struct RefreshRequest {
    pub refresh_token: String,
}

/// POST /auth/refresh — exchange a valid, non-revoked refresh token for a new
/// token pair (rotating the refresh token).
pub async fn refresh(
    State(state): State<Arc<AppState>>,
    Json(req): Json<RefreshRequest>,
) -> Result<Json<serde_json::Value>, Response> {
    let claims = auth::decode_refresh_token(jwt_config(), &req.refresh_token).map_err(er)?;
    let jti = claims.jti.ok_or_else(|| er(ErpError::Unauthorized {
        message: "Refresh token missing id".to_string(),
    }))?;

    let valid = sqlx::query_scalar::<_, bool>(
        "SELECT EXISTS(SELECT 1 FROM refresh_tokens \
         WHERE jti = $1 AND revoked = false AND expires_at > NOW())",
    )
    .bind(jti)
    .fetch_one(state.engine.pool())
    .await
    .map_err(|e| er(ErpError::Database(e)))?;

    if !valid {
        return Err(er(ErpError::Unauthorized {
            message: "Refresh token is expired or revoked".to_string(),
        }));
    }

    let user = fetch_auth_user(state.engine.pool(), None, Some(claims.sub))
        .await
        .map_err(er)?
        .filter(|u| u.is_active)
        .ok_or_else(|| er(ErpError::Unauthorized {
            message: "User no longer active".to_string(),
        }))?;

    let pair = auth::issue_token_pair(jwt_config(), user.id, user.entity_id, &user.role)
        .map_err(er)?;

    // Rotate: revoke the presented token, store the new one — atomically.
    let mut tx = state.engine.pool().begin().await.map_err(|e| er(ErpError::Database(e)))?;
    sqlx::query("UPDATE refresh_tokens SET revoked = true WHERE jti = $1")
        .bind(jti)
        .execute(&mut *tx)
        .await
        .map_err(|e| er(ErpError::Database(e)))?;
    sqlx::query(
        "INSERT INTO refresh_tokens (jti, user_id, entity_id, expires_at) VALUES ($1, $2, $3, $4)",
    )
    .bind(pair.refresh_jti)
    .bind(user.id)
    .bind(user.entity_id)
    .bind(pair.refresh_expires_at)
    .execute(&mut *tx)
    .await
    .map_err(|e| er(ErpError::Database(e)))?;
    tx.commit().await.map_err(|e| er(ErpError::Database(e)))?;

    Ok(Json(token_response(&user, &pair)))
}

#[derive(serde::Deserialize)]
pub struct RegisterRequest {
    pub email: String,
    pub display_name: String,
    pub password: String,
}

/// POST /auth/register — bootstrap the first Owner account for the served entity.
///
/// Only permitted when the entity has no active users yet; subsequent accounts
/// are added through the authenticated invite flow (`POST /users`).
pub async fn register(
    State(state): State<Arc<AppState>>,
    Json(req): Json<RegisterRequest>,
) -> Result<Json<serde_json::Value>, Response> {
    if req.password.len() < 8 {
        return Err(er(ErpError::ValidationFailed {
            message: "Password must be at least 8 characters".to_string(),
        }));
    }

    let active_users = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM era_users WHERE entity_id = $1 AND is_active = true",
    )
    .bind(served_entity())
    .fetch_one(state.engine.pool())
    .await
    .map_err(|e| er(ErpError::Database(e)))?;

    if active_users > 0 {
        return Err(er(ErpError::ValidationFailed {
            message: "Registration is closed; ask an administrator to invite you".to_string(),
        }));
    }

    let hash = auth::hash_password(&req.password).map_err(er)?;
    let id = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO era_users (id, entity_id, email, display_name, role, password_hash, status) \
         VALUES ($1, $2, $3, $4, 'Owner', $5, 'active')",
    )
    .bind(id)
    .bind(served_entity())
    .bind(&req.email)
    .bind(&req.display_name)
    .bind(&hash)
    .execute(state.engine.pool())
    .await
    .map_err(|e| er(ErpError::Database(e)))?;

    let user = AuthUserRow {
        id,
        entity_id: served_entity(),
        email: req.email.clone(),
        display_name: req.display_name.clone(),
        role: "Owner".to_string(),
        is_active: true,
        password_hash: Some(hash),
    };
    let pair = auth::issue_token_pair(jwt_config(), id, served_entity(), "Owner").map_err(er)?;
    store_refresh_token(state.engine.pool(), &pair, &user).await.map_err(er)?;

    Ok(Json(token_response(&user, &pair)))
}

/// GET /users — list users for the entity (Owner/Admin only).
pub async fn list(
    ctx: AuthContext,
    State(state): State<Arc<AppState>>,
) -> Result<Json<serde_json::Value>, Response> {
    require_role(ROLES_MANAGE, &ctx, "list users").map_err(er)?;

    let rows = sqlx::query_as::<_, EraUserRow>(
        "SELECT * FROM era_users WHERE entity_id = $1 ORDER BY created_at",
    )
    .bind(ctx.entity_id)
    .fetch_all(state.engine.pool())
    .await;

    match rows {
        Ok(r) => Ok(Json(serde_json::to_value(r).unwrap_or_default())),
        Err(e) => Err(er(ErpError::Database(e))),
    }
}

/// POST /users — invite a user (Owner/Admin only).
pub async fn create(
    ctx: AuthContext,
    State(state): State<Arc<AppState>>,
    Json(req): Json<CreateUserRequest>,
) -> Result<Json<serde_json::Value>, Response> {
    require_role(ROLES_MANAGE, &ctx, "create user").map_err(er)?;

    let role_str = serde_json::to_value(&req.role)
        .ok()
        .and_then(|v| v.as_str().map(String::from))
        .unwrap_or_else(|| "Viewer".to_string());

    let id = Uuid::new_v4();
    let result = sqlx::query(
        "INSERT INTO era_users (id, entity_id, email, display_name, role, invited_by, invited_at, status) \
         VALUES ($1, $2, $3, $4, $5, $6, NOW(), 'invited')",
    )
    .bind(id)
    .bind(ctx.entity_id)
    .bind(&req.email)
    .bind(&req.display_name)
    .bind(&role_str)
    .bind(ctx.user_id)
    .execute(state.engine.pool())
    .await;

    match result {
        Ok(_) => Ok(Json(serde_json::json!({
            "id": id,
            "email": req.email,
            "display_name": req.display_name,
            "role": role_str,
        }))),
        Err(e) => Err(er(ErpError::Database(e))),
    }
}
