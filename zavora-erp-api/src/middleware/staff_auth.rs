//! Employee self-service authentication (ESS).
//!
//! Employees are an **external-style** principal class, entirely separate from
//! back-office staff (`era_users`). Their JWTs carry `role = "Employee"`, which
//! the back-office `parse_role` does not recognise — so an Employee token is
//! rejected by every ERP/back-office endpoint (and by Amos). Conversely
//! `StaffContext` only accepts `role == "Employee"`, so a back-office token
//! cannot reach the self-service API. This mirrors the vendor portal exactly.
//!
//! Portal routes extract `StaffContext` directly (no global middleware layer),
//! which verifies the bearer token on each request.

use axum::{
    extract::FromRequestParts,
    http::{request::Parts, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use uuid::Uuid;

use zavora_erp_core::auth;

use super::auth::jwt_config;

/// The role string carried by employee self-service tokens.
pub const STAFF_ROLE: &str = "Employee";

fn unauthorized(message: &str) -> Response {
    (
        StatusCode::UNAUTHORIZED,
        Json(serde_json::json!({ "error": message })),
    )
        .into_response()
}

/// Authenticated employee self-service principal, resolved from the verified
/// JWT plus a freshness check against `employee_users` (must still be `active`
/// and linked to an `employees` master).
#[derive(Debug, Clone)]
pub struct StaffContext {
    /// `employee_users.id` — the self-service login.
    pub employee_user_id: Uuid,
    /// The employing tenant.
    pub entity_id: Uuid,
    /// The linked `employees` master. All ESS data is scoped by it.
    pub employee_id: Uuid,
}

/// Verify a Bearer token and require `role == "Employee"`. Does not touch the DB.
pub fn verify_staff_bearer(headers: &axum::http::HeaderMap) -> Result<(Uuid, Uuid), Response> {
    let token = headers
        .get(axum::http::header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.strip_prefix("Bearer ").or_else(|| s.strip_prefix("bearer ")))
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .ok_or_else(|| unauthorized("Missing or malformed Authorization bearer token"))?;

    let claims = auth::decode_access_token(jwt_config(), token)
        .map_err(|e| unauthorized(&e.to_string()))?;

    if claims.role != STAFF_ROLE {
        return Err(unauthorized("This endpoint is for employee self-service accounts only"));
    }
    Ok((claims.sub, claims.entity_id))
}

impl<S> FromRequestParts<S> for StaffContext
where
    S: Send + Sync,
    std::sync::Arc<crate::AppState>: axum::extract::FromRef<S>,
{
    type Rejection = Response;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        use axum::extract::FromRef;
        let (employee_user_id, entity_id) = verify_staff_bearer(&parts.headers)?;
        let app: std::sync::Arc<crate::AppState> = FromRef::from_ref(state);

        // Re-check on every request: account must exist, belong to the token's
        // tenant, be active, and be linked to an employees master.
        let row: Option<(String, Option<Uuid>)> = sqlx::query_as(
            "SELECT status, employee_id FROM employee_users WHERE id = $1 AND entity_id = $2",
        )
        .bind(employee_user_id)
        .bind(entity_id)
        .fetch_optional(app.engine.pool())
        .await
        .map_err(|_| unauthorized("Staff lookup failed"))?;

        let (status, employee_id) = row.ok_or_else(|| unauthorized("Staff account not found"))?;
        if status != "active" {
            return Err(unauthorized("Staff account is not active"));
        }
        let employee_id = employee_id.ok_or_else(|| unauthorized("Staff account is not linked to an employee"))?;

        Ok(StaffContext { employee_user_id, entity_id, employee_id })
    }
}
