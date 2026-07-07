use axum::extract::{Path, State};
use axum::response::{IntoResponse, Response};
use axum::Json;
use std::sync::Arc;
use uuid::Uuid;

use crate::AppState;
use crate::middleware::auth::{jwt_config, require_permission, served_entity, AuthContext};
use zavora_erp_core::auth::{self, TokenPair};
use zavora_erp_core::rbac::{CreateUserRequest, EraUserRow, UpdateUserRequest};
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
    // Note: the refresh token is intentionally NOT in the body — it is delivered
    // only as an httpOnly cookie so it is never exposed to JavaScript.
    serde_json::json!({
        "access_token": pair.access_token,
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

const REFRESH_COOKIE: &str = "era_refresh";

fn is_production() -> bool {
    std::env::var("APP_ENV")
        .map(|v| v.eq_ignore_ascii_case("production"))
        .unwrap_or(false)
}

/// Build the `Set-Cookie` value carrying the refresh token. httpOnly so JS can
/// never read it; SameSite=Strict to defeat CSRF; scoped to the auth path.
fn set_refresh_cookie(token: &str, max_age_secs: i64) -> String {
    let secure = if is_production() { "; Secure" } else { "" };
    format!(
        "{REFRESH_COOKIE}={token}; HttpOnly; SameSite=Strict; Path=/api/v1/auth; Max-Age={max_age_secs}{secure}"
    )
}

fn clear_refresh_cookie() -> String {
    let secure = if is_production() { "; Secure" } else { "" };
    format!("{REFRESH_COOKIE}=; HttpOnly; SameSite=Strict; Path=/api/v1/auth; Max-Age=0{secure}")
}

fn read_refresh_cookie(headers: &axum::http::HeaderMap) -> Option<String> {
    let raw = headers.get(axum::http::header::COOKIE)?.to_str().ok()?;
    raw.split(';').find_map(|kv| {
        let (k, v) = kv.trim().split_once('=')?;
        (k == REFRESH_COOKIE).then(|| v.to_string())
    })
}

/// Build a success response: access token + user in the body, refresh token in
/// an httpOnly cookie.
fn auth_success(user: &AuthUserRow, pair: &TokenPair) -> Response {
    let max_age = (pair.refresh_expires_at - chrono::Utc::now())
        .num_seconds()
        .max(0);
    let cookie = set_refresh_cookie(&pair.refresh_token, max_age);
    (
        [(axum::http::header::SET_COOKIE, cookie)],
        Json(token_response(user, pair)),
    )
        .into_response()
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
/// Whether `key` is a valid role for the tenant (system role or tenant custom).
async fn role_exists(pool: &sqlx::PgPool, entity_id: Uuid, key: &str) -> Result<bool, ErpError> {
    sqlx::query_scalar::<_, bool>(
        "SELECT EXISTS(SELECT 1 FROM roles WHERE key = $1 AND (entity_id = $2 OR entity_id IS NULL))",
    )
    .bind(key)
    .bind(entity_id)
    .fetch_one(pool)
    .await
    .map_err(ErpError::Database)
}


async fn fetch_auth_user(
    pool: &sqlx::PgPool,
    by_email: Option<&str>,
    by_id: Option<Uuid>,
) -> Result<Option<AuthUserRow>, ErpError> {
    // Look up the user GLOBALLY (email / id are the login identity). Previously
    // this was scoped to `served_entity()`, so only the startup tenant's users
    // could log in — every other signed-up tenant was locked out. The returned
    // row carries entity_id, so the issued token still binds the correct tenant.
    let base = "SELECT id, entity_id, email, display_name, role, is_active, password_hash \
                FROM era_users WHERE ";
    let sql = if by_email.is_some() {
        format!("{base} lower(email) = lower($1) ORDER BY id LIMIT 1")
    } else {
        format!("{base} id = $1")
    };
    let mut q = sqlx::query_as::<_, AuthUserRow>(&sql);
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
) -> Result<Response, Response> {
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

    Ok(auth_success(&user, &pair))
}

/// POST /auth/refresh — exchange the httpOnly refresh-token cookie for a new
/// token pair (rotating the refresh token and re-issuing the cookie).
pub async fn refresh(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
) -> Result<Response, Response> {
    let refresh_token = read_refresh_cookie(&headers).ok_or_else(|| er(ErpError::Unauthorized {
        message: "Missing refresh token".to_string(),
    }))?;
    let claims = auth::decode_refresh_token(jwt_config(), &refresh_token).map_err(er)?;
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

    Ok(auth_success(&user, &pair))
}

/// POST /auth/logout — revoke the current refresh token and clear its cookie.
pub async fn logout(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
) -> Result<Response, Response> {
    if let Some(token) = read_refresh_cookie(&headers) {
        if let Ok(claims) = auth::decode_refresh_token(jwt_config(), &token) {
            if let Some(jti) = claims.jti {
                let _ = sqlx::query("UPDATE refresh_tokens SET revoked = true WHERE jti = $1")
                    .bind(jti)
                    .execute(state.engine.pool())
                    .await;
            }
        }
    }
    Ok((
        [(axum::http::header::SET_COOKIE, clear_refresh_cookie())],
        Json(serde_json::json!({ "ok": true })),
    )
        .into_response())
}

#[derive(serde::Deserialize)]
pub struct RegisterRequest {
    pub email: String,
    pub display_name: String,
    pub password: String,
}

/// POST /auth/register — bootstrap the first Owner account for the served entity.
///
/// **Deprecated.** This is the legacy single-tenant bootstrap path: it only
/// creates the first Owner for the process-global served entity (the one fixed
/// by the `ENTITY_ID` environment variable) and does **not** create a new
/// tenant. Its bootstrap behaviour is retained unchanged for backward
/// compatibility with existing single-tenant deployments (Requirement 9.2).
///
/// New tenants MUST be created through the supported public tenant-creation
/// path, `POST /api/v1/auth/signup` (the `Signup_Service`), which provisions a
/// brand-new isolated tenant, its `entity_settings`, and its first Owner in a
/// single transaction. Prefer `/api/v1/auth/signup` for all new integrations
/// (Requirement 9.3).
///
/// Only permitted when the entity has no active users yet; subsequent accounts
/// are added through the authenticated invite flow (`POST /users`).
#[deprecated(
    note = "Legacy single-tenant bootstrap. Use POST /api/v1/auth/signup (Signup_Service) to create new tenants."
)]
pub async fn register(
    State(state): State<Arc<AppState>>,
    Json(req): Json<RegisterRequest>,
) -> Result<Response, Response> {
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

    Ok(auth_success(&user, &pair))
}

/// GET /users — list users for the entity (Owner/Admin only).
pub async fn list(
    ctx: AuthContext,
    State(state): State<Arc<AppState>>,
) -> Result<Json<serde_json::Value>, Response> {
    require_permission(&state, &ctx, "user.manage").await.map_err(er)?;

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
    require_permission(&state, &ctx, "user.manage").await.map_err(er)?;

    let role_str = req.role.trim().to_string();
    if !role_exists(state.engine.pool(), ctx.entity_id, &role_str).await.map_err(er)? {
        return Err(er(ErpError::ValidationFailed { message: format!("Unknown role '{role_str}'") }));
    }

    // Optional initial password → active account; otherwise an invited stub with
    // a single-use activation token (emailed as a set-password link).
    let (password_hash, status, set_token) = match req.password.as_deref() {
        Some(pw) => {
            if pw.len() < 8 {
                return Err(er(ErpError::ValidationFailed {
                    message: "Password must be at least 8 characters".to_string(),
                }));
            }
            (Some(auth::hash_password(pw).map_err(er)?), "active", None)
        }
        None => (None, "invited", Some(Uuid::new_v4().to_string())),
    };

    let id = Uuid::new_v4();
    let result = sqlx::query(
        "INSERT INTO era_users (id, entity_id, email, display_name, role, password_hash, invited_by, invited_at, status, set_token, set_token_expires) \
         VALUES ($1, $2, $3, $4, $5, $6, $7, NOW(), $8, $9, CASE WHEN $9 IS NULL THEN NULL ELSE NOW() + INTERVAL '7 days' END)",
    )
    .bind(id)
    .bind(ctx.entity_id)
    .bind(&req.email)
    .bind(&req.display_name)
    .bind(&role_str)
    .bind(&password_hash)
    .bind(ctx.user_id)
    .bind(status)
    .bind(&set_token)
    .execute(state.engine.pool())
    .await;

    match result {
        Ok(_) => {
            // Invited (no password): email the activation link.
            if let Some(token) = &set_token {
                send_activation_email(&state, ctx.entity_id, &req.email, id, token, true).await;
            }
            Ok(Json(serde_json::json!({
                "id": id,
                "email": req.email,
                "display_name": req.display_name,
                "role": role_str,
                "status": status,
            })))
        }
        Err(e) => Err(er(ErpError::Database(e))),
    }
}

/// Base URL for building set-password links in emails. Falls back to dev.
fn app_base_url() -> String {
    std::env::var("APP_BASE_URL").unwrap_or_else(|_| "http://localhost:3000".to_string())
}

/// Email an internal user a single-use set-password link (invite or reset).
async fn send_activation_email(
    state: &AppState,
    entity_id: Uuid,
    email: &str,
    user_id: Uuid,
    token: &str,
    is_invite: bool,
) {
    let link = format!("{}/set-password?token={}", app_base_url(), token);
    let (subject, body) = if is_invite {
        (
            "You've been invited to Zavora ERP".to_string(),
            format!("You've been invited to a Zavora ERP workspace. Set your password to sign in: {link}\n\nThis link expires in 7 days."),
        )
    } else {
        (
            "Reset your Zavora ERP password".to_string(),
            format!("Reset your password here: {link}\n\nThis link expires in 1 hour. If you didn't request this, ignore this email."),
        )
    };
    let req = zavora_erp_core::notifications::SendNotificationRequest {
        event_type: zavora_erp_core::notifications::NotificationEventType::LeaveRequestDecided,
        channels: vec![zavora_erp_core::types::Channel::Email],
        recipients: vec![email.to_string()],
        subject: Some(subject),
        body,
        related_type: Some("era_user".into()),
        related_id: Some(user_id),
        schedule_at: None,
        attachments: Vec::new(),
    };
    let _ = zavora_erp_core::services::notifications::send_notification(&state.engine, entity_id, req).await;
}

#[derive(serde::Deserialize)]
pub struct SetPasswordRequest {
    pub token: String,
    pub password: String,
}

/// POST /auth/set-password — consume a single-use token (invite accept or reset),
/// set the password, and activate the internal user. Public (no auth).
pub async fn set_password(
    State(state): State<Arc<AppState>>,
    Json(req): Json<SetPasswordRequest>,
) -> Result<Response, Response> {
    if req.password.len() < 8 {
        return Err(er(ErpError::ValidationFailed {
            message: "Password must be at least 8 characters".to_string(),
        }));
    }
    let row: Option<(Uuid,)> = sqlx::query_as(
        "SELECT id FROM era_users WHERE set_token = $1 AND set_token_expires > NOW()",
    )
    .bind(req.token.trim())
    .fetch_optional(state.engine.pool())
    .await
    .map_err(|e| er(ErpError::Database(e)))?;
    let (id,) = row.ok_or_else(|| er(ErpError::ValidationFailed {
        message: "This link is invalid or has expired. Ask an administrator to resend your invite.".to_string(),
    }))?;

    let hash = auth::hash_password(&req.password).map_err(er)?;
    sqlx::query(
        "UPDATE era_users SET password_hash = $1, status = 'active', is_active = true, \
             set_token = NULL, set_token_expires = NULL WHERE id = $2",
    )
    .bind(&hash)
    .bind(id)
    .execute(state.engine.pool())
    .await
    .map_err(|e| er(ErpError::Database(e)))?;

    Ok(Json(serde_json::json!({ "ok": true, "message": "Password set. You can now sign in." })).into_response())
}

#[derive(serde::Deserialize)]
pub struct ForgotPasswordRequest {
    pub email: String,
}

/// POST /auth/forgot-password — issue a reset token + email a link. Always returns
/// ok (no account enumeration). Public (no auth).
pub async fn forgot_password(
    State(state): State<Arc<AppState>>,
    Json(req): Json<ForgotPasswordRequest>,
) -> Response {
    // Match a single active account by email (globally; the token is single-use).
    let row: Option<(Uuid, Uuid, String)> = sqlx::query_as(
        "SELECT id, entity_id, email FROM era_users \
         WHERE lower(email) = lower($1) AND is_active = true ORDER BY id LIMIT 1",
    )
    .bind(req.email.trim())
    .fetch_optional(state.engine.pool())
    .await
    .ok()
    .flatten();

    if let Some((id, entity_id, email)) = row {
        let token = Uuid::new_v4().to_string();
        let _ = sqlx::query(
            "UPDATE era_users SET set_token = $1, set_token_expires = NOW() + INTERVAL '1 hour' WHERE id = $2",
        )
        .bind(&token)
        .bind(id)
        .execute(state.engine.pool())
        .await;
        send_activation_email(&state, entity_id, &email, id, &token, false).await;
    }
    Json(serde_json::json!({ "ok": true, "message": "If that account exists, a reset link has been sent." })).into_response()
}

/// POST /users/{id}/resend-invite — regenerate the activation token and re-email
/// the set-password link (Owner/Admin only). Only for accounts without a password.
pub async fn resend_invite(
    ctx: AuthContext,
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, Response> {
    require_permission(&state, &ctx, "user.manage").await.map_err(er)?;

    let target = sqlx::query_as::<_, (String, Option<String>)>(
        "SELECT email, password_hash FROM era_users WHERE entity_id = $1 AND id = $2",
    )
    .bind(ctx.entity_id)
    .bind(id)
    .fetch_optional(state.engine.pool())
    .await
    .map_err(|e| er(ErpError::Database(e)))?
    .ok_or_else(|| er(ErpError::NotFound { entity_type: "user".to_string(), id }))?;

    if target.1.is_some() {
        return Err(er(ErpError::ValidationFailed {
            message: "This user already has a password set; use forgot-password instead.".to_string(),
        }));
    }

    let token = Uuid::new_v4().to_string();
    sqlx::query(
        "UPDATE era_users SET set_token = $1, set_token_expires = NOW() + INTERVAL '7 days', status = 'invited' WHERE id = $2",
    )
    .bind(&token)
    .bind(id)
    .execute(state.engine.pool())
    .await
    .map_err(|e| er(ErpError::Database(e)))?;

    send_activation_email(&state, ctx.entity_id, &target.0, id, &token, true).await;
    Ok(Json(serde_json::json!({ "ok": true, "message": "Invitation re-sent." })))
}

/// PUT /users/{id} — update a user within the caller's tenant (Owner/Admin only).
///
/// Enforces first-Owner protection (Req 13.1, 13.2): while a tenant has exactly one
/// active Owner, that Owner can neither be deactivated nor have their role changed
/// to a non-Owner role. The target user is always loaded and updated scoped to the
/// caller's token `entity_id`, so the handler cannot touch another tenant's users
/// (Req 5.1, 5.2).
pub async fn update(
    ctx: AuthContext,
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
    Json(req): Json<UpdateUserRequest>,
) -> Result<Json<serde_json::Value>, Response> {
    require_permission(&state, &ctx, "user.manage").await.map_err(er)?;

    // Load the target user scoped to the caller's tenant (cross-tenant isolation).
    let target = sqlx::query_as::<_, EraUserRow>(
        "SELECT * FROM era_users WHERE entity_id = $1 AND id = $2",
    )
    .bind(ctx.entity_id)
    .bind(id)
    .fetch_optional(state.engine.pool())
    .await
    .map_err(|e| er(ErpError::Database(e)))?
    .ok_or_else(|| {
        er(ErpError::NotFound {
            entity_type: "user".to_string(),
            id,
        })
    })?;

    // Resolve the requested role key (if any) and validate it exists.
    let new_role: Option<String> = req.role.as_ref().map(|r| r.trim().to_string());
    if let Some(r) = &new_role {
        if !role_exists(state.engine.pool(), ctx.entity_id, r).await.map_err(er)? {
            return Err(er(ErpError::ValidationFailed { message: format!("Unknown role '{r}'") }));
        }
    }

    // Self-lockout guard: you cannot deactivate your own account or change your
    // own role (prevents an admin from accidentally locking themselves out).
    if id == ctx.user_id {
        if matches!(req.is_active, Some(false)) {
            return Err(er(ErpError::ValidationFailed {
                message: "You cannot deactivate your own account.".to_string(),
            }));
        }
        if new_role.as_deref().is_some_and(|r| r != target.role) {
            return Err(er(ErpError::ValidationFailed {
                message: "You cannot change your own role.".to_string(),
            }));
        }
    }

    // First-Owner protection (Req 13.1, 13.2): if this change would deactivate the
    // target Owner, or move the target Owner off the Owner role, the tenant must
    // retain at least one other active Owner.
    let target_is_owner = target.role == "Owner";
    let would_deactivate = matches!(req.is_active, Some(false));
    let would_demote = new_role.as_deref().is_some_and(|r| r != "Owner");

    if target_is_owner && (would_deactivate || would_demote) {
        let active_owners = sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM era_users \
             WHERE entity_id = $1 AND role = 'Owner' AND is_active = true",
        )
        .bind(ctx.entity_id)
        .fetch_one(state.engine.pool())
        .await
        .map_err(|e| er(ErpError::Database(e)))?;

        if active_owners <= 1 {
            return Err(er(ErpError::ValidationFailed {
                message: "Cannot deactivate or change the role of the tenant's sole active Owner"
                    .to_string(),
            }));
        }
    }

    // Apply the update, scoped by entity_id. COALESCE leaves omitted fields unchanged.
    let result = sqlx::query(
        "UPDATE era_users SET \
            display_name = COALESCE($3, display_name), \
            role = COALESCE($4, role), \
            is_active = COALESCE($5, is_active) \
         WHERE entity_id = $1 AND id = $2",
    )
    .bind(ctx.entity_id)
    .bind(id)
    .bind(req.display_name.as_ref())
    .bind(new_role.as_ref())
    .bind(req.is_active)
    .execute(state.engine.pool())
    .await
    .map_err(|e| er(ErpError::Database(e)))?;

    if result.rows_affected() == 0 {
        return Err(er(ErpError::NotFound {
            entity_type: "user".to_string(),
            id,
        }));
    }

    Ok(Json(serde_json::json!({
        "id": id,
        "display_name": req.display_name.unwrap_or(target.display_name),
        "role": new_role.unwrap_or(target.role),
        "is_active": req.is_active.unwrap_or(target.is_active),
    })))
}

/// GET /api/v1/auth/permissions — the current user's role + effective permission
/// keys. The frontend gates UI actions on this (single source of truth), so its
/// role/permission arrays can never drift from the backend.
pub async fn my_permissions(
    ctx: AuthContext,
    State(state): State<Arc<AppState>>,
) -> Result<Json<serde_json::Value>, Response> {
    let perms = state
        .permissions
        .effective(&state.engine, ctx.entity_id, &ctx.role_key)
        .await
        .map_err(er)?;
    let mut keys: Vec<String> = perms.iter().cloned().collect();
    keys.sort();
    Ok(Json(serde_json::json!({
        "role": ctx.role_key,
        "permissions": keys,
    })))
}
