//! Platform super-admin API:
//! - Phase 0: login + tenant directory
//! - Phase 1: suspend / unsuspend + support impersonation

use axum::{
    extract::{Path, Query, State},
    http::{header, HeaderMap},
    response::{IntoResponse, Response},
    Json,
};
use serde::Deserialize;
use std::sync::Arc;
use uuid::Uuid;

use zavora_erp_core::auth::{self, TokenPair};
use zavora_erp_core::platform::{platform_entity_id, ROLE_PLATFORM_SUPER_ADMIN};
use zavora_erp_core::services::platform as svc;
use zavora_erp_core::ErpError;

use crate::middleware::auth::jwt_config;
use crate::middleware::platform_auth::PlatformAuthContext;
use crate::AppState;

const PLATFORM_REFRESH_COOKIE: &str = "era_platform_refresh";

type ApiResult = Result<Response, Response>;

fn er(e: ErpError) -> Response {
    super::err_response(e).into_response()
}

fn set_platform_refresh_cookie(pair: &TokenPair) -> String {
    let secure = std::env::var("APP_ENV").as_deref() == Ok("production");
    let max_age = jwt_config().refresh_ttl_secs;
    format!(
        "{PLATFORM_REFRESH_COOKIE}={}; HttpOnly; Path=/api/v1/platform; SameSite=Strict; Max-Age={}{}",
        pair.refresh_token,
        max_age,
        if secure { "; Secure" } else { "" }
    )
}

fn clear_platform_refresh_cookie() -> String {
    let secure = std::env::var("APP_ENV").as_deref() == Ok("production");
    format!(
        "{PLATFORM_REFRESH_COOKIE}=; HttpOnly; Path=/api/v1/platform; SameSite=Strict; Max-Age=0{}",
        if secure { "; Secure" } else { "" }
    )
}

fn read_platform_refresh_cookie(headers: &HeaderMap) -> Option<String> {
    let cookie = headers.get(header::COOKIE)?.to_str().ok()?;
    for part in cookie.split(';') {
        let part = part.trim();
        if let Some(v) = part.strip_prefix(&format!("{PLATFORM_REFRESH_COOKIE}=")) {
            return Some(v.to_string());
        }
    }
    None
}

fn auth_json(user_id: Uuid, email: &str, display_name: &str, role: &str, pair: &TokenPair) -> Response {
    let body = serde_json::json!({
        "access_token": pair.access_token,
        "expires_in": pair.expires_in,
        "user": {
            "id": user_id,
            "email": email,
            "display_name": display_name,
            "role": role,
            "plane": "platform",
        }
    });
    let mut res = Json(body).into_response();
    if let Ok(val) = set_platform_refresh_cookie(pair).parse() {
        res.headers_mut().append(header::SET_COOKIE, val);
    }
    res
}

#[derive(Debug, Deserialize)]
pub struct PlatformLoginRequest {
    pub email: String,
    pub password: String,
}

/// POST /api/v1/platform/auth/login
pub async fn login(
    State(state): State<Arc<AppState>>,
    Json(req): Json<PlatformLoginRequest>,
) -> ApiResult {
    let invalid = || {
        er(ErpError::Unauthorized {
            message: "Invalid email or password".into(),
        })
    };

    let user = svc::find_by_email(state.engine.pool(), &req.email)
        .await
        .map_err(er)?
        .ok_or_else(invalid)?;

    if !user.is_active {
        return Err(invalid());
    }
    if !auth::verify_password(&req.password, &user.password_hash) {
        return Err(invalid());
    }

    let pair = auth::issue_token_pair(
        jwt_config(),
        user.id,
        platform_entity_id(),
        &user.role,
    )
    .map_err(er)?;

    // Store refresh jti in the shared refresh_tokens table (entity_id = nil).
    sqlx::query(
        "INSERT INTO refresh_tokens (jti, user_id, entity_id, expires_at) VALUES ($1, $2, $3, $4)",
    )
    .bind(pair.refresh_jti)
    .bind(user.id)
    .bind(platform_entity_id())
    .bind(pair.refresh_expires_at)
    .execute(state.engine.pool())
    .await
    .map_err(|e| er(ErpError::Database(e)))?;

    let _ = svc::touch_login(state.engine.pool(), user.id).await;
    let _ = svc::record_audit(
        state.engine.pool(),
        user.id,
        "login",
        None,
        Some(serde_json::json!({ "email": user.email })),
    )
    .await;

    Ok(auth_json(
        user.id,
        &user.email,
        &user.display_name,
        &user.role,
        &pair,
    ))
}

