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
    pub role: UserRole,
}

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

    let role = parse_role(&claims.role)
        .ok_or_else(|| unauthorized("Token carries an unrecognised role"))?;

    // Per-request tenant scope (Req 4.1–4.4, 5.1): the verified `entity_id` claim is
    // the authoritative scope. The token's signature, type, and expiry have already
    // been checked by `decode_access_token` (Req 5.4). The legacy single-tenant gate
    // (`claims.entity_id != served_entity()`) is intentionally removed so tokens for
    // any tenant verify; `served_entity()` is retained only for the legacy `register`
    // bootstrap path (Req 9.1, 9.4).
    Ok(AuthContext {
        user_id: claims.sub,
        entity_id: claims.entity_id,
        role,
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

/// Check whether the user's role is in the set of allowed roles for an action.
///
/// Returns `Ok(())` if the user's role is permitted, or an `ErpError::PermissionDenied`
/// with a descriptive message identifying the required permission on failure.
///
/// # Example
///
/// ```ignore
/// use zavora_erp_api::middleware::auth::{require_role, AuthContext};
/// use zavora_erp_core::rbac::UserRole;
///
/// async fn create_invoice(ctx: AuthContext) {
///     require_role(
///         &[UserRole::Owner, UserRole::Admin, UserRole::Accountant, UserRole::Editor],
///         &ctx,
///         "create invoice",
///     ).unwrap();
/// }
/// ```
pub fn require_role(
    allowed: &[UserRole],
    ctx: &AuthContext,
    action: &str,
) -> Result<(), ErpError> {
    if allowed.contains(&ctx.role) {
        Ok(())
    } else {
        let required_roles: Vec<&str> = allowed.iter().map(|r| role_name(r)).collect();
        Err(ErpError::PermissionDenied {
            action: action.to_string(),
            required_role: required_roles.join(", "),
        })
    }
}

/// Convert a role header string to a `UserRole`.
fn parse_role(s: &str) -> Option<UserRole> {
    match s.to_lowercase().as_str() {
        "owner" => Some(UserRole::Owner),
        "admin" => Some(UserRole::Admin),
        "accountant" => Some(UserRole::Accountant),
        "editor" => Some(UserRole::Editor),
        "approver" => Some(UserRole::Approver),
        "viewer" => Some(UserRole::Viewer),
        _ => None,
    }
}

/// Get the display name of a role.
fn role_name(role: &UserRole) -> &'static str {
    match role {
        UserRole::Owner => "Owner",
        UserRole::Admin => "Admin",
        UserRole::Accountant => "Accountant",
        UserRole::Editor => "Editor",
        UserRole::Approver => "Approver",
        UserRole::Viewer => "Viewer",
    }
}

// ─── Permission group constants ──────────────────────────────────────────────

/// Roles allowed to create invoices, bills, and payments.
pub const ROLES_CREATE: &[UserRole] = &[
    UserRole::Owner,
    UserRole::Admin,
    UserRole::Accountant,
    UserRole::Editor,
];

/// Roles allowed to send invoices and statements.
pub const ROLES_SEND: &[UserRole] = &[
    UserRole::Owner,
    UserRole::Admin,
    UserRole::Accountant,
    UserRole::Editor,
];

/// Roles allowed to approve bills and pay runs.
pub const ROLES_APPROVE: &[UserRole] = &[
    UserRole::Owner,
    UserRole::Admin,
    UserRole::Approver,
];

/// Roles allowed to post journal entries.
pub const ROLES_POST_JOURNAL: &[UserRole] = &[
    UserRole::Owner,
    UserRole::Admin,
    UserRole::Accountant,
];

/// Roles allowed to close or reopen fiscal periods.
pub const ROLES_CLOSE_PERIOD: &[UserRole] = &[
    UserRole::Owner,
    UserRole::Admin,
];

/// Roles allowed to manage users and settings.
pub const ROLES_MANAGE: &[UserRole] = &[
    UserRole::Owner,
    UserRole::Admin,
];

/// All roles — used for read-only access.
pub const ROLES_VIEW: &[UserRole] = &[
    UserRole::Owner,
    UserRole::Admin,
    UserRole::Accountant,
    UserRole::Editor,
    UserRole::Approver,
    UserRole::Viewer,
];
