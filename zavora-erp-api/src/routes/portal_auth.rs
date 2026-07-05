//! Vendor-portal authentication endpoints (public + self-service).
//!
//! Mirrors `routes::users` but for the external `vendor_users` principal class.
//! A successful login issues a token pair with `role = "Vendor"` (see
//! `middleware::vendor_auth`). Registrations land as `status = 'pending'` and are
//! activated by a buyer via the staff approval endpoint.

use axum::extract::State;
use axum::response::{IntoResponse, Response};
use axum::Json;
use std::sync::Arc;
use uuid::Uuid;

use crate::middleware::auth::{jwt_config, served_entity};
use crate::middleware::vendor_auth::{VendorContext, VENDOR_ROLE};
use crate::AppState;
use zavora_erp_core::auth::{self, TokenPair};
use zavora_erp_core::procurement::{RegisterVendorRequest, VendorLoginRequest, VendorUserRow};
use zavora_erp_core::ErpError;

fn er(e: ErpError) -> Response {
    super::err_response(e).into_response()
}

const REFRESH_COOKIE: &str = "vendor_refresh";

fn is_production() -> bool {
    std::env::var("APP_ENV").map(|v| v.eq_ignore_ascii_case("production")).unwrap_or(false)
}

/// Refresh cookie scoped to the portal auth path so it never rides on ERP calls.
fn set_refresh_cookie(token: &str, max_age_secs: i64) -> String {
    let secure = if is_production() { "; Secure" } else { "" };
    format!(
        "{REFRESH_COOKIE}={token}; HttpOnly; SameSite=Strict; Path=/api/v1/portal; Max-Age={max_age_secs}{secure}"
    )
}

fn clear_refresh_cookie() -> String {
    let secure = if is_production() { "; Secure" } else { "" };
    format!("{REFRESH_COOKIE}=; HttpOnly; SameSite=Strict; Path=/api/v1/portal; Max-Age=0{secure}")
}

fn read_refresh_cookie(headers: &axum::http::HeaderMap) -> Option<String> {
    let raw = headers.get(axum::http::header::COOKIE)?.to_str().ok()?;
    raw.split(';').find_map(|kv| {
        let (k, v) = kv.trim().split_once('=')?;
        (k == REFRESH_COOKIE).then(|| v.to_string())
    })
}

fn token_body(vendor: &VendorUserRow, pair: &TokenPair) -> serde_json::Value {
    serde_json::json!({
        "access_token": pair.access_token,
        "token_type": "Bearer",
        "expires_in": pair.expires_in,
        "vendor": vendor,
    })
}

fn auth_success(vendor: &VendorUserRow, pair: &TokenPair) -> Response {
    let max_age = (pair.refresh_expires_at - chrono::Utc::now()).num_seconds().max(0);
    let cookie = set_refresh_cookie(&pair.refresh_token, max_age);
    (
        [(axum::http::header::SET_COOKIE, cookie)],
        Json(token_body(vendor, pair)),
    )
        .into_response()
}

async fn store_refresh_token(pool: &sqlx::PgPool, pair: &TokenPair, vendor_user_id: Uuid, entity_id: Uuid) -> Result<(), ErpError> {
    sqlx::query("INSERT INTO refresh_tokens (jti, user_id, entity_id, expires_at) VALUES ($1,$2,$3,$4)")
        .bind(pair.refresh_jti).bind(vendor_user_id).bind(entity_id).bind(pair.refresh_expires_at)
        .execute(pool).await.map_err(ErpError::Database)?;
    Ok(())
}

/// Public projection of a vendor_user (never returns password_hash).
async fn fetch_vendor(pool: &sqlx::PgPool, id: Uuid, entity_id: Uuid) -> Result<Option<VendorUserRow>, ErpError> {
    sqlx::query_as::<_, VendorUserRow>(
        "SELECT id, entity_id, email, display_name, company_name, kra_pin, phone, status, vendor_id, last_login, created_at \
         FROM vendor_users WHERE id = $1 AND entity_id = $2",
    )
    .bind(id).bind(entity_id).fetch_optional(pool).await.map_err(ErpError::Database)
}

/// POST /api/v1/portal/register — public self-registration under the served
/// tenant. Lands as `pending`; a buyer approves to activate + link a vendor.
pub async fn register(
    State(state): State<Arc<AppState>>,
    Json(req): Json<RegisterVendorRequest>,
) -> Result<Response, Response> {
    let entity_id = served_entity();
    if req.email.trim().is_empty() || req.password.len() < 8 {
        return Err(er(ErpError::ValidationFailed { message: "email and an 8+ character password are required".into() }));
    }
    let hash = auth::hash_password(&req.password).map_err(er)?;
    let id = Uuid::new_v4();
    let row = sqlx::query_as::<_, VendorUserRow>(
        "INSERT INTO vendor_users (id, entity_id, email, display_name, company_name, kra_pin, phone, password_hash, status) \
         VALUES ($1,$2,$3,$4,$5,$6,$7,$8,'pending') \
         RETURNING id, entity_id, email, display_name, company_name, kra_pin, phone, status, vendor_id, last_login, created_at",
    )
    .bind(id).bind(entity_id).bind(req.email.trim().to_lowercase()).bind(&req.display_name)
    .bind(&req.company_name).bind(&req.kra_pin).bind(&req.phone).bind(&hash)
    .fetch_optional(state.engine.pool()).await
    .map_err(|_| er(ErpError::Duplicate { message: format!("a registration already exists for {}", req.email) }))?
    .ok_or_else(|| er(ErpError::Internal("registration failed".into())))?;

    Ok((axum::http::StatusCode::CREATED, Json(serde_json::json!({
        "vendor": row,
        "message": "Registration received. An account manager will review and approve your access.",
    }))).into_response())
}

