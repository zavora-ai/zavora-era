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
    /// `true` when the tenant has been soft-archived (closed). Archived tenants
    /// are hidden from the switcher by default and cannot be switched into.
    archived: bool,
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

/// Every active membership for `email`, newest-named first. The `archived` flag
/// reflects whether each tenant has been soft-archived; callers decide whether
/// to include or hide archived tenants.
async fn memberships(state: &AppState, email: &str) -> Result<Vec<MembershipRow>, Response> {
    sqlx::query_as::<_, MembershipRow>(
        r#"SELECT u.id AS user_id,
                  u.entity_id,
                  u.role,
                  u.display_name,
                  u.email,
                  COALESCE(s.organization_name, '(unnamed)') AS name,
                  COALESCE(s.base_currency, 'KES') AS currency,
                  (s.archived_at IS NOT NULL) AS archived
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
///
/// Archived (closed) tenants are hidden by default; pass `?include_archived=true`
/// to include them (each carries an `archived` flag) so the UI can offer a
/// "restore" affordance.
pub async fn list_tenants(
    ctx: AuthContext,
    State(state): State<Arc<AppState>>,
    axum::extract::Query(q): axum::extract::Query<ListTenantsQuery>,
) -> Result<Json<serde_json::Value>, Response> {
    let email = caller_email(&state, ctx.user_id).await?;
    let rows = memberships(&state, &email).await?;
    let items: Vec<_> = rows
        .iter()
        .filter(|m| q.include_archived || !m.archived)
        .map(|m| {
            serde_json::json!({
                "entity_id": m.entity_id,
                "name": m.name,
                "currency": m.currency,
                "role": m.role,
                "current": m.entity_id == ctx.entity_id,
                "archived": m.archived,
            })
        })
        .collect();
    Ok(Json(serde_json::json!({ "tenants": items })))
}

#[derive(serde::Deserialize, Default)]
pub struct ListTenantsQuery {
    /// When true, archived tenants are included in the response.
    #[serde(default)]
    pub include_archived: bool,
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

    // An archived (closed) tenant must be restored before it can be entered.
    if target.archived {
        return Err(er(ErpError::ValidationFailed {
            message: "That tenant is archived; restore it before switching in".to_string(),
        }));
    }

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
    /// Opt-in: seed a sample company (customers/vendors/products/invoices) into
    /// the new tenant so it has data to explore.
    #[serde(default)]
    pub with_sample_data: bool,
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

    // Optional sample-company seed (best-effort; never fails tenant creation).
    if req.with_sample_data {
        match zavora_erp_core::services::sample_data::seed_sample_company(
            &state.engine,
            provisioned.entity_id,
        )
        .await
        {
            Ok(summary) => tracing::info!(entity_id = %provisioned.entity_id, "seeded sample company: {summary:?}"),
            Err(e) => tracing::warn!(entity_id = %provisioned.entity_id, "sample company seed failed (continuing): {e}"),
        }
    }

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
        archived: false,
    };
    Ok(session_response(&m, &pair))
}


// ---------------------------------------------------------------------------
// Tenant lifecycle: archive (close) / unarchive (restore) / leave.
//
// A hard delete is intentionally not offered: the immutability triggers block
// deleting posted journal lines and the ledger/audit trail is retained for
// compliance. Archiving is the reversible, audit-preserving way for a user to
// remove a tenant from their active workspace.
// ---------------------------------------------------------------------------

/// Look up the caller's membership of a specific tenant by their email, so role
/// checks are evaluated against the *target* tenant (not the session's current
/// one). Returns `None` when the caller is not a member.
async fn membership_of(
    state: &AppState,
    email: &str,
    entity_id: Uuid,
) -> Result<Option<MembershipRow>, Response> {
    let rows = memberships(state, email).await?;
    Ok(rows.into_iter().find(|m| m.entity_id == entity_id))
}

/// Record a tenant-lifecycle audit event (archived / unarchived / left).
async fn audit_tenant_event(
    state: &AppState,
    entity_id: Uuid,
    actor_user_id: Uuid,
    event_type: &str,
) {
    let actor = serde_json::json!({ "type": "user", "user_id": actor_user_id });
    let metadata = serde_json::json!({ "by": actor_user_id, "at": chrono::Utc::now() });
    let _ = sqlx::query(
        r#"INSERT INTO audit_events
               (entity_id, event_type, object_type, object_id, actor, metadata, timestamp)
           VALUES ($1, $2, 'tenant', $3, $4, $5, NOW())"#,
    )
    .bind(entity_id)
    .bind(event_type)
    .bind(entity_id)
    .bind(actor)
    .bind(metadata)
    .execute(state.engine.pool())
    .await;
}

/// POST /api/v1/auth/tenants/{id}/archive — soft-archive (close) a tenant the
/// caller Owns. Owner-only. Refuses to archive the caller's only non-archived
/// tenant (they would be left with no active workspace) — they should create or
/// switch to another tenant first. The ledger and audit trail are untouched;
/// the tenant simply disappears from the switcher until restored.
pub async fn archive_tenant(
    ctx: AuthContext,
    State(state): State<Arc<AppState>>,
    axum::extract::Path(entity_id): axum::extract::Path<Uuid>,
) -> Result<Json<serde_json::Value>, Response> {
    let email = caller_email(&state, ctx.user_id).await?;
    let all = memberships(&state, &email).await?;

    let Some(target) = all.iter().find(|m| m.entity_id == entity_id) else {
        return Err(er(ErpError::Unauthorized {
            message: "You are not a member of that tenant".to_string(),
        }));
    };

    // Only an Owner of the target tenant may archive it.
    if target.role != "Owner" {
        return Err(er(ErpError::PermissionDenied {
            action: "archive a tenant".to_string(),
            required_role: "Owner".to_string(),
        }));
    }

    // Already archived → idempotent success.
    if target.archived {
        return Ok(Json(serde_json::json!({ "entity_id": entity_id, "archived": true })));
    }

    // Refuse to archive the caller's last remaining active workspace.
    let active_count = all.iter().filter(|m| !m.archived).count();
    if active_count <= 1 {
        return Err(er(ErpError::ValidationFailed {
            message: "Cannot archive your only active tenant; create or switch to another first"
                .to_string(),
        }));
    }

    sqlx::query(
        "UPDATE entity_settings SET archived_at = NOW(), archived_by = $2 WHERE entity_id = $1",
    )
    .bind(entity_id)
    .bind(ctx.user_id)
    .execute(state.engine.pool())
    .await
    .map_err(|e| er(ErpError::Database(e)))?;

    audit_tenant_event(&state, entity_id, ctx.user_id, "archived").await;

    Ok(Json(serde_json::json!({ "entity_id": entity_id, "archived": true })))
}

/// POST /api/v1/auth/tenants/{id}/unarchive — restore a previously archived
/// tenant. Owner-only. After restoring, the caller can switch into it again.
pub async fn unarchive_tenant(
    ctx: AuthContext,
    State(state): State<Arc<AppState>>,
    axum::extract::Path(entity_id): axum::extract::Path<Uuid>,
) -> Result<Json<serde_json::Value>, Response> {
    let email = caller_email(&state, ctx.user_id).await?;

    let Some(target) = membership_of(&state, &email, entity_id).await? else {
        return Err(er(ErpError::Unauthorized {
            message: "You are not a member of that tenant".to_string(),
        }));
    };

    if target.role != "Owner" {
        return Err(er(ErpError::PermissionDenied {
            action: "restore a tenant".to_string(),
            required_role: "Owner".to_string(),
        }));
    }

    sqlx::query(
        "UPDATE entity_settings SET archived_at = NULL, archived_by = NULL WHERE entity_id = $1",
    )
    .bind(entity_id)
    .execute(state.engine.pool())
    .await
    .map_err(|e| er(ErpError::Database(e)))?;

    audit_tenant_event(&state, entity_id, ctx.user_id, "unarchived").await;

    Ok(Json(serde_json::json!({ "entity_id": entity_id, "archived": false })))
}

/// POST /api/v1/auth/tenants/{id}/leave — the caller leaves a tenant by
/// deactivating their own membership (era_users row) in it. This removes the
/// tenant from their workspace without affecting anyone else.
///
/// A sole active Owner cannot leave (the tenant would be left ownerless) — they
/// must hand ownership to another user first, or archive the tenant instead.
/// The caller also cannot leave the tenant they are currently signed into via
/// this call's own session entity, to avoid orphaning the live session; they
/// should switch away first.
pub async fn leave_tenant(
    ctx: AuthContext,
    State(state): State<Arc<AppState>>,
    axum::extract::Path(entity_id): axum::extract::Path<Uuid>,
) -> Result<Json<serde_json::Value>, Response> {
    let email = caller_email(&state, ctx.user_id).await?;

    let Some(target) = membership_of(&state, &email, entity_id).await? else {
        return Err(er(ErpError::Unauthorized {
            message: "You are not a member of that tenant".to_string(),
        }));
    };

    // Sole-Owner protection: mirror the first-Owner rule in users.rs.
    if target.role == "Owner" {
        let active_owners = sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM era_users \
             WHERE entity_id = $1 AND role = 'Owner' AND is_active = true",
        )
        .bind(entity_id)
        .fetch_one(state.engine.pool())
        .await
        .map_err(|e| er(ErpError::Database(e)))?;

        if active_owners <= 1 {
            return Err(er(ErpError::ValidationFailed {
                message: "You are the sole Owner; transfer ownership or archive the tenant instead"
                    .to_string(),
            }));
        }
    }

    // Deactivate only the caller's own membership in the target tenant.
    sqlx::query(
        "UPDATE era_users SET is_active = false WHERE id = $1 AND entity_id = $2",
    )
    .bind(target.user_id)
    .bind(entity_id)
    .execute(state.engine.pool())
    .await
    .map_err(|e| er(ErpError::Database(e)))?;

    audit_tenant_event(&state, entity_id, ctx.user_id, "member_left").await;

    Ok(Json(serde_json::json!({ "entity_id": entity_id, "left": true })))
}
