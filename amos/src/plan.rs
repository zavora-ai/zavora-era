//! Subscription plans and the feature entitlements they grant.
//!
//! Amos's marginal cost is dominated by **voice**: the Live model bills audio
//! output at ~$12 / 1M tokens, so an "unlimited voice" tier at SME pricing is
//! not sustainable for heavy talkers. Plans exist to put the expensive
//! capabilities (voice, and to a lesser extent web search) behind higher tiers,
//! while every plan keeps the cheap, high-value basics (text chat, document
//! reading).
//!
//! Source of truth: the plan arrives on the WebSocket handshake (the ERP can
//! tell Amos which plan the account is on), falling back to the `AMOS_PLAN`
//! deployment env, then to `Business`. This mirrors how the session clock and
//! timezone are resolved — the client is authoritative, the deployment sets a
//! default. Nothing here charges money; it only *gates features* by plan.

use serde::Serialize;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Plan {
    /// Free / entry tier — Amos by text, no voice.
    Starter,
    /// The mainstream paid tier — voice + web search.
    Business,
    /// Multi-branch / groups — everything.
    Scale,
}

/// What a session is allowed to do. Serialized to the UI so it can hide the mic
/// on a text-only plan rather than letting the user hit a wall.
#[derive(Clone, Copy, Debug, Serialize)]
pub struct Entitlements {
    pub plan: &'static str,
    /// Realtime voice (mic in, spoken out) — the expensive audio path.
    pub voice: bool,
    /// The web_search sub-agent (per-query Google grounding cost).
    pub web_search: bool,
    /// The analyze_attachment sub-agent (cheap, high-value — on every plan).
    pub attachments: bool,
}

impl Plan {
    /// Resolve the plan: handshake value → `AMOS_PLAN` env → `Business`.
    pub fn resolve(handshake: Option<&str>) -> Self {
        handshake
            .map(str::to_string)
            .or_else(|| std::env::var("AMOS_PLAN").ok())
            .map(|s| Self::parse(&s))
            .unwrap_or(Plan::Business)
    }

    fn parse(s: &str) -> Self {
        match s.trim().to_lowercase().as_str() {
            "starter" | "free" | "basic" => Plan::Starter,
            "scale" | "enterprise" | "group" => Plan::Scale,
            _ => Plan::Business,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Plan::Starter => "starter",
            Plan::Business => "business",
            Plan::Scale => "scale",
        }
    }

    pub fn entitlements(self) -> Entitlements {
        match self {
            Plan::Starter => Entitlements { plan: self.name(), voice: false, web_search: false, attachments: true },
            Plan::Business => Entitlements { plan: self.name(), voice: true, web_search: true, attachments: true },
            Plan::Scale => Entitlements { plan: self.name(), voice: true, web_search: true, attachments: true },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolves_and_gates() {
        assert_eq!(Plan::resolve(Some("starter")).name(), "starter");
        assert!(!Plan::resolve(Some("starter")).entitlements().voice);
        assert!(Plan::resolve(Some("business")).entitlements().voice);
        assert!(Plan::resolve(Some("scale")).entitlements().web_search);
        // Unknown / absent → Business default.
        assert_eq!(Plan::resolve(Some("banana")).name(), "business");
        assert_eq!(Plan::resolve(None).name(), "business");
    }
}
