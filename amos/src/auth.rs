//! Session identity: verify the logged-in user's ERP access token and bind the
//! session to a single tenant (entity).
//!
//! Amos runs in the same trust domain as the ERP API, so it verifies the
//! access token with the shared `JWT_ACCESS_SECRET` (HS256) — mirroring
//! `zavora-erp-core::auth::decode_access_token`. The tenant boundary: a
//! deployment serves exactly one **entity**, and any session whose verified
//! `entity_id` differs is refused before a single tool runs.

use anyhow::{Result, anyhow};
use jsonwebtoken::{DecodingKey, Validation, decode};
use serde::Deserialize;
use uuid::Uuid;

const TOKEN_TYPE_ACCESS: &str = "access";
const DEFAULT_ISSUER: &str = "zavora-erp";

/// Access-token claims (subset — mirrors the ERP's `Claims`).
#[derive(Debug, Deserialize)]
struct Claims {
    sub: Uuid,
    entity_id: Uuid,
    role: String,
    token_type: String,
    #[serde(default)]
    iss: String,
    // Present so jsonwebtoken's `Validation` enforces expiry; not read directly.
    #[allow(dead_code)]
    exp: i64,
}

/// The verified principal behind a session.
#[derive(Debug, Clone)]
pub struct Principal {
    pub user_id: Uuid,
    pub entity_id: Uuid,
    pub role: String,
}

impl Principal {
    /// Scopes granted to this principal, mirroring the ERP's role gates
    /// (`middleware/auth.rs`: ROLES_POST_JOURNAL = Owner/Admin/Accountant).
    /// Amos can never exceed the user's ERP role.
    pub fn scopes(&self) -> Vec<String> {
        let mut scopes = vec![format!("tenant:{}", self.entity_id), "erp:read".to_string()];
        match self.role.as_str() {
            "Owner" | "Admin" | "Accountant" => {
                scopes.push("erp:write".into());
                scopes.push("ledger:post".into());
            }
            "Approver" => scopes.push("erp:write".into()),
            _ => {} // Viewer and anything unknown: read-only
        }
        scopes
    }
}

/// Verifies access tokens against the shared secret and enforces the served
/// entity.
#[derive(Clone)]
pub struct TokenVerifier {
    secret: String,
    issuer: String,
    pub served_entity: Uuid,
}

impl TokenVerifier {
    pub fn new(served_entity: Uuid) -> Result<Self> {
        let secret = std::env::var("JWT_ACCESS_SECRET")
            .map_err(|_| anyhow!("JWT_ACCESS_SECRET must be set for Amos to verify user identity"))?;
        let issuer = std::env::var("JWT_ISSUER").unwrap_or_else(|_| DEFAULT_ISSUER.to_string());
        Ok(Self { secret, issuer, served_entity })
    }

    /// Verify a token and confirm it belongs to the served entity. Returns the
    /// principal only when the signature, type, expiry, issuer, AND tenant all
    /// check out — this is the tenant boundary.
    pub fn verify(&self, token: &str) -> Result<Principal> {
        let mut validation = Validation::default(); // HS256, checks exp
        validation.set_issuer(&[&self.issuer]);
        let data = decode::<Claims>(token, &DecodingKey::from_secret(self.secret.as_bytes()), &validation)
            .map_err(|e| anyhow!("invalid token: {e}"))?;
        let c = data.claims;
        if c.token_type != TOKEN_TYPE_ACCESS {
            return Err(anyhow!("wrong token type: expected access"));
        }
        // Redundant with validation.set_issuer, but explicit.
        if !c.iss.is_empty() && c.iss != self.issuer {
            return Err(anyhow!("unexpected token issuer"));
        }
        if c.entity_id != self.served_entity {
            return Err(anyhow!(
                "this Amos serves a different organisation — access denied"
            ));
        }
        // Defence in depth: Amos is a back-office-staff-only assistant. External
        // principals — the vendor portal issues `role = "Vendor"` and employee
        // self-service issues `role = "Employee"`, both signed with the same
        // secret for the served entity — must never open a session.
        if c.role.eq_ignore_ascii_case("Vendor") || c.role.eq_ignore_ascii_case("Employee") {
            return Err(anyhow!("external portal accounts cannot use Amos"));
        }
        Ok(Principal { user_id: c.sub, entity_id: c.entity_id, role: c.role })
    }
}
