//! JWT authentication middleware for Zavora ERP API (Requirement 1 & 3).
//!
//! Every authenticated request must carry a valid `Authorization: Bearer <jwt>`
//! access token. Identity (`user_id`, `entity_id`, `role`) is taken from the
//! verified token claims — the legacy `X-User-*` headers are ignored entirely,
//! so a browser cannot spoof identity.
//!
//! Tenant scope: identity (`user_id`, `entity_id`, `role`) comes from the verified
//! token claims, and the verified `entity_id` claim is the authoritative per-request
//! tenant scope. `served_entity()` is retained only for the legacy `register`
//! bootstrap path.

use std::sync::OnceLock;

use axum::{
    extract::FromRequestParts,
    http::{request::Parts, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use uuid::Uuid;

use zavora_erp_core::auth::{self, JwtConfig};
use zavora_erp_core::rbac::UserRole;
use zavora_erp_core::ErpError;

/// Process-global JWT signing configuration, set once at startup.
static JWT_CONFIG: OnceLock<JwtConfig> = OnceLock::new();
/// The single entity this process serves (from startup config).
static SERVED_ENTITY: OnceLock<Uuid> = OnceLock::new();

/// Initialise the auth layer. Must be called once before serving requests.
pub fn init_auth(config: JwtConfig, served_entity: Uuid) {
    let _ = JWT_CONFIG.set(config);
    let _ = SERVED_ENTITY.set(served_entity);
}

/// The active JWT configuration. Panics if `init_auth` was not called.
pub fn jwt_config() -> &'static JwtConfig {
    JWT_CONFIG.get().expect("auth layer not initialised")
}

/// The entity this process serves.
pub fn served_entity() -> Uuid {
    *SERVED_ENTITY.get().expect("auth layer not initialised")
}

fn unauthorized(message: &str) -> Response {
    (
        StatusCode::UNAUTHORIZED,
        Json(serde_json::json!({ "error": message })),
    )
        .into_response()
}

/// Authentication context extracted from the verified JWT.
#[derive(Debug, Clone)]
pub struct AuthContext {
    pub user_id: Uuid,
    pub entity_id: Uuid,
    /// Coarse enum view of the role, retained for convenience/back-compat. A
    /// **custom** tenant role has no enum variant and maps to `Viewer` here — but
    /// this field is NOT used for authorization. All access decisions go through
    /// `role_key` + the granular permission registry (`authz_layer`), so custom
    /// roles are enforced correctly regardless of this coarse view.
    pub role: UserRole,
    /// The raw role KEY from the token (system role name or custom-role slug).
    /// The authoritative identifier for authorization (`require_permission`).
    pub role_key: String,
}

/// External principal roles that must never be accepted by the back-office auth
/// layer (they have their own portals). Barring these explicitly lets us accept
/// arbitrary *tenant* role keys (custom roles) without letting a portal token in.
const EXTERNAL_PRINCIPAL_ROLES: &[&str] = &["Customer", "Vendor", "Employee"];

/// Verify an `Authorization: Bearer <jwt>` access token from request headers and
/// build the `AuthContext`. Shared by the global middleware and the extractor.
pub fn verify_bearer(headers: &axum::http::HeaderMap) -> Result<AuthContext, Response> {
    let token = headers
        .get(axum::http::header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.strip_prefix("Bearer ").or_else(|| s.strip_prefix("bearer ")))
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .ok_or_else(|| unauthorized("Missing or malformed Authorization bearer token"))?;

    let claims = auth::decode_access_token(jwt_config(), token)
        .map_err(|e| unauthorized(&e.to_string()))?;

    // Bar external-portal principals from the back-office entirely.
    if EXTERNAL_PRINCIPAL_ROLES.iter().any(|r| r.eq_ignore_ascii_case(&claims.role)) {
        return Err(unauthorized("This endpoint is not available to portal accounts"));
    }
    // System roles map to their enum; a custom tenant role falls back to Viewer
    // for the legacy `require_role` path. `role_key` carries the true role.
    let role = parse_role(&claims.role).unwrap_or(UserRole::Viewer);

    Ok(AuthContext {
        user_id: claims.sub,
        entity_id: claims.entity_id,
        role,
        role_key: claims.role.clone(),
    })
}

/// Global authentication gate. Applied to every protected route so that no
/// endpoint can be reached without a valid access token, regardless of whether
/// its handler extracts `AuthContext`. The verified context is stashed in the
/// request extensions for handlers that need it (see the extractor below).
pub async fn require_authenticated(
    mut req: axum::extract::Request,
    next: axum::middleware::Next,
) -> Result<Response, Response> {
    let ctx = verify_bearer(req.headers())?;
    req.extensions_mut().insert(ctx);
    Ok(next.run(req).await)
}

/// Extract `AuthContext` — populated by `require_authenticated`. If it is absent,
/// the route was not placed behind the auth middleware (a wiring bug); fail closed.
impl<S> FromRequestParts<S> for AuthContext
where
    S: Send + Sync,
{
    type Rejection = Response;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        parts
            .extensions
            .get::<AuthContext>()
            .cloned()
            .ok_or_else(|| unauthorized("Not authenticated"))
    }
}

