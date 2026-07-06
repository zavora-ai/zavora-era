//! Customer-portal authentication (CRM add-in).
//!
//! Customers are an **external** principal class (`customer_users`), entirely
//! separate from back-office staff (`era_users`), vendors, and employees. Their
//! JWTs carry `role = "Customer"`, which the back-office `parse_role` does not
//! recognise — so a Customer token is rejected by every ERP/back-office endpoint.
//! Conversely `CustomerContext` only accepts `role == "Customer"`. Mirrors the
//! vendor and staff portals.

use axum::{
    extract::FromRequestParts,
    http::{request::Parts, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use uuid::Uuid;

use zavora_erp_core::auth;

use super::auth::jwt_config;

/// The role string carried by customer-portal tokens.
pub const CUSTOMER_ROLE: &str = "Customer";

fn unauthorized(message: &str) -> Response {
    (StatusCode::UNAUTHORIZED, Json(serde_json::json!({ "error": message }))).into_response()
}

/// Authenticated customer-portal principal, resolved from the verified JWT plus
/// a freshness check against `customer_users` (must still be `active`).
#[derive(Debug, Clone)]
pub struct CustomerContext {
    /// `customer_users.id` — the portal login.
    pub customer_user_id: Uuid,
    /// The serving tenant.
    pub entity_id: Uuid,
    /// The linked AR `customers` account, if any (needed for invoices/statement).
    pub customer_id: Option<Uuid>,
}

/// Verify a Bearer token and require `role == "Customer"`. No DB access.
pub fn verify_customer_bearer(headers: &axum::http::HeaderMap) -> Result<(Uuid, Uuid), Response> {
    let token = headers
        .get(axum::http::header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.strip_prefix("Bearer ").or_else(|| s.strip_prefix("bearer ")))
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .ok_or_else(|| unauthorized("Missing or malformed Authorization bearer token"))?;

    let claims = auth::decode_access_token(jwt_config(), token).map_err(|e| unauthorized(&e.to_string()))?;
    if claims.role != CUSTOMER_ROLE {
        return Err(unauthorized("This endpoint is for customer-portal accounts only"));
    }
    Ok((claims.sub, claims.entity_id))
}

impl<S> FromRequestParts<S> for CustomerContext
where
    S: Send + Sync,
    std::sync::Arc<crate::AppState>: axum::extract::FromRef<S>,
{
    type Rejection = Response;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        use axum::extract::FromRef;
        let (customer_user_id, entity_id) = verify_customer_bearer(&parts.headers)?;
        let app: std::sync::Arc<crate::AppState> = FromRef::from_ref(state);

        let row: Option<(String, Option<Uuid>)> = sqlx::query_as(
            "SELECT status, customer_id FROM customer_users WHERE id = $1 AND entity_id = $2",
        )
        .bind(customer_user_id)
        .bind(entity_id)
        .fetch_optional(app.engine.pool())
        .await
        .map_err(|_| unauthorized("Customer lookup failed"))?;

        let (status, customer_id) = row.ok_or_else(|| unauthorized("Customer account not found"))?;
        if status != "active" {
            return Err(unauthorized("Customer account is not active"));
        }
        Ok(CustomerContext { customer_user_id, entity_id, customer_id })
    }
}
