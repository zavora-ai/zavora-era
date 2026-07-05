//! Prompt-injection & abuse guardrails on inbound user turns.
//!
//! A rules-based screen (zero added latency, no extra model call) that catches
//! the common attacks before a turn reaches the model: instruction-override
//! ("ignore your instructions"), system-prompt / secret exfiltration, and
//! attempts to reach outside the tenant. On a hit the turn is NOT forwarded;
//! Amos replies with a fixed refusal.

use adk_guardrail::{GuardrailResult, Severity};

/// Lowercased-substring signatures. Deliberately conservative — these phrases
/// don't occur in normal accounting requests.
const INJECTION: &[&str] = &[
    "ignore previous instructions",
    "ignore all previous",
    "ignore your instructions",
    "disregard previous",
    "disregard your instructions",
    "disregard all instructions",
    "forget your instructions",
    "forget all previous",
    "you are now",
    "act as if",
    "developer mode",
    "system prompt",
    "reveal your instructions",
    "print your instructions",
    "show me your prompt",
    "what is your system prompt",
    "repeat the text above",
    "override your rules",
    "bypass your rules",
];

const EXFIL: &[&str] = &[
    "your api key",
    "google_api_key",
    "service account password",
    "your password",
    "the jwt secret",
    "access token",
    "other tenant",
    "other company's",
    "another company's books",
    "all tenants",
    "other organisations' data",
];

/// Screen an inbound user message. `Pass` ⇒ forward to the model; `Fail` ⇒
/// refuse with the reason.
pub fn screen_user_input(text: &str) -> GuardrailResult {
    let t = text.to_lowercase();
    if INJECTION.iter().any(|p| t.contains(p)) {
        return GuardrailResult::fail(
            "That looks like an attempt to change my instructions. I can only work on this company's books within my rules.",
            Severity::High,
        );
    }
    if EXFIL.iter().any(|p| t.contains(p)) {
        return GuardrailResult::fail(
            "I can't share credentials, my configuration, or any other organisation's data.",
            Severity::High,
        );
    }
    GuardrailResult::pass()
}

/// Secret-shaped content must never be written to long-term memory. Used by the
/// `remember` tool to enforce the AGENTS.md rule in code.
pub fn looks_like_secret(text: &str) -> bool {
    let t = text.to_lowercase();
    const SECRET_HINTS: &[&str] =
        &["password", "api key", "api_key", "secret", "token", "private key", "credential"];
    SECRET_HINTS.iter().any(|h| t.contains(h))
}

/// The refusal reason from a failed screen, for relaying to the user.
pub fn fail_reason(result: &GuardrailResult) -> Option<String> {
    match result {
        GuardrailResult::Fail { reason, .. } => Some(reason.clone()),
        _ => None,
    }
}
