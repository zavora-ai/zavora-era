//! Employee self-service (ESS) authentication endpoints.
//!
//! Mirrors `routes::portal_auth` but for the `employee_users` principal class.
//! A successful login issues a token pair with `role = "Employee"` (see
//! `middleware::staff_auth`). Accounts are created by HR via the invite
//! endpoint; login requires an `active` account with a password set.

use axum::extract::State;
use axum::response::{IntoResponse, Response};
use axum::Json;
use std::sync::Arc;
use uuid::Uuid;

use crate::middleware::auth::jwt_config;
use crate::middleware::staff_auth::{StaffContext, STAFF_ROLE};
use crate::AppState;
use zavora_erp_core::auth::{self, TokenPair};
use zavora_erp_core::hr::{EmployeeUserRow, StaffLoginRequest};
use zavora_erp_core::ErpError;

fn er(e: ErpError) -> Response {
    super::err_response(e).into_response()
}

const REFRESH_COOKIE: &str = "staff_refresh";

fn is_production() -> bool {
    std::env::var("APP_ENV").map(|v| v.eq_ignore_ascii_case("production")).unwrap_or(false)
}

/// Refresh cookie scoped to the staff auth path so it never rides on ERP calls.
fn set_refresh_cookie(token: &str, max_age_secs: i64) -> String {
    let secure = if is_production() { "; Secure" } else { "" };
    format!(
        "{REFRESH_COOKIE}={token}; HttpOnly; SameSite=Strict; Path=/api/v1/staff; Max-Age={max_age_secs}{secure}"
    )
}

fn clear_refresh_cookie() -> String {
    let secure = if is_production() { "; Secure" } else { "" };
    format!("{REFRESH_COOKIE}=; HttpOnly; SameSite=Strict; Path=/api/v1/staff; Max-Age=0{secure}")
}

fn read_refresh_cookie(headers: &axum::http::HeaderMap) -> Option<String> {
    let raw = headers.get(axum::http::header::COOKIE)?.to_str().ok()?;
    raw.split(';').find_map(|kv| {
        let (k, v) = kv.trim().split_once('=')?;
        (k == REFRESH_COOKIE).then(|| v.to_string())
    })
}

fn token_body(staff: &EmployeeUserRow, pair: &TokenPair) -> serde_json::Value {
    serde_json::json!({
        "access_token": pair.access_token,
        "token_type": "Bearer",
        "expires_in": pair.expires_in,
        "staff": staff,
    })
}

fn auth_success(staff: &EmployeeUserRow, pair: &TokenPair) -> Response {
    let max_age = (pair.refresh_expires_at - chrono::Utc::now()).num_seconds().max(0);
    let cookie = set_refresh_cookie(&pair.refresh_token, max_age);
    ([(axum::http::header::SET_COOKIE, cookie)], Json(token_body(staff, pair))).into_response()
}

async fn store_refresh_token(pool: &sqlx::PgPool, pair: &TokenPair, uid: Uuid, entity_id: Uuid) -> Result<(), ErpError> {
    sqlx::query("INSERT INTO refresh_tokens (jti, user_id, entity_id, expires_at) VALUES ($1,$2,$3,$4)")
        .bind(pair.refresh_jti).bind(uid).bind(entity_id).bind(pair.refresh_expires_at)
        .execute(pool).await.map_err(ErpError::Database)?;
    Ok(())
}

async fn fetch_staff(pool: &sqlx::PgPool, id: Uuid, entity_id: Uuid) -> Result<Option<EmployeeUserRow>, ErpError> {
    sqlx::query_as::<_, EmployeeUserRow>(
        "SELECT id, entity_id, email, display_name, status, employee_id, last_login, created_at \
         FROM employee_users WHERE id = $1 AND entity_id = $2",
    )
    .bind(id).bind(entity_id).fetch_optional(pool).await.map_err(ErpError::Database)
}

