//! In-app tenant management for an authenticated user:
//!   * `GET  /api/v1/auth/tenants`        — list the tenants this user belongs to
//!   * `POST /api/v1/auth/switch-tenant`  — re-issue a session scoped to another
//!                                          of the user's tenants
//!   * `POST /api/v1/auth/tenants`        — create a brand-new tenant for this
//!                                          user (Owner), reusing their password
//!                                          hash, and switch into it
//!
//! Membership is email-based: a person (by email) may have one `era_users` row
//! per entity. Switching simply finds the user's row in the target entity and
//! issues a fresh token pair bound to that `entity_id`; creating provisions a
//! new tenant with the same email + the user's existing password hash so they
//! can later log in to it directly too.

use axum::extract::State;
use axum::response::{IntoResponse, Response};
use axum::Json;
use std::sync::Arc;
use uuid::Uuid;

use crate::middleware::auth::{jwt_config, AuthContext};
use crate::AppState;
use zavora_erp_core::auth::{self, TokenPair};
use zavora_erp_core::tenant::{self, ProvisionTenantWithHash};
use zavora_erp_core::ErpError;

const REFRESH_COOKIE: &str = "era_refresh";

fn er(e: ErpError) -> Response {
    super::err_response(e).into_response()
}

fn is_production() -> bool {
    std::env::var("APP_ENV")
        .map(|v| v.eq_ignore_ascii_case("production"))
        .unwrap_or(false)
}

fn set_refresh_cookie(token: &str, max_age_secs: i64) -> String {
    let secure = if is_production() { "; Secure" } else { "" };
    format!(
        "{REFRESH_COOKIE}={token}; HttpOnly; SameSite=Strict; Path=/api/v1/auth; Max-Age={max_age_secs}{secure}"
    )
}

/// One tenant the user belongs to.
#[derive(sqlx::FromRow)]
struct MembershipRow {
    user_id: Uuid,
    entity_id: Uuid,
    role: String,
    display_name: String,
    email: String,
    name: String,
    currency: String,
}

/// Resolve the caller's email from their user id (AuthContext carries no email).
async fn caller_email(state: &AppState, user_id: Uuid) -> Result<String, Response> {
    sqlx::query_scalar::<_, String>("SELECT email FROM era_users WHERE id = $1")
        .bind(user_id)
        .fetch_optional(state.engine.pool())
        .await
        .map_err(|e| er(ErpError::Database(e)))?
        .ok_or_else(|| er(ErpError::Unauthorized { message: "User not found".to_string() }))
}

/// Every active membership for `email`, newest-named first.
async fn memberships(state: &AppState, email: &str) -> Result<Vec<MembershipRow>, Response> {
    sqlx::query_as::<_, MembershipRow>(
        r#"SELECT u.id AS user_id,
                  u.entity_id,
                  u.role,
                  u.display_name,
                  u.email,
                  COALESCE(s.organization_name, '(unnamed)') AS name,
                  COALESCE(s.base_currency, 'KES') AS currency
           FROM era_users u
           LEFT JOIN entity_settings s ON s.entity_id = u.entity_id
           WHERE lower(u.email) = lower($1) AND u.is_active = true
           ORDER BY name"#,
    )
    .bind(email)
    .fetch_all(state.engine.pool())
    .await
    .map_err(|e| er(ErpError::Database(e)))
}

/// Persist a freshly issued refresh token so it can later be revoked.
async fn store_refresh_token(
    pool: &sqlx::PgPool,
    pair: &TokenPair,
    user_id: Uuid,
    entity_id: Uuid,
) -> Result<(), Response> {
    sqlx::query(
        "INSERT INTO refresh_tokens (jti, user_id, entity_id, expires_at) VALUES ($1, $2, $3, $4)",
    )
    .bind(pair.refresh_jti)
    .bind(user_id)
    .bind(entity_id)
    .bind(pair.refresh_expires_at)
    .execute(pool)
    .await
    .map(|_| ())
    .map_err(|e| er(ErpError::Database(e)))
}

/// Build the session response: access token + identity in the body, refresh
/// token only in the httpOnly cookie — identical shape to login/signup.
fn session_response(m: &MembershipRow, pair: &TokenPair) -> Response {
    let max_age = (pair.refresh_expires_at - chrono::Utc::now()).num_seconds().max(0);
    let cookie = set_refresh_cookie(&pair.refresh_token, max_age);
    (
        [(axum::http::header::SET_COOKIE, cookie)],
        Json(serde_json::json!({
            "access_token": pair.access_token,
            "token_type": "Bearer",
            "expires_in": pair.expires_in,
            "user": {
                "user_id": m.user_id,
                "entity_id": m.entity_id,
                "role": m.role,
                "display_name": m.display_name,
                "email": m.email,
            }
        })),
    )
        .into_response()
}