/// POST /api/v1/portal/login — verify password, issue a Vendor token pair.
pub async fn login(
    State(state): State<Arc<AppState>>,
    Json(req): Json<VendorLoginRequest>,
) -> Result<Response, Response> {
    let entity_id = served_entity();
    let invalid = || er(ErpError::Unauthorized { message: "Invalid email or password".into() });

    let row: Option<(Uuid, String, String)> = sqlx::query_as(
        "SELECT id, password_hash, status FROM vendor_users WHERE entity_id = $1 AND lower(email) = lower($2)",
    )
    .bind(entity_id).bind(req.email.trim())
    .fetch_optional(state.engine.pool()).await.map_err(|e| er(ErpError::Database(e)))?;

    let (vendor_user_id, hash, status) = row.ok_or_else(invalid)?;
    if !auth::verify_password(&req.password, &hash) {
        return Err(invalid());
    }
    if status == "pending" {
        return Err(er(ErpError::Unauthorized { message: "Your registration is awaiting approval".into() }));
    }
    if status != "active" {
        return Err(er(ErpError::Unauthorized { message: "Your account is not active. Contact the buyer.".into() }));
    }

    let pair = auth::issue_token_pair(jwt_config(), vendor_user_id, entity_id, VENDOR_ROLE).map_err(er)?;
    store_refresh_token(state.engine.pool(), &pair, vendor_user_id, entity_id).await.map_err(er)?;
    let _ = sqlx::query("UPDATE vendor_users SET last_login = NOW() WHERE id = $1").bind(vendor_user_id).execute(state.engine.pool()).await;

    let vendor = fetch_vendor(state.engine.pool(), vendor_user_id, entity_id).await.map_err(er)?.ok_or_else(invalid)?;
    Ok(auth_success(&vendor, &pair))
}

/// POST /api/v1/portal/refresh — rotate the refresh cookie for a new pair.
pub async fn refresh(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
) -> Result<Response, Response> {
    let token = read_refresh_cookie(&headers).ok_or_else(|| er(ErpError::Unauthorized { message: "Missing refresh token".into() }))?;
    let claims = auth::decode_refresh_token(jwt_config(), &token).map_err(er)?;
    if claims.role != VENDOR_ROLE {
        return Err(er(ErpError::Unauthorized { message: "Not a vendor session".into() }));
    }
    let jti = claims.jti.ok_or_else(|| er(ErpError::Unauthorized { message: "Refresh token missing id".into() }))?;

    let valid: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM refresh_tokens WHERE jti = $1 AND revoked = false AND expires_at > NOW())",
    )
    .bind(jti).fetch_one(state.engine.pool()).await.map_err(|e| er(ErpError::Database(e)))?;
    if !valid {
        return Err(er(ErpError::Unauthorized { message: "Session expired".into() }));
    }

    // Rotate: revoke the old jti, issue + persist a new pair.
    let _ = sqlx::query("UPDATE refresh_tokens SET revoked = true WHERE jti = $1").bind(jti).execute(state.engine.pool()).await;
    let pair = auth::issue_token_pair(jwt_config(), claims.sub, claims.entity_id, VENDOR_ROLE).map_err(er)?;
    store_refresh_token(state.engine.pool(), &pair, claims.sub, claims.entity_id).await.map_err(er)?;

    let vendor = fetch_vendor(state.engine.pool(), claims.sub, claims.entity_id).await.map_err(er)?
        .ok_or_else(|| er(ErpError::Unauthorized { message: "Vendor not found".into() }))?;
    Ok(auth_success(&vendor, &pair))
}

/// POST /api/v1/portal/logout — revoke the current session + clear the cookie.
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

/// GET /api/v1/portal/me — the authenticated vendor's own profile.
pub async fn me(
    ctx: VendorContext,
    State(state): State<Arc<AppState>>,
) -> Result<Json<VendorUserRow>, Response> {
    let vendor = fetch_vendor(state.engine.pool(), ctx.vendor_user_id, ctx.entity_id).await.map_err(er)?
        .ok_or_else(|| er(ErpError::Unauthorized { message: "Vendor not found".into() }))?;
    Ok(Json(vendor))
}
