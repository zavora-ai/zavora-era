//! Customer-portal authentication + self-onboarding (CRM add-in).
//!
//! Mirrors `routes::staff_auth` for the `customer_users` principal (role
//! `Customer`). Adds **self-onboarding** (`register`): a prospect signs up,
//! which creates an active portal login and a CRM **lead** for the sales team to
//! qualify/convert. Sales-assisted onboarding uses the invite endpoint in
//! `routes::crm` (sets a single-use password token, like ESS invite).

use axum::extract::State;
use axum::response::{IntoResponse, Response};
use axum::Json;
use std::sync::Arc;
use uuid::Uuid;

use crate::middleware::auth::jwt_config;
use crate::middleware::customer_auth::{CustomerContext, CUSTOMER_ROLE};
use crate::AppState;
use zavora_erp_core::auth::{self, TokenPair};
use zavora_erp_core::crm::{CustomerRegisterRequest, CustomerUserRow};
use zavora_erp_core::ErpError;

fn er(e: ErpError) -> Response { super::err_response(e).into_response() }

const REFRESH_COOKIE: &str = "customer_refresh";

fn is_production() -> bool {
    std::env::var("APP_ENV").map(|v| v.eq_ignore_ascii_case("production")).unwrap_or(false)
}
fn portal_base_url() -> String {
    std::env::var("APP_BASE_URL").unwrap_or_else(|_| "http://localhost:3000".to_string())
}
fn set_refresh_cookie(token: &str, max_age_secs: i64) -> String {
    let secure = if is_production() { "; Secure" } else { "" };
    format!("{REFRESH_COOKIE}={token}; HttpOnly; SameSite=Strict; Path=/api/v1/customerportal; Max-Age={max_age_secs}{secure}")
}
fn clear_refresh_cookie() -> String {
    let secure = if is_production() { "; Secure" } else { "" };
    format!("{REFRESH_COOKIE}=; HttpOnly; SameSite=Strict; Path=/api/v1/customerportal; Max-Age=0{secure}")
}
fn read_refresh_cookie(headers: &axum::http::HeaderMap) -> Option<String> {
    let raw = headers.get(axum::http::header::COOKIE)?.to_str().ok()?;
    raw.split(';').find_map(|kv| { let (k, v) = kv.trim().split_once('=')?; (k == REFRESH_COOKIE).then(|| v.to_string()) })
}

fn auth_success(cust: &CustomerUserRow, pair: &TokenPair) -> Response {
    let max_age = (pair.refresh_expires_at - chrono::Utc::now()).num_seconds().max(0);
    let cookie = set_refresh_cookie(&pair.refresh_token, max_age);
    let body = serde_json::json!({
        "access_token": pair.access_token, "token_type": "Bearer",
        "expires_in": pair.expires_in, "customer": cust,
    });
    ([(axum::http::header::SET_COOKIE, cookie)], Json(body)).into_response()
}

async fn store_refresh(pool: &sqlx::PgPool, pair: &TokenPair, uid: Uuid, entity_id: Uuid) -> Result<(), ErpError> {
    sqlx::query("INSERT INTO refresh_tokens (jti, user_id, entity_id, expires_at) VALUES ($1,$2,$3,$4)")
        .bind(pair.refresh_jti).bind(uid).bind(entity_id).bind(pair.refresh_expires_at)
        .execute(pool).await.map_err(ErpError::Database)?;
    Ok(())
}

async fn fetch_customer(pool: &sqlx::PgPool, id: Uuid, entity_id: Uuid) -> Result<Option<CustomerUserRow>, ErpError> {
    sqlx::query_as::<_, CustomerUserRow>(
        "SELECT id, entity_id, email, display_name, status, customer_id, last_login, created_at \
         FROM customer_users WHERE id = $1 AND entity_id = $2",
    ).bind(id).bind(entity_id).fetch_optional(pool).await.map_err(ErpError::Database)
}

#[derive(serde::Deserialize)]
pub struct CustomerLoginRequest { pub email: String, pub password: String }

