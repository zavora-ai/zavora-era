//! JWT authentication and password hashing (Requirement 1).
//!
//! User identity is proven cryptographically rather than trusted from
//! client-supplied headers:
//!
//! - Passwords are hashed with **Argon2id** before storage/comparison.
//! - Access tokens are short-lived signed **JWTs** (HS256) carrying the
//!   `user_id`, `entity_id`, and `role` claims.
//! - Refresh tokens are longer-lived JWTs carrying a `jti` (token id) that is
//!   persisted server-side so individual sessions can be revoked.
//!
//! The API layer verifies the access token on every request and rejects any
//! request lacking a valid token — legacy `X-User-*` headers are ignored.

use argon2::password_hash::rand_core::OsRng;
use argon2::password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString};
use argon2::Argon2;
use chrono::{Duration, Utc};
use jsonwebtoken::{decode, encode, DecodingKey, EncodingKey, Header, Validation};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::error::{ErpError, ErpResult};

/// Token kind, embedded in the claims so an access token cannot be replayed as
/// a refresh token (or vice-versa).
pub const TOKEN_TYPE_ACCESS: &str = "access";
pub const TOKEN_TYPE_REFRESH: &str = "refresh";

/// Default access-token lifetime: 15 minutes.
pub const DEFAULT_ACCESS_TTL_SECS: i64 = 15 * 60;
/// Default refresh-token lifetime: 7 days.
pub const DEFAULT_REFRESH_TTL_SECS: i64 = 7 * 24 * 60 * 60;
/// Default token issuer (`iss` claim) when `JWT_ISSUER` is not set.
pub const DEFAULT_ISSUER: &str = "zavora-erp";

/// Signing configuration loaded from the environment at startup.
#[derive(Clone)]
pub struct JwtConfig {
    access_secret: String,
    refresh_secret: String,
    pub access_ttl_secs: i64,
    pub refresh_ttl_secs: i64,
    /// Expected `iss` claim — issued into every token and verified on decode.
    pub issuer: String,
}

impl std::fmt::Debug for JwtConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // Never leak signing keys (Req 9.3).
        f.debug_struct("JwtConfig")
            .field("access_secret", &"[REDACTED]")
            .field("refresh_secret", &"[REDACTED]")
            .field("access_ttl_secs", &self.access_ttl_secs)
            .field("refresh_ttl_secs", &self.refresh_ttl_secs)
            .field("issuer", &self.issuer)
            .finish()
    }
}

impl JwtConfig {
    /// Construct directly (used in tests).
    pub fn new(
        access_secret: String,
        refresh_secret: String,
        access_ttl_secs: i64,
        refresh_ttl_secs: i64,
    ) -> Self {
        Self {
            access_secret,
            refresh_secret,
            access_ttl_secs,
            refresh_ttl_secs,
            issuer: DEFAULT_ISSUER.to_string(),
        }
    }

    /// Construct with an explicit issuer (used in tests / multi-tenant setups).
    pub fn with_issuer(
        access_secret: String,
        refresh_secret: String,
        access_ttl_secs: i64,
        refresh_ttl_secs: i64,
        issuer: String,
    ) -> Self {
        Self {
            access_secret,
            refresh_secret,
            access_ttl_secs,
            refresh_ttl_secs,
            issuer,
        }
    }

    /// Load signing keys from the environment, failing fast with a descriptive
    /// error if a required secret is missing (Req 9.4).
    pub fn from_env() -> ErpResult<Self> {
        let access_secret = require_secret("JWT_ACCESS_SECRET")?;
        let refresh_secret = require_secret("JWT_REFRESH_SECRET")?;
        if access_secret == refresh_secret {
            return Err(ErpError::Internal(
                "JWT_ACCESS_SECRET and JWT_REFRESH_SECRET must differ".to_string(),
            ));
        }
        let access_ttl_secs = std::env::var("JWT_ACCESS_TTL_SECS")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(DEFAULT_ACCESS_TTL_SECS);
        let refresh_ttl_secs = std::env::var("JWT_REFRESH_TTL_SECS")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(DEFAULT_REFRESH_TTL_SECS);
        let issuer = std::env::var("JWT_ISSUER").unwrap_or_else(|_| DEFAULT_ISSUER.to_string());
        Ok(Self {
            access_secret,
            refresh_secret,
            access_ttl_secs,
            refresh_ttl_secs,
            issuer,
        })
    }
}

