//! Agent-level authorization: even inside an accepted (same-entity) session,
//! Amos may not exceed the user's ERP role. Each tool is wrapped so its
//! required scopes are checked (via adk-auth's `check_scopes`) against the
//! session principal's granted scopes before it runs, and every attempt is
//! written to the audit trail.

use adk_auth::{AuditEvent, AuditOutcome, AuditSink, check_scopes};
use adk_core::{Tool, ToolContext};
use async_trait::async_trait;
use std::sync::Arc;

/// Capability required to invoke a tool, by tool name. Writes to the ledger
/// require `ledger:post`; other mutations require `erp:write`; everything else
/// is a read. Native orchestration tools (planning, memory, showcase) carry no
/// requirement — they don't touch the books.
fn required_scopes(tool_name: &str) -> &'static [&'static str] {
    match tool_name {
        // Ledger postings — the highest bar.
        "post_bill" | "record_payment" | "post_journal_entry" | "post_invoice" => &["ledger:post"],
        // Other writes: drafting documents and any browser action that can
        // mutate ERP state (clicks/typing/form fills / navigation that posts).
        "create_bill_draft" | "create_invoice_draft" | "create_customer" | "create_vendor"
        | "update_customer" | "update_vendor" | "browser_click" | "browser_type"
        | "browser_fill_form" | "browser_select_option" | "browser_press_key" => &["erp:write"],
        // Native orchestration / memory / evidence — no ERP capability needed.
        "plan_tasks" | "update_task" | "use_skill" | "remember" | "recall" | "showcase_step"
        | "erp_login" => &[],
        // Everything else (lists, gets, reports, dashboard, snapshots,
        // read-only browser tools) is a read.
        _ => &["erp:read"],
    }
}

/// Wraps a tool so its required scopes are enforced against the session
/// principal's granted scopes, with an audit entry per attempt.
pub struct ScopedTool {
    inner: Arc<dyn Tool>,
    granted: Arc<Vec<String>>,
    user_id: String,
    session_id: String,
    audit: Option<Arc<dyn AuditSink>>,
}

impl ScopedTool {
    pub fn wrap(
        inner: Arc<dyn Tool>,
        granted: Arc<Vec<String>>,
        user_id: String,
        session_id: String,
        audit: Option<Arc<dyn AuditSink>>,
    ) -> Arc<dyn Tool> {
        Arc::new(Self { inner, granted, user_id, session_id, audit })
    }

    async fn record(&self, tool: &str, outcome: AuditOutcome) {
        if let Some(sink) = &self.audit {
            let _ = sink
                .log(AuditEvent::tool_access(&self.user_id, tool, outcome).with_session(&self.session_id))
                .await;
        }
    }
}

#[async_trait]
impl Tool for ScopedTool {
    fn name(&self) -> &str {
        self.inner.name()
    }
    fn description(&self) -> &str {
        self.inner.description()
    }
    fn parameters_schema(&self) -> Option<serde_json::Value> {
        self.inner.parameters_schema()
    }

    async fn execute(&self, ctx: Arc<dyn ToolContext>, args: serde_json::Value) -> adk_core::Result<serde_json::Value> {
        let required = required_scopes(self.inner.name());
        match check_scopes(required, &self.granted) {
            Ok(()) => {
                self.record(self.inner.name(), AuditOutcome::Allowed).await;
                self.inner.execute(ctx, args).await
            }
            Err(denied) => {
                tracing::warn!(tool = self.inner.name(), user = %self.user_id, "scope denied: {denied}");
                self.record(self.inner.name(), AuditOutcome::Denied).await;
                // Return an error to the model instead of running — it will
                // relay the refusal to the user.
                Ok(serde_json::json!({
                    "error": format!(
                        "Not permitted: this action needs [{}], which your ERP role does not grant.",
                        required.join(", ")
                    )
                }))
            }
        }
    }
}
