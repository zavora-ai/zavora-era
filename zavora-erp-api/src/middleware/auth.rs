//! RBAC middleware for Zavora ERP API.
//!
//! Extracts user identity from request headers (JWT/session) and provides
//! permission checking via `require_role()`.

use axum::{
    extract::FromRequestParts,
    http::{request::Parts, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use uuid::Uuid;

use zavora_erp_core::rbac::UserRole;
use zavora_erp_core::ErpError;

/// Authentication context extracted from the request.
///
/// Contains the authenticated user's identity and role, used by route handlers
/// to enforce RBAC policies.
#[derive(Debug, Clone)]
pub struct AuthContext {
    pub user_id: Uuid,
    pub entity_id: Uuid,
    pub role: UserRole,
}

/// Extract `AuthContext` from request headers.
///
/// Looks for the following headers (typically set by an upstream auth gateway or JWT middleware):
/// - `X-User-Id`: UUID of the authenticated user
/// - `X-Entity-Id`: UUID of the entity/tenant
/// - `X-User-Role`: one of Owner, Admin, Accountant, Editor, Approver, Viewer
///
/// In production, these would be extracted from a verified JWT token. This extractor
/// supports both direct header injection (for gateway-authenticated requests) and
/// can be extended to verify JWTs directly.
impl<S> FromRequestParts<S> for AuthContext
where
    S: Send + Sync,
{
    type Rejection = Response;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        let user_id = parts
            .headers
            .get("x-user-id")
            .and_then(|v| v.to_str().ok())
            .and_then(|s| Uuid::parse_str(s).ok())
            .ok_or_else(|| {
                (
                    StatusCode::UNAUTHORIZED,
                    Json(serde_json::json!({
                        "error": "Missing or invalid X-User-Id header"
                    })),
                )
                    .into_response()
            })?;

        let entity_id = parts
            .headers
            .get("x-entity-id")
            .and_then(|v| v.to_str().ok())
            .and_then(|s| Uuid::parse_str(s).ok())
            .ok_or_else(|| {
                (
                    StatusCode::UNAUTHORIZED,
                    Json(serde_json::json!({
                        "error": "Missing or invalid X-Entity-Id header"
                    })),
                )
                    .into_response()
            })?;

        let role_str = parts
            .headers
            .get("x-user-role")
            .and_then(|v| v.to_str().ok())
            .ok_or_else(|| {
                (
                    StatusCode::UNAUTHORIZED,
                    Json(serde_json::json!({
                        "error": "Missing X-User-Role header"
                    })),
                )
                    .into_response()
            })?;

        let role = parse_role(role_str).ok_or_else(|| {
            (
                StatusCode::UNAUTHORIZED,
                Json(serde_json::json!({
                    "error": format!("Invalid role: '{}'. Expected one of: Owner, Admin, Accountant, Editor, Approver, Viewer", role_str)
                })),
            )
                .into_response()
        })?;

        Ok(AuthContext {
            user_id,
            entity_id,
            role,
        })
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