/// GET /api/v1/auth/tenants — the tenants the current user belongs to, with the
/// active one flagged. Drives the in-app tenant switcher.
pub async fn list_tenants(
    ctx: AuthContext,
    State(state): State<Arc<AppState>>,
) -> Result<Json<serde_json::Value>, Response> {
    let email = caller_email(&state, ctx.user_id).await?;
    let rows = memberships(&state, &email).await?;
    let items: Vec<_> = rows
        .iter()
        .map(|m| {
            serde_json::json!({
                "entity_id": m.entity_id,
                "name": m.name,
                "currency": m.currency,
                "role": m.role,
                "current": m.entity_id == ctx.entity_id,
            })
        })
        .collect();
    Ok(Json(serde_json::json!({ "tenants": items })))
}

#[derive(serde::Deserialize)]
pub struct SwitchTenantRequest {
    pub entity_id: Uuid,
}

/// POST /api/v1/auth/switch-tenant — re-issue a session scoped to another tenant
/// the caller is a member of. Verifies membership by the caller's email (so a
/// user can never switch into a tenant they don't belong to), then issues a
/// fresh token pair bound to that entity and rotates the refresh cookie.
pub async fn switch_tenant(
    ctx: AuthContext,
    State(state): State<Arc<AppState>>,
    Json(req): Json<SwitchTenantRequest>,
) -> Result<Response, Response> {
    let email = caller_email(&state, ctx.user_id).await?;
    let rows = memberships(&state, &email).await?;

    let Some(target) = rows.into_iter().find(|m| m.entity_id == req.entity_id) else {
        // Not a member — do not reveal whether the tenant exists.
        return Err(er(ErpError::Unauthorized {
            message: "You are not a member of that tenant".to_string(),
        }));
    };

    let pair = auth::issue_token_pair(jwt_config(), target.user_id, target.entity_id, &target.role)
        .map_err(er)?;
    store_refresh_token(state.engine.pool(), &pair, target.user_id, target.entity_id).await?;

    let _ = sqlx::query("UPDATE era_users SET last_login = NOW() WHERE id = $1")
        .bind(target.user_id)
        .execute(state.engine.pool())
        .await;

    Ok(session_response(&target, &pair))
}

#[derive(serde::Deserialize)]
pub struct CreateTenantRequest {
    pub organization_name: String,
    pub organization_type: String,
    #[serde(default)]
    pub kra_pin: Option<String>,
}

/// POST /api/v1/auth/tenants — create a new tenant owned by the current user and
/// switch into it. The new Owner reuses the caller's email, display name and
/// existing password hash (so they can log in to the new tenant directly too).
pub async fn create_tenant(
    ctx: AuthContext,
    State(state): State<Arc<AppState>>,
    Json(req): Json<CreateTenantRequest>,
) -> Result<Response, Response> {
    // Validate the org fields (reuse the signup validator by faking a password
    // long enough to pass; we then discard it and use the real hash instead).
    let org_name = req.organization_name.trim();
    let org_type = req.organization_type.trim();
    if org_name.is_empty() {
        return Err(er(ErpError::ValidationFailed { message: "organization_name must not be empty".to_string() }));
    }
    if org_type.is_empty() {
        return Err(er(ErpError::ValidationFailed { message: "organization_type must not be empty".to_string() }));
    }
    let kra_pin = req.kra_pin.as_ref().map(|p| p.trim().to_uppercase()).filter(|p| !p.is_empty());

    // Pull the caller's identity + existing password hash.
    let (email, display_name, password_hash): (String, String, Option<String>) = sqlx::query_as(
        "SELECT email, display_name, password_hash FROM era_users WHERE id = $1",
    )
    .bind(ctx.user_id)
    .fetch_optional(state.engine.pool())
    .await
    .map_err(|e| er(ErpError::Database(e)))?
    .ok_or_else(|| er(ErpError::Unauthorized { message: "User not found".to_string() }))?;

    let Some(password_hash) = password_hash else {
        return Err(er(ErpError::ValidationFailed {
            message: "Your account has no password set; cannot create a tenant".to_string(),
        }));
    };

    let provisioned = tenant::provision_tenant_with_hash(
        state.engine.pool(),
        ProvisionTenantWithHash {
            organization_name: org_name.to_string(),
            organization_type: org_type.to_string(),
            kra_pin,
            owner_email: email.clone(),
            owner_display_name: display_name.clone(),
            owner_password_hash: password_hash,
            seed_chart_of_accounts: true,
        },
    )
    .await
    .map_err(er)?;

    // Issue a session for the new tenant and switch into it.
    let pair = auth::issue_token_pair(
        jwt_config(),
        provisioned.owner_user_id,
        provisioned.entity_id,
        &provisioned.role,
    )
    .map_err(er)?;
    store_refresh_token(state.engine.pool(), &pair, provisioned.owner_user_id, provisioned.entity_id).await?;

    let m = MembershipRow {
        user_id: provisioned.owner_user_id,
        entity_id: provisioned.entity_id,
        role: provisioned.role.clone(),
        display_name,
        email,
        name: provisioned.organization_name.clone(),
        currency: "KES".to_string(),
    };
    Ok(session_response(&m, &pair))
}
