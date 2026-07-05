//! Vendor-portal authentication (P2P).
//!
//! Vendors are an **external** principal class, entirely separate from internal
//! staff (`era_users`). Their JWTs carry `role = "Vendor"`, which the staff auth
//! layer's `parse_role` does not recognise — so a Vendor token is rejected by
//! every internal ERP endpoint (and by Amos). Conversely `VendorContext` only
//! accepts `role == "Vendor"`, so a staff token cannot reach the portal API.
//!
//! There is no global middleware layer here: portal routes extract
//! `VendorContext` directly, which verifies the bearer token on each request.

use axum::{
    extract::FromRequestParts,
    http::{request::Parts, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use uuid::Uuid;

use zavora_erp_core::auth;

use super::auth::jwt_config;

/// The role string carried by vendor-portal tokens.
pub const VENDOR_ROLE: &str = "Vendor";

fn unauthorized(message: &str) -> Response {
    (
        StatusCode::UNAUTHORIZED,
        Json(serde_json::json!({ "error": message })),
    )
        .into_response()
}

/// Authenticated vendor-portal principal, resolved from the verified JWT plus a
/// freshness check against `vendor_users` (the account must still be `active`).
#[derive(Debug, Clone)]
pub struct VendorContext {
    /// `vendor_users.id` — the portal login.
    pub vendor_user_id: Uuid,
    /// The tenant this vendor supplies.
    pub entity_id: Uuid,
    /// The linked `vendors` master (set on approval). Portal data is scoped by it.
    pub vendor_id: Uuid,
}

/// Verify a Bearer token and require `role == "Vendor"`. Does not touch the DB.
pub fn verify_vendor_bearer(headers: &axum::http::HeaderMap) -> Result<(Uuid, Uuid), Response> {
    let token = headers
        .get(axum::http::header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.strip_prefix("Bearer ").or_else(|| s.strip_prefix("bearer ")))
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .ok_or_else(|| unauthorized("Missing or malformed Authorization bearer token"))?;

    let claims = auth::decode_access_token(jwt_config(), token)
        .map_err(|e| unauthorized(&e.to_string()))?;

    if claims.role != VENDOR_ROLE {
        return Err(unauthorized("This endpoint is for vendor-portal accounts only"));
    }
    Ok((claims.sub, claims.entity_id))
}

impl<S> FromRequestParts<S> for VendorContext
where
    S: Send + Sync,
    std::sync::Arc<crate::AppState>: axum::extract::FromRef<S>,
{
    type Rejection = Response;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        use axum::extract::FromRef;
        let (vendor_user_id, entity_id) = verify_vendor_bearer(&parts.headers)?;
        let app: std::sync::Arc<crate::AppState> = FromRef::from_ref(state);

        // Re-check the account on every request: it must still exist, belong to
        // the token's tenant, be active, and be linked to a vendors master.
        let row: Option<(String, Option<Uuid>)> = sqlx::query_as(
            "SELECT status, vendor_id FROM vendor_users WHERE id = $1 AND entity_id = $2",
        )
        .bind(vendor_user_id)
        .bind(entity_id)
        .fetch_optional(app.engine.pool())
        .await
        .map_err(|_| unauthorized("Vendor lookup failed"))?;

        let (status, vendor_id) = row.ok_or_else(|| unauthorized("Vendor account not found"))?;
        if status != "active" {
            return Err(unauthorized("Vendor account is not active"));
        }
        let vendor_id = vendor_id.ok_or_else(|| unauthorized("Vendor account is not yet linked"))?;

        Ok(VendorContext { vendor_user_id, entity_id, vendor_id })
    }
}
