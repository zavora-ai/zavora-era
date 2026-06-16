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

/// Signing configuration loaded from the environment at startup.
#[derive(Clone)]
pub struct JwtConfig {
    access_secret: String,
    refresh_secret: String,
    pub access_ttl_secs: i64,
    pub refresh_ttl_secs: i64,
}

impl std::fmt::Debug for JwtConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // Never leak signing keys (Req 9.3).
        f.debug_struct("JwtConfig")
            .field("access_secret", &"[REDACTED]")
            .field("refresh_secret", &"[REDACTED]")
            .field("access_ttl_secs", &self.access_ttl_secs)
            .field("refresh_ttl_secs", &self.refresh_ttl_secs)
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
        Ok(Self {
            access_secret,
            refresh_secret,
            access_ttl_secs,
            refresh_ttl_secs,
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
    /// Refresh-token id (present on refresh tokens; used for revocation).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub jti: Option<Uuid>,
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
    let now = Utc::now();
    let access_claims = Claims {
        sub: user_id,
        entity_id,
        role: role.to_string(),
        token_type: TOKEN_TYPE_ACCESS.to_string(),
        jti: None,
        iat: now.timestamp(),
        exp: (now + Duration::seconds(config.access_ttl_secs)).timestamp(),
    };

    let refresh_jti = Uuid::new_v4();
    let refresh_expires_at = now + Duration::seconds(config.refresh_ttl_secs);
    let refresh_claims = Claims {
        sub: user_id,
        entity_id,
        role: role.to_string(),
        token_type: TOKEN_TYPE_REFRESH.to_string(),
        jti: Some(refresh_jti),
        iat: now.timestamp(),
        exp: refresh_expires_at.timestamp(),
    };

    let access_token = encode_token(&access_claims, &config.access_secret)?;
    let refresh_token = encode_token(&refresh_claims, &config.refresh_secret)?;

    Ok(TokenPair {
        access_token,
        refresh_token,
        expires_in: config.access_ttl_secs,
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
    decode_token(token, &config.access_secret, TOKEN_TYPE_ACCESS)
}

/// Decode and verify a **refresh** token, returning its claims.
pub fn decode_refresh_token(config: &JwtConfig, token: &str) -> ErpResult<Claims> {
    decode_token(token, &config.refresh_secret, TOKEN_TYPE_REFRESH)
}

fn decode_token(token: &str, secret: &str, expected_type: &str) -> ErpResult<Claims> {
    let validation = Validation::default(); // HS256, validates `exp`
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

#[cfg(test)]
mod tests {
    use super::*;

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

        let r = decode_refresh_token(&cfg, &pair.refresh_token).unwrap();
        assert_eq!(r.jti, Some(pair.refresh_jti));
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
}
