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

/// Default maximum number of **successful** tenant signups permitted per client
/// within a single fixed window when `SIGNUP_RATE_MAX` is unset or unparseable.
///
/// Only successful signups count against this budget — malformed submissions and
/// duplicate-email retries never consume it, so a user fixing a form is never
/// locked out. The cap exists purely to throttle automated mass tenant creation.
const DEFAULT_SIGNUP_RATE_MAX: u64 = 20;

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

/// Redis key for the current fixed window's successful-signup counter.
fn signup_rate_key(client_key: &str, window: u64) -> String {
    let now = chrono::Utc::now().timestamp().max(0) as u64;
    let window_bucket = now / window;
    format!("signup:rl:{client_key}:{window_bucket}")
}

/// Seconds remaining in the current fixed window (for the `Retry-After` header).
fn signup_rate_retry_after(window: u64) -> u64 {
    let now = chrono::Utc::now().timestamp().max(0) as u64;
    window - (now % window)
}

/// Whether the client has already used its full signup budget for the current
/// window. This only **reads** the counter (no increment), so it never consumes
/// budget itself — malformed or duplicate attempts are free.
///
/// Availability: if Redis is unreachable this fails open (returns `false`) and
/// logs a warning — abuse protection is best-effort and must never block correct
/// tenant creation.
pub async fn signup_rate_exceeded(redis: &mut MultiplexedConnection, client_key: &str) -> bool {
    let max = signup_rate_max();
    let key = signup_rate_key(client_key, signup_rate_window_secs());

    let count: Option<i64> = match redis::cmd("GET").arg(&key).query_async(redis).await {
        Ok(c) => c,
        Err(e) => {
            tracing::warn!(error = %e, "signup rate limiter unavailable (redis); failing open");
            return false;
        }
    };

    count.unwrap_or(0) as u64 >= max
}

/// Record one **successful** signup against the client's window budget. Called
/// only after a tenant is actually provisioned, so failed validations and
/// duplicate-email retries never count. Best-effort: limiter outages are ignored.
pub async fn record_signup(redis: &mut MultiplexedConnection, client_key: &str) {
    let window = signup_rate_window_secs();
    let key = signup_rate_key(client_key, window);

    let count: i64 = match redis::cmd("INCR").arg(&key).query_async(redis).await {
        Ok(c) => c,
        Err(e) => {
            tracing::warn!(error = %e, "failed to record signup for rate limiting; continuing");
            return;
        }
    };

    // Attach the window expiry on the first success of a fresh window.
    if count == 1 {
        if let Err(e) = redis::cmd("EXPIRE")
            .arg(&key)
            .arg(window as i64)
            .query_async::<()>(redis)
            .await
        {
            tracing::warn!(error = %e, "failed to set signup rate-limit window expiry; continuing");
        }
    }
}

// ---------------------------------------------------------------------------
// Signup_Service route (design section "3. Signup_Service route").
// ---------------------------------------------------------------------------

/// Public signup payload. Mirrors the field names used by `register`/`login`
/// while adding the organisation name needed to provision a brand-new tenant.
#[derive(serde::Deserialize)]
pub struct SignupRequest {
    pub organization_name: String,
    pub organization_type: String,
    #[serde(default)]
    pub kra_pin: Option<String>,
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
fn rate_limited(retry_after_secs: u64) -> Response {
    (
        StatusCode::TOO_MANY_REQUESTS,
        [(axum::http::header::RETRY_AFTER, retry_after_secs.to_string())],
        Json(serde_json::json!({
            "error": "Too many signups from this network; please try again later.",
            "retry_after_seconds": retry_after_secs,
        })),
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
    // 1. Rate limit (best-effort; fails open if Redis is down). This only READS
    //    the counter — it does not consume budget — so malformed submissions and
    //    duplicate-email retries below never count against the client. Only a
    //    fully successful signup is recorded (step 4), so a user fixing a form is
    //    never locked out; the cap solely throttles automated mass tenant creation.
    let client_key = derive_client_key(&headers, peer);
    let mut redis: MultiplexedConnection = state.engine.redis_conn().await;
    if signup_rate_exceeded(&mut redis, &client_key).await {
        return Err(rate_limited(signup_rate_retry_after(signup_rate_window_secs())));
    }

    // 2. Validate + normalise before any persistence (Req 7.4). A failure names
    //    exactly one offending field and reveals no identifiers.
    let provision_req = tenant::validate_signup(SignupInput {
        organization_name: req.organization_name,
        organization_type: req.organization_type,
        kra_pin: req.kra_pin,
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

    // 4. Record the successful signup against the client's window budget.
    record_signup(&mut redis, &client_key).await;

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