/// POST /api/v1/staff/login — verify password, issue an Employee token pair.
pub async fn login(
    State(state): State<Arc<AppState>>,
    Json(req): Json<StaffLoginRequest>,
) -> Result<Response, Response> {
    let entity_id = crate::middleware::auth::served_entity();
    let invalid = || er(ErpError::Unauthorized { message: "Invalid email or password".into() });

    let row: Option<(Uuid, Option<String>, String)> = sqlx::query_as(
        "SELECT id, password_hash, status FROM employee_users WHERE entity_id = $1 AND lower(email) = lower($2)",
    )
    .bind(entity_id).bind(req.email.trim())
    .fetch_optional(state.engine.pool()).await.map_err(|e| er(ErpError::Database(e)))?;

    let (uid, hash, status) = row.ok_or_else(invalid)?;
    let hash = hash.ok_or_else(|| er(ErpError::Unauthorized { message: "Your account is not yet activated. Ask HR to set your password.".into() }))?;
    if !auth::verify_password(&req.password, &hash) {
        return Err(invalid());
    }
    if status != "active" {
        return Err(er(ErpError::Unauthorized { message: "Your account is not active. Contact HR.".into() }));
    }

    let pair = auth::issue_token_pair(jwt_config(), uid, entity_id, STAFF_ROLE).map_err(er)?;
    store_refresh_token(state.engine.pool(), &pair, uid, entity_id).await.map_err(er)?;
    let _ = sqlx::query("UPDATE employee_users SET last_login = NOW() WHERE id = $1").bind(uid).execute(state.engine.pool()).await;

    let staff = fetch_staff(state.engine.pool(), uid, entity_id).await.map_err(er)?.ok_or_else(invalid)?;
    Ok(auth_success(&staff, &pair))
}

/// POST /api/v1/staff/refresh — rotate the refresh cookie for a new pair.
pub async fn refresh(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
) -> Result<Response, Response> {
    let token = read_refresh_cookie(&headers).ok_or_else(|| er(ErpError::Unauthorized { message: "Missing refresh token".into() }))?;
    let claims = auth::decode_refresh_token(jwt_config(), &token).map_err(er)?;
    if claims.role != STAFF_ROLE {
        return Err(er(ErpError::Unauthorized { message: "Not a staff session".into() }));
    }
    let jti = claims.jti.ok_or_else(|| er(ErpError::Unauthorized { message: "Refresh token missing id".into() }))?;

    let valid: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM refresh_tokens WHERE jti = $1 AND revoked = false AND expires_at > NOW())",
    )
    .bind(jti).fetch_one(state.engine.pool()).await.map_err(|e| er(ErpError::Database(e)))?;
    if !valid {
        return Err(er(ErpError::Unauthorized { message: "Session expired".into() }));
    }

    let _ = sqlx::query("UPDATE refresh_tokens SET revoked = true WHERE jti = $1").bind(jti).execute(state.engine.pool()).await;
    let pair = auth::issue_token_pair(jwt_config(), claims.sub, claims.entity_id, STAFF_ROLE).map_err(er)?;
    store_refresh_token(state.engine.pool(), &pair, claims.sub, claims.entity_id).await.map_err(er)?;

    let staff = fetch_staff(state.engine.pool(), claims.sub, claims.entity_id).await.map_err(er)?
        .ok_or_else(|| er(ErpError::Unauthorized { message: "Staff not found".into() }))?;
    Ok(auth_success(&staff, &pair))
}

/// POST /api/v1/staff/logout — revoke the current session + clear the cookie.
pub async fn logout(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
) -> Response {
    if let Some(token) = read_refresh_cookie(&headers) {
        if let Ok(claims) = auth::decode_refresh_token(jwt_config(), &token) {
            if let Some(jti) = claims.jti {
                let _ = sqlx::query("UPDATE refresh_tokens SET revoked = true WHERE jti = $1").bind(jti).execute(state.engine.pool()).await;
            }
        }
    }
    ([(axum::http::header::SET_COOKIE, clear_refresh_cookie())], Json(serde_json::json!({ "ok": true }))).into_response()
}

/// GET /api/v1/staff/me — the authenticated employee's own login profile.
pub async fn me(
    ctx: StaffContext,
    State(state): State<Arc<AppState>>,
) -> Result<Json<EmployeeUserRow>, Response> {
    let staff = fetch_staff(state.engine.pool(), ctx.employee_user_id, ctx.entity_id).await.map_err(er)?
        .ok_or_else(|| er(ErpError::Unauthorized { message: "Staff not found".into() }))?;
    Ok(Json(staff))
}