/// POST /api/v1/platform/auth/refresh
pub async fn refresh(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
) -> ApiResult {
    let token = read_platform_refresh_cookie(&headers).ok_or_else(|| {
        er(ErpError::Unauthorized {
            message: "Missing platform refresh token".into(),
        })
    })?;
    let claims = auth::decode_refresh_token(jwt_config(), &token).map_err(er)?;
    if !zavora_erp_core::platform::is_platform_role(&claims.role) {
        return Err(er(ErpError::Unauthorized {
            message: "Not a platform refresh token".into(),
        }));
    }
    let jti = claims.jti.ok_or_else(|| {
        er(ErpError::Unauthorized {
            message: "Refresh token missing id".into(),
        })
    })?;

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
            message: "Refresh token is expired or revoked".into(),
        }));
    }

    let user = svc::find_by_id(state.engine.pool(), claims.sub)
        .await
        .map_err(er)?
        .ok_or_else(|| {
            er(ErpError::Unauthorized {
                message: "Platform user not found".into(),
            })
        })?;
    if !user.is_active {
        return Err(er(ErpError::Unauthorized {
            message: "Platform user is inactive".into(),
        }));
    }

    // Rotate refresh token.
    let _ = sqlx::query("UPDATE refresh_tokens SET revoked = true WHERE jti = $1")
        .bind(jti)
        .execute(state.engine.pool())
        .await;

    let pair = auth::issue_token_pair(
        jwt_config(),
        user.id,
        platform_entity_id(),
        &user.role,
    )
    .map_err(er)?;
    sqlx::query(
        "INSERT INTO refresh_tokens (jti, user_id, entity_id, expires_at) VALUES ($1, $2, $3, $4)",
    )
    .bind(pair.refresh_jti)
    .bind(user.id)
    .bind(platform_entity_id())
    .bind(pair.refresh_expires_at)
    .execute(state.engine.pool())
    .await
    .map_err(|e| er(ErpError::Database(e)))?;

    Ok(auth_json(
        user.id,
        &user.email,
        &user.display_name,
        &user.role,
        &pair,
    ))
}

/// POST /api/v1/platform/auth/logout
pub async fn logout(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
) -> ApiResult {
    if let Some(token) = read_platform_refresh_cookie(&headers) {
        if let Ok(claims) = auth::decode_refresh_token(jwt_config(), &token) {
            if let Some(jti) = claims.jti {
                let _ = sqlx::query("UPDATE refresh_tokens SET revoked = true WHERE jti = $1")
                    .bind(jti)
                    .execute(state.engine.pool())
                    .await;
            }
        }
    }
    let mut res = Json(serde_json::json!({ "ok": true })).into_response();
    if let Ok(val) = clear_platform_refresh_cookie().parse() {
        res.headers_mut().append(header::SET_COOKIE, val);
    }
    Ok(res)
}

/// GET /api/v1/platform/me
pub async fn me(
    State(state): State<Arc<AppState>>,
    ctx: PlatformAuthContext,
) -> ApiResult {
    let user = svc::find_by_id(state.engine.pool(), ctx.user_id)
        .await
        .map_err(er)?
        .ok_or_else(|| {
            er(ErpError::Unauthorized {
                message: "Platform user not found".into(),
            })
        })?;
    Ok(Json(serde_json::json!({
        "id": user.id,
        "email": user.email,
        "display_name": user.display_name,
        "role": user.role,
        "plane": "platform",
        "last_login": user.last_login,
    }))
    .into_response())
}