/// POST /api/v1/customerportal/login
pub async fn login(State(state): State<Arc<AppState>>, Json(req): Json<CustomerLoginRequest>) -> Result<Response, Response> {
    let entity_id = crate::middleware::auth::served_entity();
    let invalid = || er(ErpError::Unauthorized { message: "Invalid email or password".into() });
    let row: Option<(Uuid, Option<String>, String)> = sqlx::query_as(
        "SELECT id, password_hash, status FROM customer_users WHERE entity_id = $1 AND lower(email) = lower($2)",
    ).bind(entity_id).bind(req.email.trim()).fetch_optional(state.engine.pool()).await.map_err(|e| er(ErpError::Database(e)))?;
    let (uid, hash, status) = row.ok_or_else(invalid)?;
    let hash = hash.ok_or_else(|| er(ErpError::Unauthorized { message: "Account not activated — set your password from the invite link.".into() }))?;
    if !auth::verify_password(&req.password, &hash) { return Err(invalid()); }
    if status != "active" { return Err(er(ErpError::Unauthorized { message: "Your account is not active.".into() })); }

    let pair = auth::issue_token_pair(jwt_config(), uid, entity_id, CUSTOMER_ROLE).map_err(er)?;
    store_refresh(state.engine.pool(), &pair, uid, entity_id).await.map_err(er)?;
    let _ = sqlx::query("UPDATE customer_users SET last_login = NOW() WHERE id = $1").bind(uid).execute(state.engine.pool()).await;
    let cust = fetch_customer(state.engine.pool(), uid, entity_id).await.map_err(er)?.ok_or_else(invalid)?;
    Ok(auth_success(&cust, &pair))
}

/// POST /api/v1/customerportal/register — self-onboarding: create an active
/// portal login + a CRM lead. Only when CRM is enabled for the tenant.
pub async fn register(State(state): State<Arc<AppState>>, Json(req): Json<CustomerRegisterRequest>) -> Result<Response, Response> {
    let entity_id = crate::middleware::auth::served_entity();
    if !zavora_erp_core::services::crm::is_enabled(&state.engine, entity_id).await {
        return Err(er(ErpError::ValidationFailed { message: "Customer portal is not enabled for this workspace".into() }));
    }
    if req.password.len() < 8 {
        return Err(er(ErpError::ValidationFailed { message: "Password must be at least 8 characters".into() }));
    }
    let email = req.email.trim().to_lowercase();
    let exists: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM customer_users WHERE entity_id=$1 AND lower(email)=lower($2))")
        .bind(entity_id).bind(&email).fetch_one(state.engine.pool()).await.map_err(|e| er(ErpError::Database(e)))?;
    if exists {
        return Err(er(ErpError::ValidationFailed { message: "An account with this email already exists — sign in instead.".into() }));
    }
    let hash = auth::hash_password(&req.password).map_err(er)?;
    let uid = Uuid::new_v4();
    let mut tx = state.engine.pool().begin().await.map_err(|e| er(ErpError::Database(e)))?;
    sqlx::query(
        "INSERT INTO customer_users (id, entity_id, email, display_name, password_hash, status) VALUES ($1,$2,$3,$4,$5,'active')",
    )
    .bind(uid).bind(entity_id).bind(&email).bind(&req.display_name).bind(&hash)
    .execute(&mut *tx).await.map_err(|e| er(ErpError::Database(e)))?;
    // Create a CRM lead for the sales team.
    sqlx::query(
        "INSERT INTO crm_leads (id, entity_id, name, company, email, phone, source, status) \
         VALUES ($1,$2,$3,$4,$5,$6,'Portal Signup','New')",
    )
    .bind(Uuid::new_v4()).bind(entity_id).bind(&req.display_name).bind(&req.company).bind(&email).bind(&req.phone)
    .execute(&mut *tx).await.map_err(|e| er(ErpError::Database(e)))?;
    tx.commit().await.map_err(|e| er(ErpError::Database(e)))?;

    let pair = auth::issue_token_pair(jwt_config(), uid, entity_id, CUSTOMER_ROLE).map_err(er)?;
    store_refresh(state.engine.pool(), &pair, uid, entity_id).await.map_err(er)?;
    let cust = fetch_customer(state.engine.pool(), uid, entity_id).await.map_err(er)?
        .ok_or_else(|| er(ErpError::Internal("customer create failed".into())))?;
    Ok(auth_success(&cust, &pair))
}

/// POST /api/v1/customerportal/refresh
pub async fn refresh(State(state): State<Arc<AppState>>, headers: axum::http::HeaderMap) -> Result<Response, Response> {
    let token = read_refresh_cookie(&headers).ok_or_else(|| er(ErpError::Unauthorized { message: "Missing refresh token".into() }))?;
    let claims = auth::decode_refresh_token(jwt_config(), &token).map_err(er)?;
    if claims.role != CUSTOMER_ROLE { return Err(er(ErpError::Unauthorized { message: "Not a customer session".into() })); }
    let jti = claims.jti.ok_or_else(|| er(ErpError::Unauthorized { message: "Refresh token missing id".into() }))?;
    let valid: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM refresh_tokens WHERE jti=$1 AND revoked=false AND expires_at>NOW())")
        .bind(jti).fetch_one(state.engine.pool()).await.map_err(|e| er(ErpError::Database(e)))?;
    if !valid { return Err(er(ErpError::Unauthorized { message: "Session expired".into() })); }
    let _ = sqlx::query("UPDATE refresh_tokens SET revoked=true WHERE jti=$1").bind(jti).execute(state.engine.pool()).await;
    let pair = auth::issue_token_pair(jwt_config(), claims.sub, claims.entity_id, CUSTOMER_ROLE).map_err(er)?;
    store_refresh(state.engine.pool(), &pair, claims.sub, claims.entity_id).await.map_err(er)?;
    let cust = fetch_customer(state.engine.pool(), claims.sub, claims.entity_id).await.map_err(er)?
        .ok_or_else(|| er(ErpError::Unauthorized { message: "Customer not found".into() }))?;
    Ok(auth_success(&cust, &pair))
}

