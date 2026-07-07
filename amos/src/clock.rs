//! Amos's sense of time — the session clock and the `current_datetime` tool.
//!
//! An LLM has no clock, and an accountant that guesses "today" is dangerous:
//! ageing, "this month", overdue, and posting dates all hinge on it. Two facts
//! shape the design:
//!
//! 1. **Timezone matters.** The tenant sets an IANA timezone (e.g. `Africa/Nairobi`).
//!    Computing "today" in UTC is wrong near midnight — 01:00 in Nairobi (UTC+3)
//!    is still "yesterday" in UTC. We resolve the real date in the user's zone.
//! 2. **The posting date is a user preference.** The ERP UI lets each user pick a
//!    "work-as-of date" (`workDate.ts`) so they can finalise a prior period without
//!    retyping the date on every form. New documents default to it. Amos must
//!    respect the *same* date, so it never silently posts to the wrong day.
//!
//! Both values are per-user preferences held on the client; they arrive with the
//! WebSocket handshake ([`SessionClock::from_handshake`]) and can be refreshed
//! mid-session. The clock is injected into the system instruction (so the model
//! is always grounded without a tool call) *and* exposed as `current_datetime`
//! for precise or long-running / cross-midnight sessions.

use adk_realtime::config::ToolDefinition;
use adk_realtime::events::ToolCall;
use adk_realtime::runner::ToolHandler;
use async_trait::async_trait;
use chrono::{DateTime, NaiveDate, Utc};
use chrono_tz::Tz;
use serde_json::json;
use std::sync::Arc;
use tokio::sync::RwLock;

/// A session's timezone + optional work-as-of (posting) date, both sourced from
/// the signed-in user's preferences.
#[derive(Clone)]
pub struct SessionClock {
    pub tz: Tz,
    /// The user's work-as-of date; `None` means "use the real current date".
    pub work_date: Option<NaiveDate>,
}

impl SessionClock {
    /// Resolve from the raw handshake values. Falls back to `AMOS_DEFAULT_TIMEZONE`
    /// (then `Africa/Nairobi`) for an absent/invalid zone, and ignores an
    /// unparseable work date rather than failing the session.
    pub fn from_handshake(timezone: Option<&str>, work_date: Option<&str>) -> Self {
        let default_tz =
            std::env::var("AMOS_DEFAULT_TIMEZONE").unwrap_or_else(|_| "Africa/Nairobi".to_string());
        let tz = timezone
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .and_then(|s| s.parse::<Tz>().ok())
            .or_else(|| default_tz.parse::<Tz>().ok())
            .unwrap_or(chrono_tz::Africa::Nairobi);
        let work_date = work_date
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .and_then(|s| NaiveDate::parse_from_str(s, "%Y-%m-%d").ok());
        Self { tz, work_date }
    }

    pub fn now(&self) -> DateTime<Tz> {
        Utc::now().with_timezone(&self.tz)
    }

    /// The real calendar date in the user's timezone.
    pub fn real_today(&self) -> NaiveDate {
        self.now().date_naive()
    }

    /// The date new documents/postings should default to: the work-as-of date
    /// when set, otherwise the real today.
    pub fn effective_posting_date(&self) -> NaiveDate {
        self.work_date.unwrap_or_else(|| self.real_today())
    }

    /// A block for the system instruction (`{now}` placeholder). Always grounds
    /// the model without a tool call.
    pub fn instruction_block(&self) -> String {
        let now = self.now();
        let stamp = now.format("%A, %-d %B %Y, %H:%M");
        let offset = now.format("%:z");
        let tz = self.tz.name();
        let posting = match self.work_date {
            Some(d) => format!(
                "The user's work-as-of (posting) date is **{}**. DEFAULT the date on any \
                 document you create or post to this date unless the user names another — \
                 they are working in that period on purpose.",
                d.format("%A, %-d %B %Y")
            ),
            None => "The user has not set a work-as-of date, so use the real current date above \
                     as the default posting date."
                .to_string(),
        };
        format!(
            "Current date & time: {stamp} ({tz}, UTC{offset}). Treat \"today\" and \"now\" as \
             this real current time. {posting} All dates in tool calls must be ISO YYYY-MM-DD. \
             If a session runs long or you need the exact time again, call current_datetime."
        )
    }
}

impl Default for SessionClock {
    fn default() -> Self {
        Self { tz: chrono_tz::Africa::Nairobi, work_date: None }
    }
}

/// Shared, updatable clock — the `context` WS frame can refresh it mid-session
/// (e.g. the user changes their work-date) and the tool reads the latest.
pub type SharedClock = Arc<RwLock<SessionClock>>;

pub fn shared(clock: SessionClock) -> SharedClock {
    Arc::new(RwLock::new(clock))
}

// ─── current_datetime tool ───────────────────────────────────────────────────

pub fn current_datetime_def() -> ToolDefinition {
    ToolDefinition {
        name: "current_datetime".into(),
        description: Some(
            "Get the real current date and time in the user's timezone, plus the user's \
             work-as-of (posting) date. Use it when you need the exact time again, when a \
             session has run a while, or before stamping a date on something and you're not \
             sure what 'today' is. Prefer effective_posting_date for new documents."
                .into(),
        ),
        parameters: Some(json!({ "type": "object", "properties": {} })),
    }
}

pub struct CurrentDateTime {
    pub clock: SharedClock,
}

#[async_trait]
impl ToolHandler for CurrentDateTime {
    async fn execute(&self, _call: &ToolCall) -> adk_realtime::error::Result<serde_json::Value> {
        let clock = self.clock.read().await.clone();
        let now = clock.now();
        Ok(json!({
            "now_iso": now.to_rfc3339(),
            "real_today": clock.real_today().to_string(),
            "weekday": now.format("%A").to_string(),
            "local_time": now.format("%H:%M").to_string(),
            "timezone": clock.tz.name(),
            "utc_offset": now.format("%:z").to_string(),
            "work_as_of_date": clock.work_date.map(|d| d.to_string()),
            "effective_posting_date": clock.effective_posting_date().to_string(),
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolves_timezone_and_work_date() {
        let c = SessionClock::from_handshake(Some("America/New_York"), Some("2025-12-31"));
        assert_eq!(c.tz, chrono_tz::America::New_York);
        assert_eq!(c.work_date, Some(NaiveDate::from_ymd_opt(2025, 12, 31).unwrap()));
        assert_eq!(c.effective_posting_date(), NaiveDate::from_ymd_opt(2025, 12, 31).unwrap());
    }

    #[test]
    fn bad_inputs_fall_back_safely() {
        let c = SessionClock::from_handshake(Some("Not/AZone"), Some("31-12-2025"));
        assert_eq!(c.tz, chrono_tz::Africa::Nairobi); // default
        assert_eq!(c.work_date, None); // unparseable → ignored
        // No work date ⇒ posting date is the real today (in Nairobi).
        assert_eq!(c.effective_posting_date(), c.real_today());
    }

    #[test]
    fn empty_and_absent_use_defaults() {
        let c = SessionClock::from_handshake(None, None);
        assert_eq!(c.tz, chrono_tz::Africa::Nairobi);
        assert!(c.work_date.is_none());
        assert!(c.instruction_block().contains("has not set a work-as-of date"));
    }
}
