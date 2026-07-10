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

/// External (non-staff) principals that must never open an Amos session. The
/// vendor portal issues `role="Vendor"`, employee self-service `"Employee"`,
/// and the customer portal `"Customer"` — all signed with the same secret for
/// the served entity — so Amos (a back-office-staff assistant) refuses them.
fn is_external_portal_role(role: &str) -> bool {
    ["Vendor", "Employee", "Customer"]
        .iter()
        .any(|r| role.eq_ignore_ascii_case(r))
}

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

/// Why a session was refused. The `code` is a stable, machine-readable tag the
/// UI keys off to show an *honest* state — a persistent "Amos isn't enabled for
/// your organisation yet" gate for `wrong_tenant`/`portal_account`, versus the
/// transient "couldn't reach the service" notice for connectivity.
#[derive(Debug, Clone)]
pub struct VerifyError {
    pub code: &'static str,
    pub message: String,
}

impl VerifyError {
    fn new(code: &'static str, message: impl Into<String>) -> Self {
        Self { code, message: message.into() }
    }
}

impl std::fmt::Display for VerifyError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for VerifyError {}

impl Principal {
    /// Scopes granted to this principal, mirroring the ERP's role gates
    /// (`utils/roles.ts`: ROLES_POST = Owner/Admin/Accountant; ROLES_CREATE/SEND
    /// also include Editor; ROLES_APPROVE = Owner/Admin/Approver). Amos can never
    /// exceed the user's ERP role.
    ///
    /// NOTE: this is a coarse role→scope map. The deeper fix is to derive scopes
    /// from the user's *granular* permissions (`GET /auth/permissions`) so custom
    /// roles (e.g. a bespoke "Bookkeeper") are scoped exactly; tracked in
    /// REMAINING.md §7.2. Until then, unknown/custom roles are safely read-only.
    pub fn scopes(&self) -> Vec<String> {
        let mut scopes = vec![format!("tenant:{}", self.entity_id), "erp:read".to_string()];
        match self.role.as_str() {
            // Post to the ledger (+ write): mirrors ERP ROLES_POST.
            "Owner" | "Admin" | "Accountant" => {
                scopes.push("erp:write".into());
                scopes.push("ledger:post".into());
            }
            // Create/edit/send/approve but NOT post: Editor (ROLES_CREATE/SEND)
            // and Approver (ROLES_APPROVE). Editor was previously mis-mapped to
            // read-only, silently blocking it from drafting through Amos.
            "Approver" | "Editor" => scopes.push("erp:write".into()),
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
    pub fn verify(&self, token: &str) -> Result<Principal, VerifyError> {
        let mut validation = Validation::default(); // HS256, checks exp
        validation.set_issuer(&[&self.issuer]);
        let data = decode::<Claims>(token, &DecodingKey::from_secret(self.secret.as_bytes()), &validation)
            .map_err(|e| VerifyError::new("invalid_token", format!("invalid token: {e}")))?;
        let c = data.claims;
        if c.token_type != TOKEN_TYPE_ACCESS {
            return Err(VerifyError::new("invalid_token", "wrong token type: expected access"));
        }
        // Redundant with validation.set_issuer, but explicit.
        if !c.iss.is_empty() && c.iss != self.issuer {
            return Err(VerifyError::new("invalid_token", "unexpected token issuer"));
        }
        if c.entity_id != self.served_entity {
            // Not an error the user can fix by retrying — Amos simply isn't
            // provisioned for their tenant. The UI turns this into a friendly
            // "being set up for your workspace" gate rather than a dead "Offline".
            return Err(VerifyError::new(
                "wrong_tenant",
                "Amos isn't enabled for your organisation yet",
            ));
        }
        // Defence in depth: Amos is a back-office-staff-only assistant. External
        // principals — vendor portal (`role="Vendor"`), employee self-service
        // (`"Employee"`), and customer portal (`"Customer"`), all signed with the
        // same secret for the served entity — must never open a session.
        if is_external_portal_role(&c.role) {
            return Err(VerifyError::new(
                "portal_account",
                "external portal accounts cannot use Amos",
            ));
        }
        Ok(Principal { user_id: c.sub, entity_id: c.entity_id, role: c.role })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn principal(role: &str) -> Principal {
        Principal { user_id: Uuid::nil(), entity_id: Uuid::nil(), role: role.to_string() }
    }

    #[test]
    fn post_roles_get_ledger_post() {
        for r in ["Owner", "Admin", "Accountant"] {
            let s = principal(r).scopes();
            assert!(s.contains(&"erp:read".to_string()), "{r} read");
            assert!(s.contains(&"erp:write".to_string()), "{r} write");
            assert!(s.contains(&"ledger:post".to_string()), "{r} should post");
        }
    }

    #[test]
    fn write_roles_cannot_post() {
        // Editor was previously mis-mapped to read-only — regression guard.
        for r in ["Approver", "Editor"] {
            let s = principal(r).scopes();
            assert!(s.contains(&"erp:write".to_string()), "{r} should write");
            assert!(!s.contains(&"ledger:post".to_string()), "{r} must NOT post");
        }
    }

    #[test]
    fn unknown_and_viewer_are_read_only() {
        for r in ["Viewer", "Bookkeeper", "SomethingCustom"] {
            let s = principal(r).scopes();
            assert!(s.contains(&"erp:read".to_string()));
            assert!(!s.contains(&"erp:write".to_string()), "{r} read-only");
            assert!(!s.contains(&"ledger:post".to_string()), "{r} read-only");
        }
    }

    #[test]
    fn external_portal_roles_are_blocked() {
        // Customer must be blocked like Vendor/Employee (case-insensitive).
        for r in ["Vendor", "vendor", "Employee", "employee", "Customer", "CUSTOMER"] {
            assert!(is_external_portal_role(r), "{r} must be blocked from Amos");
        }
        for r in ["Owner", "Admin", "Accountant", "Approver", "Editor", "Viewer"] {
            assert!(!is_external_portal_role(r), "{r} is staff, not a portal account");
        }
    }
}
