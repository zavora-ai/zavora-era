//! Platform super-admin authentication (separate from tenant AuthContext).

use axum::{
    extract::FromRequestParts,
    http::{request::Parts, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use uuid::Uuid;

use zavora_erp_core::auth;
use zavora_erp_core::platform::{is_platform_role, platform_entity_id};

use super::auth::jwt_config;

/// Authenticated platform operator.
#[derive(Debug, Clone)]
pub struct PlatformAuthContext {
    pub user_id: Uuid,
    pub email: String,
    pub display_name: String,
    pub role: String,
}

fn unauthorized(message: &str) -> Response {
    (
        StatusCode::UNAUTHORIZED,
        Json(serde_json::json!({ "error": message })),
    )
        .into_response()
}

/// Verify Bearer access token is a platform operator JWT.
pub fn verify_platform_bearer(headers: &axum::http::HeaderMap) -> Result<PlatformAuthContext, Response> {
    let token = headers
        .get(axum::http::header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.strip_prefix("Bearer ").or_else(|| s.strip_prefix("bearer ")))
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .ok_or_else(|| unauthorized("Missing or malformed Authorization bearer token"))?;

    let claims = auth::decode_access_token(jwt_config(), token)
        .map_err(|e| unauthorized(&e.to_string()))?;

    if !is_platform_role(&claims.role) {
        return Err(unauthorized("Not a platform operator token"));
    }
    // Platform tokens use the nil entity id.
    if claims.entity_id != platform_entity_id() {
        return Err(unauthorized("Invalid platform token scope"));
    }

    // Load display fields best-effort from claims only for extractor speed;
    // handlers may re-fetch from DB when needed.
    Ok(PlatformAuthContext {
        user_id: claims.sub,
        email: String::new(),
        display_name: String::new(),
        role: claims.role,
    })
}

impl<S> FromRequestParts<S> for PlatformAuthContext
where
    S: Send + Sync,
{
    type Rejection = Response;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        let mut ctx = verify_platform_bearer(&parts.headers)?;
        // Enrich from extensions if a middleware stashed the row; otherwise leave empty.
        if let Some(email) = parts.extensions.get::<PlatformEmail>() {
            ctx.email = email.0.clone();
            ctx.display_name = email.1.clone();
        }
        Ok(ctx)
    }
}

/// Optional enrichment stashed by handlers after DB lookup.
#[derive(Clone)]
pub struct PlatformEmail(pub String, pub String);