/// POST /api/v1/customerportal/logout
pub async fn logout(State(state): State<Arc<AppState>>, headers: axum::http::HeaderMap) -> Response {
    if let Some(token) = read_refresh_cookie(&headers) {
        if let Ok(claims) = auth::decode_refresh_token(jwt_config(), &token) {
            if let Some(jti) = claims.jti {
                let _ = sqlx::query("UPDATE refresh_tokens SET revoked=true WHERE jti=$1").bind(jti).execute(state.engine.pool()).await;
            }
        }
    }
    ([(axum::http::header::SET_COOKIE, clear_refresh_cookie())], Json(serde_json::json!({ "ok": true }))).into_response()
}

/// GET /api/v1/customerportal/me
pub async fn me(ctx: CustomerContext, State(state): State<Arc<AppState>>) -> Result<Json<CustomerUserRow>, Response> {
    let cust = fetch_customer(state.engine.pool(), ctx.customer_user_id, ctx.entity_id).await.map_err(er)?
        .ok_or_else(|| er(ErpError::Unauthorized { message: "Customer not found".into() }))?;
    Ok(Json(cust))
}

#[derive(serde::Deserialize)]
pub struct SetPasswordRequest { pub token: String, pub password: String }

/// POST /api/v1/customerportal/set-password — accept an assisted invite / reset.
pub async fn set_password(State(state): State<Arc<AppState>>, Json(req): Json<SetPasswordRequest>) -> Result<Response, Response> {
    if req.password.len() < 8 {
        return Err(er(ErpError::ValidationFailed { message: "Password must be at least 8 characters".into() }));
    }
    let row: Option<(Uuid,)> = sqlx::query_as("SELECT id FROM customer_users WHERE set_token=$1 AND set_token_expires>NOW()")
        .bind(req.token.trim()).fetch_optional(state.engine.pool()).await.map_err(|e| er(ErpError::Database(e)))?;
    let (id,) = row.ok_or_else(|| er(ErpError::ValidationFailed { message: "This link is invalid or has expired.".into() }))?;
    let hash = auth::hash_password(&req.password).map_err(er)?;
    sqlx::query("UPDATE customer_users SET password_hash=$1, status='active', set_token=NULL, set_token_expires=NULL WHERE id=$2")
        .bind(&hash).bind(id).execute(state.engine.pool()).await.map_err(|e| er(ErpError::Database(e)))?;
    Ok(Json(serde_json::json!({ "ok": true, "message": "Password set. You can now sign in." })).into_response())
}

#[derive(serde::Deserialize)]
pub struct ForgotPasswordRequest { pub email: String }

/// POST /api/v1/customerportal/forgot-password (no account enumeration).
pub async fn forgot_password(State(state): State<Arc<AppState>>, Json(req): Json<ForgotPasswordRequest>) -> Response {
    let entity_id = crate::middleware::auth::served_entity();
    let row: Option<(Uuid, String)> = sqlx::query_as("SELECT id, email FROM customer_users WHERE entity_id=$1 AND lower(email)=lower($2)")
        .bind(entity_id).bind(req.email.trim()).fetch_optional(state.engine.pool()).await.ok().flatten();
    if let Some((id, email)) = row {
        let token = Uuid::new_v4().to_string();
        let _ = sqlx::query("UPDATE customer_users SET set_token=$1, set_token_expires=NOW()+INTERVAL '1 hour' WHERE id=$2")
            .bind(&token).bind(id).execute(state.engine.pool()).await;
        let link = format!("{}/customerportal/set-password?token={}", portal_base_url(), token);
        let email_req = zavora_erp_core::notifications::SendNotificationRequest {
            event_type: zavora_erp_core::notifications::NotificationEventType::LeaveRequestDecided,
            channels: vec![zavora_erp_core::types::Channel::Email],
            recipients: vec![email],
            subject: Some("Reset your customer portal password".into()),
            body: format!("Reset your password here: {link}\n\nThis link expires in 1 hour."),
            related_type: Some("customer_user".into()), related_id: Some(id), schedule_at: None, attachments: Vec::new(),
        };
        let _ = zavora_erp_core::services::notifications::send_notification(&state.engine, entity_id, email_req).await;
    }
    Json(serde_json::json!({ "ok": true, "message": "If that account exists, a reset link has been sent." })).into_response()
}