/// Data-driven authorization: check whether the caller's role grants `perm`
/// (a permission key like `journal.post`). Resolves the role→permission set via
/// the process `PermissionCache` (loaded from the DB on miss). This is the
/// forward path that supersedes the hard-coded `require_role` groups; the seeded
/// system roles reproduce those groups exactly (see the golden test).
pub async fn require_permission(
    state: &crate::AppState,
    ctx: &AuthContext,
    perm: &str,
) -> Result<(), ErpError> {
    let granted = state
        .permissions
        .has(&state.engine, ctx.entity_id, &ctx.role_key, perm)
        .await?;
    if granted {
        Ok(())
    } else {
        Err(ErpError::PermissionDenied {
            action: perm.to_string(),
            required_role: format!("the '{perm}' permission"),
        })
    }
}

/// Convert a role header string to a `UserRole`.
fn parse_role(s: &str) -> Option<UserRole> {
    match s.to_lowercase().as_str() {
        "owner" => Some(UserRole::Owner),
        "admin" => Some(UserRole::Admin),
        "accountant" => Some(UserRole::Accountant),
        "hrmanager" => Some(UserRole::HrManager),
        "editor" => Some(UserRole::Editor),
        "approver" => Some(UserRole::Approver),
        "viewer" => Some(UserRole::Viewer),
        _ => None,
    }
}

// (legacy role-group constants and role_name removed — enforcement is granular
// and centralized in middleware::authz_layer.)



// (legacy role-group constants and role_name removed — enforcement is granular
// and centralized in middleware::authz_layer.)



// ─── Golden test: data-driven RBAC seed reproduces the legacy role groups ────

#[cfg(test)]
mod rbac_seed_golden {
    use std::collections::HashSet;
    use zavora_erp_core::rbac::{permission_catalog, seeded_permissions_for, UserRole};

    fn perms(role: UserRole) -> HashSet<String> {
        seeded_permissions_for(&role)
    }
    fn has(role: UserRole, key: &str) -> bool {
        perms(role).contains(key)
    }

    /// The catalog is granular (resource×action) — sanity-check its size and that
    /// every key is unique and well-formed (`resource.verb`).
    #[test]
    fn catalog_is_granular_and_wellformed() {
        let cat = permission_catalog();
        assert!(cat.len() >= 120, "expected a granular catalog (≥120 perms), found {}", cat.len());
        let mut seen = HashSet::new();
        for p in &cat {
            assert!(p.key.contains('.'), "key `{}` must be resource.verb", p.key);
            assert!(seen.insert(p.key.clone()), "duplicate permission key `{}`", p.key);
        }
    }

    /// Owner/Admin hold EVERY catalog permission.
    #[test]
    fn owner_admin_have_everything() {
        let all: HashSet<String> = permission_catalog().into_iter().map(|p| p.key).collect();
        for role in [UserRole::Owner, UserRole::Admin] {
            assert_eq!(perms(role), all, "{role:?} must hold every permission");
        }
    }

    /// Viewer is read-only and blind to sensitive (payroll/HR/admin) data.
    #[test]
    fn viewer_is_read_only_and_non_sensitive() {
        for k in perms(UserRole::Viewer) {
            assert!(k.ends_with(".read") || k.ends_with(".export"), "Viewer holds non-read `{k}`");
        }
        for k in ["pay_run.read", "employee.read", "payroll_config.read", "audit.read", "user.read", "role.read", "settings.read"] {
            assert!(!has(UserRole::Viewer, k), "Viewer must NOT have `{k}`");
        }
        assert!(has(UserRole::Viewer, "invoice.read"), "Viewer should read invoices");
    }

    /// SoD: Editor creates/edits but never posts, approves, deletes or configures.
    #[test]
    fn editor_has_no_post_approve_delete() {
        for k in perms(UserRole::Editor) {
            for verb in [".post", ".approve", ".delete", ".void", ".reverse", ".pay", ".config", ".manage"] {
                assert!(!k.ends_with(verb), "Editor must not hold `{k}` (SoD)");
            }
        }
        assert!(has(UserRole::Editor, "invoice.create") && has(UserRole::Editor, "invoice.update"));
    }

    /// SoD: Approver only authorizes — no create/post.
    #[test]
    fn approver_only_approves() {
        assert!(has(UserRole::Approver, "bill.approve") && has(UserRole::Approver, "pay_run.approve"));
        for k in perms(UserRole::Approver) {
            assert!(!k.ends_with(".create") && !k.ends_with(".post"), "Approver must not hold `{k}` (SoD)");
        }
    }

    /// SoD: Accountant posts the books but cannot approve, nor administer users/HR config.
    #[test]
    fn accountant_posts_not_approves_or_admins() {
        for k in ["invoice.create", "invoice.post", "invoice.void", "journal.post", "journal.reverse", "bill.post", "period.close", "pay_run.post", "pay_run.pay", "employee.read", "pay_run.read"] {
            assert!(has(UserRole::Accountant, k), "Accountant must hold `{k}`");
        }
        for k in ["bill.approve", "pay_run.approve", "user.manage", "role.create", "payroll_config.config", "settings.config"] {
            assert!(!has(UserRole::Accountant, k), "Accountant must NOT hold `{k}` (SoD)");
        }
    }

    /// SoD: HrManager owns HR/payroll only — no finance/GL/admin.
    #[test]
    fn hr_manager_hr_only() {
        for k in ["employee.read", "employee.update", "pay_run.create", "pay_run.post", "payroll_config.config", "leave.approve", "onboarding.create"] {
            assert!(has(UserRole::HrManager, k), "HrManager must hold `{k}`");
        }
        for k in ["invoice.create", "journal.post", "bill.approve", "user.manage", "role.create"] {
            assert!(!has(UserRole::HrManager, k), "HrManager must NOT hold `{k}`");
        }
    }
}
