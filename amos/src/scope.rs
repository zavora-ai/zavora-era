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
        // Ledger postings, statutory transmissions and period locks — the
        // highest bar. Anything that writes journal lines, moves money,
        // files with KRA, or (un)locks a period lands here.
        "post_bill" | "record_payment" | "post_journal_entry" | "post_invoice"
        | "post_pay_run" | "approve_pay_run" | "mark_pay_run_paid"
        | "close_period" | "reopen_period"
        | "file_tax_return" | "remit_tax_filing" | "etims_transmit_invoice"
        | "run_depreciation" | "run_fx_revaluation"
        | "complete_reconciliation"
        // Stock receipts/adjustments and debit notes post inventory JEs.
        | "receive_goods" | "adjust_stock" | "create_debit_note" => &["ledger:post"],
        // Other writes: drafting documents, masters, payroll prep, procurement
        // workflow, outward sends, and any browser action that can mutate ERP
        // state (clicks/typing/form fills / navigation that posts).
        "create_bill_draft" | "create_invoice_draft" | "submit_invoice"
        | "create_customer" | "create_vendor" | "update_customer" | "update_vendor"
        | "create_product" | "update_product" | "transfer_stock"
        | "send_customer_statement" | "import_bank_statement" | "compute_reconciliation"
        | "set_budget" | "run_payroll" | "add_pay_run_input" | "recompute_pay_run"
        | "create_sales_order_draft" | "submit_sales_order"
        | "create_purchase_order_draft" | "submit_purchase_order" | "create_direct_po"
        | "send_purchase_order" | "create_requisition" | "approve_requisition"
        | "convert_requisition" | "create_expense_claim" | "approve_expense_claim"
        | "browser_click" | "browser_type" | "browser_fill_form"
        | "browser_select_option" | "browser_press_key" => &["erp:write"],
        // Native orchestration / memory / evidence — no ERP capability needed.
        "plan_tasks" | "update_task" | "use_skill" | "remember" | "recall" | "showcase_step"
        | "erp_login" => &[],
        // Everything else (lists, gets, reports, dashboard, snapshots,
        // read-only browser tools) is a read.
        _ => &["erp:read"],
    }
}

/// How long a posting waits for the user's Approve/Deny before giving up.
const CONFIRM_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(120);

/// Wraps a tool so its required scopes are enforced against the session
/// principal's granted scopes, with an audit entry per attempt. In an
/// interactive session (`session` is Some), ledger:post tools additionally
/// block until the user clicks Approve in the UI — the code gate behind the
/// confirm-before-write promise. Ambient routines pass None: they are
/// deliberately unattended (e.g. the eTIMS sweep).
pub struct ScopedTool {
    inner: Arc<dyn Tool>,
    granted: Arc<Vec<String>>,
    user_id: String,
    session_id: String,
    audit: Option<Arc<dyn AuditSink>>,
    session: Option<Arc<crate::state::SessionState>>,
}

impl ScopedTool {
    pub fn wrap(
        inner: Arc<dyn Tool>,
        granted: Arc<Vec<String>>,
        user_id: String,
        session_id: String,
        audit: Option<Arc<dyn AuditSink>>,
        session: Option<Arc<crate::state::SessionState>>,
    ) -> Arc<dyn Tool> {
        Arc::new(Self { inner, granted, user_id, session_id, audit, session })
    }

    /// The interactive write gate. Returns Ok(()) to proceed, or the refusal
    /// message the model should relay.
    async fn confirm_with_user(&self, args: &serde_json::Value) -> Result<(), String> {
        let Some(session) = &self.session else { return Ok(()) };
        // Escape hatch for demos/dev; the gate is on by default.
        if matches!(std::env::var("AMOS_CONFIRM_WRITES").as_deref(), Ok("0") | Ok("false")) {
            return Ok(());
        }

        let (id, rx) = session.confirmations.request().await;
        // Compact argument preview so the user can see WHAT they are approving
        // without a wall of JSON.
        let preview = serde_json::to_string(args).unwrap_or_default();
        let preview = if preview.chars().count() > 400 {
            let cut: String = preview.chars().take(400).collect();
            format!("{cut}…")
        } else {
            preview
        };
        session.push_json(serde_json::json!({
            "type": "confirm_request",
            "id": id,
            "tool": self.inner.name(),
            "args": preview,
        }));

        match tokio::time::timeout(CONFIRM_TIMEOUT, rx).await {
            Ok(Ok(true)) => Ok(()),
            Ok(Ok(false)) => {
                session.push_json(serde_json::json!({"type": "confirm_closed", "id": id}));
                Err("The user DECLINED this posting. Do not retry it; ask what they want changed.".to_string())
            }
            _ => {
                session.confirmations.forget(&id).await;
                session.push_json(serde_json::json!({"type": "confirm_closed", "id": id}));
                Err("No confirmation arrived in time, so the posting was NOT executed. Ask the user to approve the pending action and try again.".to_string())
            }
        }
    }

    async fn record(&self, tool: &str, outcome: AuditOutcome) {
        if let Some(sink) = &self.audit {
            let _ = sink
                .log(AuditEvent::tool_access(&self.user_id, tool, outcome).with_session(&self.session_id))
                .await;
        }
    }

    /// Inject the session user's access token into an ERP tool's arguments so
    /// mcp-erp uses it as the request bearer (making the human the ledger
    /// actor). Only ERP tools take it — `browser_*` tools don't, and would
    /// reject an unknown arg. ScopedTool wraps only ERP + browser MCP tools, so
    /// "not a browser tool" == "an ERP tool". Ambient routines pass
    /// `session: None` and so run as the service account, by design.
    async fn with_user_token(&self, mut args: serde_json::Value) -> serde_json::Value {
        if self.inner.name().starts_with("browser_") {
            return args;
        }
        let Some(session) = &self.session else { return args };
        let Some(token) = session.user_token().await else { return args };
        if let Some(obj) = args.as_object_mut() {
            obj.insert(USER_TOKEN_ARG.to_string(), serde_json::Value::String(token));
        }
        args
    }
}

