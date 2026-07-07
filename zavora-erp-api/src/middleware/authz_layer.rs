//! Central authorization enforcement (RBAC v2, default-deny).
//!
//! Runs on the protected router *after* `require_authenticated`. For each request
//! it looks up `(method, matched path)` in the declarative `ROUTE_PERMISSIONS`
//! registry and enforces the required permission via `require_permission`. A
//! route with **no registry entry is denied** (default-deny) and logged loudly —
//! so no protected endpoint can ever be reached without an explicit, audited
//! permission decision. This is the single, auditable access-control gate.

use std::sync::Arc;

use axum::{
    extract::{MatchedPath, Request, State},
    http::StatusCode,
    middleware::Next,
    response::{IntoResponse, Response},
    Json,
};

use super::auth::{require_permission, AuthContext};
use super::route_perms::{Access, ROUTE_PERMISSIONS};
use crate::AppState;

fn forbidden(msg: &str) -> Response {
    (StatusCode::FORBIDDEN, Json(serde_json::json!({ "error": msg }))).into_response()
}

/// Look up the access requirement for a `(method, matched_path)`.
fn required_access(method: &str, path: &str) -> Option<Access> {
    ROUTE_PERMISSIONS
        .iter()
        .find(|(m, p, _)| *m == method && *p == path)
        .map(|(_, _, a)| *a)
}

/// Default-deny permission gate for the protected router.
pub async fn enforce_permissions(
    State(state): State<Arc<AppState>>,
    req: Request,
    next: Next,
) -> Response {
    let method = req.method().as_str().to_string();
    let matched = req
        .extensions()
        .get::<MatchedPath>()
        .map(|m| m.as_str().to_string());

    let Some(path) = matched else {
        // No matched route pattern — cannot make a permission decision → deny.
        tracing::error!("authz default-deny: request with no matched path ({method})");
        return forbidden("Not permitted");
    };

    match required_access(&method, &path) {
        Some(Access::SelfScoped) => next.run(req).await,
        Some(Access::Perm(key)) => {
            let Some(ctx) = req.extensions().get::<AuthContext>().cloned() else {
                return forbidden("Not authenticated");
            };
            match require_permission(&state, &ctx, key).await {
                Ok(()) => next.run(req).await,
                Err(e) => crate::routes::err_response(e).into_response(),
            }
        }
        None => {
            // Default-deny: a protected route with no declared permission must never
            // be silently open. Add it to ROUTE_PERMISSIONS (gen_route_perms.py).
            tracing::error!("authz default-deny: no permission mapping for {method} {path}");
            forbidden("This action is not permitted")
        }
    }
}