#[derive(Debug, Deserialize)]
pub struct ListTenantsParams {
    pub q: Option<String>,
    pub plan_status: Option<String>,
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

/// GET /api/v1/platform/tenants
pub async fn list_tenants(
    State(state): State<Arc<AppState>>,
    ctx: PlatformAuthContext,
    Query(params): Query<ListTenantsParams>,
) -> ApiResult {
    let (data, total) = svc::list_tenants(
        state.engine.pool(),
        svc::ListTenantsQuery {
            q: params.q,
            plan_status: params.plan_status,
            limit: params.limit.unwrap_or(50),
            offset: params.offset.unwrap_or(0),
        },
    )
    .await
    .map_err(er)?;

    let _ = svc::record_audit(
        state.engine.pool(),
        ctx.user_id,
        "list_tenants",
        None,
        Some(serde_json::json!({ "total": total })),
    )
    .await;

    Ok(Json(serde_json::json!({
        "data": data,
        "total_count": total,
    }))
    .into_response())
}

/// GET /api/v1/platform/tenants/{entity_id}
pub async fn get_tenant(
    State(state): State<Arc<AppState>>,
    ctx: PlatformAuthContext,
    Path(entity_id): Path<Uuid>,
) -> ApiResult {
    let tenant = svc::get_tenant(state.engine.pool(), entity_id)
        .await
        .map_err(er)?
        .ok_or_else(|| {
            er(ErpError::NotFound {
                entity_type: "Tenant".into(),
                id: entity_id,
            })
        })?;

    let _ = svc::record_audit(
        state.engine.pool(),
        ctx.user_id,
        "get_tenant",
        Some(entity_id),
        None,
    )
    .await;

    Ok(Json(serde_json::json!({ "data": tenant })).into_response())
}

#[derive(Debug, Deserialize)]
pub struct SuspendRequest {
    pub reason: Option<String>,
}

/// POST /api/v1/platform/tenants/{entity_id}/suspend
pub async fn suspend_tenant(
    State(state): State<Arc<AppState>>,
    ctx: PlatformAuthContext,
    Path(entity_id): Path<Uuid>,
    body: Option<Json<SuspendRequest>>,
) -> ApiResult {
    let reason = body.and_then(|Json(b)| b.reason);
    let tenant = svc::suspend_tenant(
        state.engine.pool(),
        entity_id,
        reason.as_deref(),
    )
    .await
    .map_err(er)?;

    let _ = svc::record_audit(
        state.engine.pool(),
        ctx.user_id,
        "suspend_tenant",
        Some(entity_id),
        Some(serde_json::json!({
            "reason": reason,
            "organization_name": tenant.organization_name,
        })),
    )
    .await;

    Ok(Json(serde_json::json!({ "data": tenant })).into_response())
}

/// POST /api/v1/platform/tenants/{entity_id}/unsuspend
pub async fn unsuspend_tenant(
    State(state): State<Arc<AppState>>,
    ctx: PlatformAuthContext,
    Path(entity_id): Path<Uuid>,
) -> ApiResult {
    let tenant = svc::unsuspend_tenant(state.engine.pool(), entity_id)
        .await
        .map_err(er)?;

    let _ = svc::record_audit(
        state.engine.pool(),
        ctx.user_id,
        "unsuspend_tenant",
        Some(entity_id),
        Some(serde_json::json!({
            "organization_name": tenant.organization_name,
        })),
    )
    .await;

    Ok(Json(serde_json::json!({ "data": tenant })).into_response())
}

/// POST /api/v1/platform/tenants/{entity_id}/impersonate
///
/// Issues a short-lived tenant session as the primary Owner (or first active
/// user). The JWT carries `impersonator_id` so the UI can show a support banner.
/// Allowed even when the tenant is suspended so ops can still diagnose issues.
pub async fn impersonate_tenant(
    State(state): State<Arc<AppState>>,
    ctx: PlatformAuthContext,
    Path(entity_id): Path<Uuid>,
) -> ApiResult {
    // Confirm tenant exists (and refresh counts).
    let tenant = svc::get_tenant(state.engine.pool(), entity_id)
        .await
        .map_err(er)?
        .ok_or_else(|| {
            er(ErpError::NotFound {
                entity_type: "Tenant".into(),
                id: entity_id,
            })
        })?;

    let target = svc::pick_impersonation_target(state.engine.pool(), entity_id)
        .await
        .map_err(er)?;

    let pair = auth::issue_impersonation_token_pair(
        jwt_config(),
        target.id,
        target.entity_id,
        &target.role,
        ctx.user_id,
    )
    .map_err(er)?;

    sqlx::query(
        "INSERT INTO refresh_tokens (jti, user_id, entity_id, expires_at) VALUES ($1, $2, $3, $4)",
    )
    .bind(pair.refresh_jti)
    .bind(target.id)
    .bind(target.entity_id)
    .bind(pair.refresh_expires_at)
    .execute(state.engine.pool())
    .await
    .map_err(|e| er(ErpError::Database(e)))?;

    let _ = svc::record_audit(
        state.engine.pool(),
        ctx.user_id,
        "impersonate_tenant",
        Some(entity_id),
        Some(serde_json::json!({
            "target_user_id": target.id,
            "target_email": target.email,
            "target_role": target.role,
            "organization_name": tenant.organization_name,
            "expires_in": pair.expires_in,
        })),
    )
    .await;

    // Deliver tenant refresh cookie so the ERP app can bootstrap like a normal login.
    let max_age = (pair.refresh_expires_at - chrono::Utc::now())
        .num_seconds()
        .max(0);
    let secure = std::env::var("APP_ENV").as_deref() == Ok("production");
    let cookie = format!(
        "era_refresh={}; HttpOnly; SameSite=Strict; Path=/api/v1/auth; Max-Age={}{}",
        pair.refresh_token,
        max_age,
        if secure { "; Secure" } else { "" }
    );

    let body = serde_json::json!({
        "access_token": pair.access_token,
        "token_type": "Bearer",
        "expires_in": pair.expires_in,
        "impersonation": true,
        "tenant": {
            "entity_id": tenant.entity_id,
            "organization_name": tenant.organization_name,
            "suspended": tenant.suspended,
        },
        "user": {
            "user_id": target.id,
            "entity_id": target.entity_id,
            "role": target.role,
            "display_name": target.display_name,
            "email": target.email,
            "impersonated_by": ctx.user_id,
            "support_session": true,
        }
    });

    let mut res = Json(body).into_response();
    if let Ok(val) = cookie.parse() {
        res.headers_mut().append(header::SET_COOKIE, val);
    }
    Ok(res)
}

// Silence unused import if ROLE only used as doc reference in future.
#[allow(dead_code)]
fn _role_const() -> &'static str {
    ROLE_PLATFORM_SUPER_ADMIN
}