fn require_secret(name: &str) -> ErpResult<String> {
    match std::env::var(name) {
        Ok(v) if v.len() >= 16 => Ok(v),
        Ok(_) => Err(ErpError::Internal(format!(
            "required secret {name} is too short (must be >= 16 chars)"
        ))),
        Err(_) => Err(ErpError::Internal(format!(
            "required secret {name} is not set"
        ))),
    }
}

/// JWT claims. `sub` is the user id; `entity_id`/`role` scope authorization.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Claims {
    pub sub: Uuid,
    pub entity_id: Uuid,
    pub role: String,
    pub token_type: String,
    /// Token issuer (`iss`). Verified against [`JwtConfig::issuer`] on decode.
    #[serde(default)]
    pub iss: String,
    /// Refresh-token id (present on refresh tokens; used for revocation).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub jti: Option<Uuid>,
    /// When set, this is a platform-operator support session acting as `sub`
    /// inside `entity_id`. Audit / UI can show a support banner.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub impersonator_id: Option<Uuid>,
    /// Support session restricted to read-only tenant permissions (Viewer).
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub read_only: bool,
    pub iat: i64,
    pub exp: i64,
}

/// A freshly issued access + refresh token pair.
#[derive(Debug, Clone, Serialize)]
pub struct TokenPair {
    pub access_token: String,
    pub refresh_token: String,
    /// Access-token lifetime in seconds (for the client to schedule refresh).
    pub expires_in: i64,
    /// Refresh-token id — the caller persists this so the session can be revoked.
    #[serde(skip)]
    pub refresh_jti: Uuid,
    /// Refresh-token absolute expiry (UTC) — the caller persists this.
    #[serde(skip)]
    pub refresh_expires_at: chrono::DateTime<Utc>,
}

/// Hash a plaintext password with Argon2id.
pub fn hash_password(password: &str) -> ErpResult<String> {
    let salt = SaltString::generate(&mut OsRng);
    Argon2::default()
        .hash_password(password.as_bytes(), &salt)
        .map(|h| h.to_string())
        .map_err(|e| ErpError::Internal(format!("password hashing failed: {e}")))
}

/// Verify a plaintext password against a stored Argon2 hash.
pub fn verify_password(password: &str, stored_hash: &str) -> bool {
    match PasswordHash::new(stored_hash) {
        Ok(parsed) => Argon2::default()
            .verify_password(password.as_bytes(), &parsed)
            .is_ok(),
        Err(_) => false,
    }
}

/// Issue a new access + refresh token pair for a user.
pub fn issue_token_pair(
    config: &JwtConfig,
    user_id: Uuid,
    entity_id: Uuid,
    role: &str,
) -> ErpResult<TokenPair> {
    issue_token_pair_opts(config, user_id, entity_id, role, None, false, None, None)
}