/// The argument name that carries the session user's access token to mcp-erp.
/// Must match `mcp-erp`'s `server::USER_TOKEN_ARG`; mcp-erp strips it before
/// the typed tool input deserializes.
const USER_TOKEN_ARG: &str = "__user_token";

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
                // Scope alone isn't enough for a posting in an interactive
                // session — the user must approve THIS call in the UI.
                if required.contains(&"ledger:post") {
                    if let Err(refusal) = self.confirm_with_user(&args).await {
                        tracing::warn!(tool = self.inner.name(), user = %self.user_id, "write not confirmed");
                        self.record(self.inner.name(), AuditOutcome::Denied).await;
                        return Ok(serde_json::json!({ "error": refusal }));
                    }
                }
                self.record(self.inner.name(), AuditOutcome::Allowed).await;
                // User-scoped ERP auth: thread the session user's access token
                // to mcp-erp (as `__user_token`) so the ledger actor is the
                // human, not the service account. Injected AFTER the confirm
                // preview so the token never appears on the approval card, and
                // it is not in any tool's schema so the model never sets it.
                let args = self.with_user_token(args).await;
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

#[cfg(test)]
mod inject_tests {
    use super::*;
    use crate::state::SessionState;

    struct DummyTool {
        name: String,
    }
    #[async_trait]
    impl Tool for DummyTool {
        fn name(&self) -> &str {
            &self.name
        }
        fn description(&self) -> &str {
            "dummy"
        }
        async fn execute(&self, _ctx: Arc<dyn ToolContext>, args: serde_json::Value) -> adk_core::Result<serde_json::Value> {
            Ok(args)
        }
    }

    fn scoped(name: &str, session: Option<Arc<SessionState>>) -> ScopedTool {
        ScopedTool {
            inner: Arc::new(DummyTool { name: name.to_string() }),
            granted: Arc::new(vec!["erp:read".into(), "erp:write".into(), "ledger:post".into()]),
            user_id: "u".into(),
            session_id: "s".into(),
            audit: None,
            session,
        }
    }

    #[tokio::test]
    async fn injects_token_for_erp_tool() {
        let session = Arc::new(SessionState::new());
        session.set_user_token("jwt-xyz".into()).await;
        let out = scoped("post_bill", Some(session)).with_user_token(serde_json::json!({"id": "b1"})).await;
        assert_eq!(out["__user_token"], "jwt-xyz", "ERP tool gets the user token");
        assert_eq!(out["id"], "b1", "existing args preserved");
    }

    #[tokio::test]
    async fn no_token_for_browser_tool() {
        let session = Arc::new(SessionState::new());
        session.set_user_token("jwt-xyz".into()).await;
        let out = scoped("browser_click", Some(session)).with_user_token(serde_json::json!({"ref": "e1"})).await;
        assert!(out.get("__user_token").is_none(), "browser tools must not get the token");
    }

    #[tokio::test]
    async fn no_token_when_session_absent() {
        // Ambient routines pass session: None → service account, by design.
        let out = scoped("post_bill", None).with_user_token(serde_json::json!({"id": "b1"})).await;
        assert!(out.get("__user_token").is_none());
    }

    #[tokio::test]
    async fn no_token_when_session_has_none() {
        let session = Arc::new(SessionState::new()); // token never set
        let out = scoped("post_bill", Some(session)).with_user_token(serde_json::json!({"id": "b1"})).await;
        assert!(out.get("__user_token").is_none());
    }
}

#[cfg(test)]
mod tests {
    use super::required_scopes;

    /// Every mutating mcp-erp tool must map to a write or post scope. A tool
    /// falling through to the read arm is a privilege hole (a Viewer session
    /// could run it) — this test pins the full mutating surface so adding a
    /// tool to mcp-erp without classifying it here fails loudly.
    #[test]
    fn every_mutating_tool_requires_write_or_post() {
        let posts = [
            "post_bill", "record_payment", "post_journal_entry", "post_invoice",
            "post_pay_run", "approve_pay_run", "mark_pay_run_paid",
            "close_period", "reopen_period",
            "file_tax_return", "remit_tax_filing", "etims_transmit_invoice",
            "run_depreciation", "run_fx_revaluation", "complete_reconciliation",
            "receive_goods", "adjust_stock", "create_debit_note",
        ];
        let writes = [
            "create_bill_draft", "create_invoice_draft", "submit_invoice",
            "create_customer", "create_vendor", "update_customer", "update_vendor",
            "create_product", "update_product", "transfer_stock",
            "send_customer_statement", "import_bank_statement", "compute_reconciliation",
            "set_budget", "run_payroll", "add_pay_run_input", "recompute_pay_run",
            "create_sales_order_draft", "submit_sales_order",
            "create_purchase_order_draft", "submit_purchase_order", "create_direct_po",
            "send_purchase_order", "create_requisition", "approve_requisition",
            "convert_requisition", "create_expense_claim", "approve_expense_claim",
        ];
        for t in posts {
            assert_eq!(required_scopes(t), &["ledger:post"], "{t} must require ledger:post");
        }
        for t in writes {
            assert_eq!(required_scopes(t), &["erp:write"], "{t} must require erp:write");
        }
        // Reads stay reads — the default arm.
        for t in ["list_invoices", "get_dashboard", "run_report", "cit_estimate", "three_way_match"] {
            assert_eq!(required_scopes(t), &["erp:read"], "{t} should be a read");
        }
    }
}
