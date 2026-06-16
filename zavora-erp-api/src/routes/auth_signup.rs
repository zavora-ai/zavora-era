//! Public tenant signup route and its supporting rate limiter.
//!
//! This module hosts the public `POST /api/v1/auth/signup` endpoint (the signup
//! handler is implemented in a later task) and the `Rate_Limiter` described in
//! the tenant-signup design (section "4. Rate_Limiter"). The rate limiter is a
//! Redis-backed fixed-window counter that throttles abuse of the public signup
//! endpoint while remaining best-effort: if Redis is unavailable it fails open
//! so that legitimate tenant creation is never blocked.

use std::net::SocketAddr;
use std::sync::Arc;

use axum::extract::{ConnectInfo, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::Json;
use redis::aio::MultiplexedConnection;
use zavora_erp_core::auth::{self, TokenPair};
use zavora_erp_core::tenant::{self, ProvisionedTenant, SignupInput};
use zavora_erp_core::{ErpError, ErpResult};

use crate::middleware::auth::jwt_config;
use crate::AppState;

/// Default maximum number of signup attempts permitted per client within a
/// single fixed window when `SIGNUP_RATE_MAX` is unset or unparseable.
const DEFAULT_SIGNUP_RATE_MAX: u64 = 5;

/// Default fixed-window length in seconds when `SIGNUP_RATE_WINDOW_SECS` is
/// unset or unparseable.
const DEFAULT_SIGNUP_RATE_WINDOW_SECS: u64 = 3600;

/// Resolve the configured signup rate threshold from the environment, falling
/// back to a safe default.
fn signup_rate_max() -> u64 {
    std::env::var("SIGNUP_RATE_MAX")
        .ok()
        .and_then(|v| v.trim().parse::<u64>().ok())
        .filter(|&v| v > 0)
        .unwrap_or(DEFAULT_SIGNUP_RATE_MAX)
}

/// Resolve the configured signup rate window (in seconds) from the environment,
/// falling back to a safe default.
fn signup_rate_window_secs() -> u64 {
    std::env::var("SIGNUP_RATE_WINDOW_SECS")
        .ok()
        .and_then(|v| v.trim().parse::<u64>().ok())
        .filter(|&v| v > 0)
        .unwrap_or(DEFAULT_SIGNUP_RATE_WINDOW_SECS)
}

/// Fixed-window rate limiter for the public signup endpoint.
///
/// Returns `Ok(())` when the client is under the configured threshold for the
/// current window and `Err(ErpError::ValidationFailed)` ("rate limited") once
/// the threshold is exceeded.
///
/// Implementation: a fixed-window counter keyed by
/// `signup:rl:{client_key}:{window}`, where `{window}` is the current window
/// bucket. The first request in a window `INCR`s the key to 1 and sets an
/// `EXPIRE` equal to the window length; subsequent requests increment the same
/// counter until the window rolls over and the key expires.
///
/// Availability: if Redis is unreachable the limiter fails open (returns
/// `Ok(())`) and logs a warning — abuse protection is best-effort and must
/// never block correct tenant creation.
pub async fn check_signup_rate(
    redis: &mut MultiplexedConnection,
    client_key: &str,
) -> ErpResult<()> {
    let max = signup_rate_max();
    let window = signup_rate_window_secs();

    // Bucket the current time into a fixed window so old counters expire and
    // every window starts counting from zero.
    let now = chrono::Utc::now().timestamp().max(0) as u64;
    let window_bucket = now / window;
    let key = format!("signup:rl:{client_key}:{window_bucket}");

    // INCR returns the post-increment counter value.
    let count: i64 = match redis::cmd("INCR").arg(&key).query_async(redis).await {
        Ok(c) => c,
        Err(e) => {
            // Fail open: signup correctness is not affected by limiter outages.
            tracing::warn!(
                error = %e,
                "signup rate limiter unavailable (redis); failing open"
            );
            return Ok(());
        }
    };

    // On the first hit of a fresh window, attach the window expiry. Best-effort:
    // a failure here only risks a stale counter, which the next window bucket
    // sidesteps anyway.
    if count == 1 {
        if let Err(e) = redis::cmd("EXPIRE")
            .arg(&key)
            .arg(window as i64)
            .query_async::<()>(redis)
            .await
        {
            tracing::warn!(
                error = %e,
                "failed to set signup rate-limit window expiry; continuing"
            );
        }
    }

    if count as u64 > max {
        return Err(ErpError::ValidationFailed {
            message: "rate limited".to_string(),
        });
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// Signup_Service route (design section "3. Signup_Service route").
// ---------------------------------------------------------------------------

/// Public signup payload. Mirrors the field names used by `register`/`login`
/// while adding the organisation name needed to provision a brand-new tenant.
#[derive(serde::Deserialize)]
pub struct SignupRequest {
    pub organization_name: String,
    pub email: String,
    pub display_name: String,
    pub password: String,
}

const REFRESH_COOKIE: &str = "era_refresh";

fn is_production() -> bool {
    std::env::var("APP_ENV")
        .map(|v| v.eq_ignore_ascii_case("production"))
        .unwrap_or(false)
}

/// Build the `Set-Cookie` value carrying the refresh token. httpOnly so JS can
/// never read it; SameSite=Strict to defeat CSRF; scoped to the auth path —
/// identical to the login/register conventions in `routes::users`.
fn set_refresh_cookie(token: &str, max_age_secs: i64) -> String {
    let secure = if is_production() { "; Secure" } else { "" };
    format!(
        "{REFRESH_COOKIE}={token}; HttpOnly; SameSite=Strict; Path=/api/v1/auth; Max-Age={max_age_secs}{secure}"
    )
}

/// Map an `ErpError` to a concrete HTTP `Response` via the shared mapping.
fn er(e: ErpError) -> Response {
    super::err_response(e).into_response()
}

/// A rate-limited rejection. Returns `429` regardless of the underlying
/// `ErpError` variant so abuse protection is surfaced distinctly from
/// validation failures (design "Error Handling": rate limit → 429).
fn rate_limited() -> Response {
    (
        StatusCode::TOO_MANY_REQUESTS,
        Json(serde_json::json!({ "error": "rate limited" })),
    )
        .into_response()
}

/// Derive the client key used to bucket signup attempts.
///
/// When `SIGNUP_TRUSTED_FORWARDED_HEADER` names a header set by a trusted
/// proxy (e.g. `x-forwarded-for`), the first address in that header is used.
/// Otherwise the socket peer address is used. This keeps the limiter honest
/// behind a proxy without trusting client-supplied headers by default.
fn derive_client_key(headers: &HeaderMap, peer: SocketAddr) -> String {
    if let Ok(header_name) = std::env::var("SIGNUP_TRUSTED_FORWARDED_HEADER") {
        let header_name = header_name.trim();
        if !header_name.is_empty() {
            if let Some(value) = headers.get(header_name).and_then(|v| v.to_str().ok()) {
                // `X-Forwarded-For` may carry a comma-separated chain; the first
                // entry is the originating client per convention.
                if let Some(first) = value.split(',').next() {
                    let first = first.trim();
                    if !first.is_empty() {
                        return first.to_string();
                    }
                }
            }
        }
    }
    peer.ip().to_string()
}

/// Build the success response body: access token + owner identity. The refresh
/// token is intentionally absent from the body — it travels only in the
/// httpOnly cookie (Req 1.5).
fn signup_body(provisioned: &ProvisionedTenant, pair: &TokenPair) -> serde_json::Value {
    serde_json::json!({
        "access_token": pair.access_token,
        "token_type": "Bearer",
        "expires_in": pair.expires_in,
        "user": {
            "user_id": provisioned.owner_user_id,
            "entity_id": provisioned.entity_id,
            "role": provisioned.role,
            "display_name": provisioned.owner_display_name,
            "email": provisioned.owner_email,
        }
    })
}

/// Assemble the full signup success response: access token + owner identity in
/// the body, refresh token only in the `era_refresh` httpOnly cookie.
fn signup_success(provisioned: &ProvisionedTenant, pair: &TokenPair) -> Response {
    let max_age = (pair.refresh_expires_at - chrono::Utc::now())
        .num_seconds()
        .max(0);
    let cookie = set_refresh_cookie(&pair.refresh_token, max_age);
    (
        [(axum::http::header::SET_COOKIE, cookie)],
        Json(signup_body(provisioned, pair)),
    )
        .into_response()
}

/// Persist a freshly issued refresh token so it can later be revoked — the same
/// `refresh_tokens` insert performed by `login`/`register`.
async fn store_refresh_token(
    pool: &sqlx::PgPool,
    pair: &TokenPair,
    user_id: uuid::Uuid,
    entity_id: uuid::Uuid,
) -> ErpResult<()> {
    sqlx::query(
        "INSERT INTO refresh_tokens (jti, user_id, entity_id, expires_at) VALUES ($1, $2, $3, $4)",
    )
    .bind(pair.refresh_jti)
    .bind(user_id)
    .bind(entity_id)
    .bind(pair.refresh_expires_at)
    .execute(pool)
    .await
    .map_err(ErpError::Database)?;
    Ok(())
}

/// POST /api/v1/auth/signup — create a new tenant + first Owner and return a
/// session. Public and unauthenticated (Req 1.1).
///
/// Flow:
/// 1. Rate-limit by client key (429 when over the limit) — Req 10.1.
/// 2. Validate + normalise input (400 with the offending field) — Req 1.6, 7.x.
/// 3. Provision the tenant atomically; a duplicate Owner email yields a generic,
///    non-enumerating 409 — Req 1.2, 8.3, 10.2.
/// 4. Issue a JWT token pair and persist the refresh token — Req 1.3, 5.3.
/// 5. Respond via the shared auth shape: access token + owner identity in the
///    body, refresh token only in the httpOnly `SameSite=Strict` cookie
///    — Req 1.3, 1.4, 1.5.
pub async fn signup(
    State(state): State<Arc<AppState>>,
    ConnectInfo(peer): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
    Json(req): Json<SignupRequest>,
) -> Result<Response, Response> {
    // 1. Rate limit (best-effort; fails open if Redis is down). Over the limit
    //    is surfaced as a distinct 429 rather than a validation error.
    let client_key = derive_client_key(&headers, peer);
    let mut redis: MultiplexedConnection = state.engine.redis_conn().await;
    if check_signup_rate(&mut redis, &client_key).await.is_err() {
        return Err(rate_limited());
    }

    // 2. Validate + normalise before any persistence (Req 7.4). A failure names
    //    exactly one offending field and reveals no identifiers.
    let provision_req = tenant::validate_signup(SignupInput {
        organization_name: req.organization_name,
        owner_email: req.email,
        owner_display_name: req.display_name,
        owner_password: req.password,
    })
    .map_err(er)?;

    // 3. Provision atomically. A duplicate Owner email surfaces as a generic
    //    `Duplicate` (409) that never reveals cross-tenant existence (Req 10.2).
    let provisioned = tenant::provision_tenant(state.engine.pool(), provision_req)
        .await
        .map_err(er)?;

    // 4. Issue the session token pair for the new tenant and persist the
    //    refresh token so it can later be revoked.
    let pair = auth::issue_token_pair(
        jwt_config(),
        provisioned.owner_user_id,
        provisioned.entity_id,
        &provisioned.role,
    )
    .map_err(er)?;
    store_refresh_token(
        state.engine.pool(),
        &pair,
        provisioned.owner_user_id,
        provisioned.entity_id,
    )
    .await
    .map_err(er)?;

    // 5. Access token + owner identity in the body; refresh token only in the
    //    httpOnly SameSite=Strict cookie.
    Ok(signup_success(&provisioned, &pair))
}