/// Issue a short-lived support (impersonation) session for a platform operator
/// acting as a tenant user. Default access TTL is 30 minutes; refresh 2 hours.
///
/// When `read_only` is true the token role is forced to `Viewer` regardless of
/// the target user's role.
pub fn issue_impersonation_token_pair(
    config: &JwtConfig,
    target_user_id: Uuid,
    entity_id: Uuid,
    role: &str,
    impersonator_id: Uuid,
    read_only: bool,
) -> ErpResult<TokenPair> {
    // Support sessions are deliberately shorter than normal logins.
    let access_ttl = std::env::var("PLATFORM_IMPERSONATE_ACCESS_TTL_SECS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(30 * 60);
    let refresh_ttl = std::env::var("PLATFORM_IMPERSONATE_REFRESH_TTL_SECS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(2 * 60 * 60);
    let effective_role = if read_only { "Viewer" } else { role };
    issue_token_pair_opts(
        config,
        target_user_id,
        entity_id,
        effective_role,
        Some(impersonator_id),
        read_only,
        Some(access_ttl),
        Some(refresh_ttl),
    )
}

fn issue_token_pair_opts(
    config: &JwtConfig,
    user_id: Uuid,
    entity_id: Uuid,
    role: &str,
    impersonator_id: Option<Uuid>,
    read_only: bool,
    access_ttl_secs: Option<i64>,
    refresh_ttl_secs: Option<i64>,
) -> ErpResult<TokenPair> {
    let now = Utc::now();
    let access_ttl = access_ttl_secs.unwrap_or(config.access_ttl_secs);
    let refresh_ttl = refresh_ttl_secs.unwrap_or(config.refresh_ttl_secs);
    let access_claims = Claims {
        sub: user_id,
        entity_id,
        role: role.to_string(),
        token_type: TOKEN_TYPE_ACCESS.to_string(),
        iss: config.issuer.clone(),
        jti: None,
        impersonator_id,
        read_only,
        iat: now.timestamp(),
        exp: (now + Duration::seconds(access_ttl)).timestamp(),
    };

    let refresh_jti = Uuid::new_v4();
    let refresh_expires_at = now + Duration::seconds(refresh_ttl);
    let refresh_claims = Claims {
        sub: user_id,
        entity_id,
        role: role.to_string(),
        token_type: TOKEN_TYPE_REFRESH.to_string(),
        iss: config.issuer.clone(),
        jti: Some(refresh_jti),
        impersonator_id,
        read_only,
        iat: now.timestamp(),
        exp: refresh_expires_at.timestamp(),
    };

    let access_token = encode_token(&access_claims, &config.access_secret)?;
    let refresh_token = encode_token(&refresh_claims, &config.refresh_secret)?;

    Ok(TokenPair {
        access_token,
        refresh_token,
        expires_in: access_ttl,
        refresh_jti,
        refresh_expires_at,
    })
}

fn encode_token(claims: &Claims, secret: &str) -> ErpResult<String> {
    encode(
        &Header::default(),
        claims,
        &EncodingKey::from_secret(secret.as_bytes()),
    )
    .map_err(|e| ErpError::Internal(format!("token encoding failed: {e}")))
}

/// Decode and verify an **access** token, returning its claims.
pub fn decode_access_token(config: &JwtConfig, token: &str) -> ErpResult<Claims> {
    decode_token(token, &config.access_secret, TOKEN_TYPE_ACCESS, &config.issuer)
}

/// Decode and verify a **refresh** token, returning its claims.
pub fn decode_refresh_token(config: &JwtConfig, token: &str) -> ErpResult<Claims> {
    decode_token(token, &config.refresh_secret, TOKEN_TYPE_REFRESH, &config.issuer)
}

fn decode_token(
    token: &str,
    secret: &str,
    expected_type: &str,
    issuer: &str,
) -> ErpResult<Claims> {
    let mut validation = Validation::default(); // HS256, validates `exp`
    validation.set_issuer(&[issuer]);
    let data = decode::<Claims>(
        token,
        &DecodingKey::from_secret(secret.as_bytes()),
        &validation,
    )
    .map_err(|e| ErpError::Unauthorized {
        message: format!("invalid token: {e}"),
    })?;

    if data.claims.token_type != expected_type {
        return Err(ErpError::Unauthorized {
            message: format!(
                "wrong token type: expected {expected_type}, got {}",
                data.claims.token_type
            ),
        });
    }
    Ok(data.claims)
}

/// Server-side refresh-token store (Req 1.6, Req 12.5).
///
/// Refresh tokens are issued as signed JWTs (see [`issue_token_pair`]) but a
/// row is also persisted per token so individual sessions can be revoked and so
/// a token cannot be replayed after logout/rotation. The store is backed by the
/// `refresh_tokens` table (migration 006: `jti` PK, `revoked`, `expires_at`),
/// which gives durable revocation across restarts.
///
/// > Note: the design sketch described a Redis-with-TTL store; the committed
/// > schema and API flows use the durable Postgres table instead, so these
/// > helpers centralise that storage. Expired rows are reaped lazily by
/// > [`purge_expired_refresh_tokens`] and excluded by [`refresh_token_active`].
pub mod refresh_store {
    use super::*;

    /// Persist a freshly issued refresh token so it can later be revoked.
    pub async fn persist_refresh_token<'e, E>(
        executor: E,
        pair: &TokenPair,
        user_id: Uuid,
        entity_id: Uuid,
    ) -> ErpResult<()>
    where
        E: sqlx::PgExecutor<'e>,
    {
        sqlx::query(
            "INSERT INTO refresh_tokens (jti, user_id, entity_id, expires_at) \
             VALUES ($1, $2, $3, $4)",
        )
        .bind(pair.refresh_jti)
        .bind(user_id)
        .bind(entity_id)
        .bind(pair.refresh_expires_at)
        .execute(executor)
        .await
        .map_err(ErpError::Database)?;
        Ok(())
    }

    /// Return `true` if the refresh token `jti` exists, is not revoked, and has
    /// not expired (the "valid and not revoked" check for token refresh).
    pub async fn refresh_token_active<'e, E>(executor: E, jti: Uuid) -> ErpResult<bool>
    where
        E: sqlx::PgExecutor<'e>,
    {
        let active = sqlx::query_scalar::<_, bool>(
            "SELECT EXISTS(SELECT 1 FROM refresh_tokens \
             WHERE jti = $1 AND revoked = false AND expires_at > NOW())",
        )
        .bind(jti)
        .fetch_one(executor)
        .await
        .map_err(ErpError::Database)?;
        Ok(active)
    }

    /// Revoke a single refresh token (e.g. on logout or rotation).
    pub async fn revoke_refresh_token<'e, E>(executor: E, jti: Uuid) -> ErpResult<()>
    where
        E: sqlx::PgExecutor<'e>,
    {
        sqlx::query("UPDATE refresh_tokens SET revoked = true WHERE jti = $1")
            .bind(jti)
            .execute(executor)
            .await
            .map_err(ErpError::Database)?;
        Ok(())
    }

    /// Revoke every active refresh token for a user (e.g. on deactivation,
    /// Req 12.5). Returns the number of sessions revoked.
    pub async fn revoke_user_refresh_tokens<'e, E>(executor: E, user_id: Uuid) -> ErpResult<u64>
    where
        E: sqlx::PgExecutor<'e>,
    {
        let result =
            sqlx::query("UPDATE refresh_tokens SET revoked = true WHERE user_id = $1 AND revoked = false")
                .bind(user_id)
                .execute(executor)
                .await
                .map_err(ErpError::Database)?;
        Ok(result.rows_affected())
    }

    /// Delete expired refresh-token rows (housekeeping). Returns rows removed.
    pub async fn purge_expired_refresh_tokens<'e, E>(executor: E) -> ErpResult<u64>
    where
        E: sqlx::PgExecutor<'e>,
    {
        let result = sqlx::query("DELETE FROM refresh_tokens WHERE expires_at <= NOW()")
            .execute(executor)
            .await
            .map_err(ErpError::Database)?;
        Ok(result.rows_affected())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    fn test_config() -> JwtConfig {
        JwtConfig::new(
            "test-access-secret-key-0123456789".to_string(),
            "test-refresh-secret-key-0123456789".to_string(),
            900,
            604800,
        )
    }

    #[test]
    fn password_hash_round_trip() {
        let hash = hash_password("hunter2-correct-horse").unwrap();
        assert!(hash.starts_with("$argon2id$"));
        assert!(verify_password("hunter2-correct-horse", &hash));
        assert!(!verify_password("wrong-password", &hash));
    }

    #[test]
    fn password_hash_is_salted() {
        // Same password hashes to different strings (random salt).
        let a = hash_password("same").unwrap();
        let b = hash_password("same").unwrap();
        assert_ne!(a, b);
    }

    #[test]
    fn jwt_round_trip_preserves_claims() {
        let cfg = test_config();
        let user_id = Uuid::new_v4();
        let entity_id = Uuid::new_v4();
        let pair = issue_token_pair(&cfg, user_id, entity_id, "Owner").unwrap();

        let claims = decode_access_token(&cfg, &pair.access_token).unwrap();
        assert_eq!(claims.sub, user_id);
        assert_eq!(claims.entity_id, entity_id);
        assert_eq!(claims.role, "Owner");
        assert_eq!(claims.token_type, TOKEN_TYPE_ACCESS);
        assert!(claims.impersonator_id.is_none());

        let r = decode_refresh_token(&cfg, &pair.refresh_token).unwrap();
        assert_eq!(r.jti, Some(pair.refresh_jti));
    }

    #[test]
    fn impersonation_token_carries_impersonator_and_shorter_ttl() {
        let cfg = test_config();
        let target = Uuid::new_v4();
        let entity = Uuid::new_v4();
        let ops = Uuid::new_v4();
        let pair =
            issue_impersonation_token_pair(&cfg, target, entity, "Owner", ops, false).unwrap();
        assert_eq!(pair.expires_in, 30 * 60);
        let claims = decode_access_token(&cfg, &pair.access_token).unwrap();
        assert_eq!(claims.sub, target);
        assert_eq!(claims.entity_id, entity);
        assert_eq!(claims.impersonator_id, Some(ops));
        assert!(!claims.read_only);
        assert_eq!(claims.role, "Owner");
        let r = decode_refresh_token(&cfg, &pair.refresh_token).unwrap();
        assert_eq!(r.impersonator_id, Some(ops));

        let ro =
            issue_impersonation_token_pair(&cfg, target, entity, "Owner", ops, true).unwrap();
        let c = decode_access_token(&cfg, &ro.access_token).unwrap();
        assert!(c.read_only);
        assert_eq!(c.role, "Viewer");
    }

    #[test]
    fn access_token_rejected_as_refresh() {
        let cfg = test_config();
        let pair = issue_token_pair(&cfg, Uuid::new_v4(), Uuid::new_v4(), "Admin").unwrap();
        // Access token must not validate against the refresh secret/type.
        assert!(decode_refresh_token(&cfg, &pair.access_token).is_err());
        assert!(decode_access_token(&cfg, &pair.refresh_token).is_err());
    }

    #[test]
    fn tampered_token_rejected() {
        let cfg = test_config();
        let pair = issue_token_pair(&cfg, Uuid::new_v4(), Uuid::new_v4(), "Viewer").unwrap();
        let mut bad = pair.access_token.clone();
        bad.push('x');
        assert!(decode_access_token(&cfg, &bad).is_err());
    }

    #[test]
    fn expired_token_rejected() {
        // TTL well beyond the validator's default 60s leeway => already expired.
        let cfg = JwtConfig::new(
            "test-access-secret-key-0123456789".to_string(),
            "test-refresh-secret-key-0123456789".to_string(),
            -120,
            -120,
        );
        let pair = issue_token_pair(&cfg, Uuid::new_v4(), Uuid::new_v4(), "Owner").unwrap();
        assert!(decode_access_token(&cfg, &pair.access_token).is_err());
    }

    proptest::proptest! {
        #![proptest_config(proptest::prelude::ProptestConfig { cases: 100, ..proptest::prelude::ProptestConfig::default() })]

        // Feature: tenant-signup, Property 6: Password is hashed, never stored in plaintext
        /// For any password, hashing it produces an Argon2id string that
        /// `verify_password` accepts for that password, that differs from the
        /// plaintext, and against which a different password fails to verify.
        /// Validates: Requirements 2.6, 2.3
        #[test]
        fn password_is_hashed_never_plaintext(
            password in ".{1,128}",
            different in ".{1,128}",
        ) {
            // Skip the degenerate case where the two generated passwords collide,
            // since a different password is required to assert verification failure.
            proptest::prop_assume!(password != different);

            let hash = hash_password(&password).expect("hashing must succeed");

            // Output is an Argon2id PHC string.
            proptest::prop_assert!(
                hash.starts_with("$argon2id$"),
                "hash must be Argon2id, got: {}",
                hash
            );
            // The hash is never the plaintext password.
            proptest::prop_assert_ne!(&hash, &password);
            // The correct password verifies against its hash.
            proptest::prop_assert!(verify_password(&password, &hash));
            // A different password does not verify against the hash.
            proptest::prop_assert!(!verify_password(&different, &hash));
        }
    }
}
