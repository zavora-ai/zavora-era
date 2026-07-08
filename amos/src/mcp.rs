//! MCP server wiring: the Zavora ERP server (mcp-erp with the zavora backend)
//! and Playwright browser control, both spawned as stdio children and exposed
//! as `adk_core::Tool`s through `McpServerManager`.

use adk_core::{Content, ReadonlyContext, Tool, Toolset};
use adk_tool::mcp::manager::{McpServerConfig, McpServerManager};
use anyhow::Result;
use std::collections::HashMap;
use std::sync::Arc;

/// ERP tools Amos is allowed to use. Kept deliberately tight — Gemini Live
/// degrades with very large tool sets.
const ERP_TOOLS: &[&str] = &[
    "get_dashboard",
    "run_report",
    "list_bank_accounts",
    "list_accounts",
    "list_customers",
    "get_customer",
    "create_customer",
    "update_customer",
    "list_vendors",
    "get_vendor",
    "list_products",
    // AR — an accountant must be able to raise an invoice, not just read them.
    "list_invoices",
    "get_invoice",
    "create_invoice_draft",
    "submit_invoice",
    "post_invoice",
    "list_bills",
    "get_bill",
    "create_bill_draft",
    "post_bill",
    "list_payments",
    "record_payment",
    "post_journal_entry",
    "get_journal_entries",
    "list_employees",
    "list_fiscal_periods",
    "list_departments",
    "list_pay_runs",
    "get_pay_run",
    "run_payroll",
    "add_pay_run_input",
    "recompute_pay_run",
    "approve_pay_run",
    "post_pay_run",
    "mark_pay_run_paid",
    // KRA eTIMS — referenced by system.md; must be visible to the session.
    "etims_status",
    "etims_transmit_invoice",
];

/// Browser tools for the showcase. Everything needed to log in, navigate and
/// read the ERP UI; nothing destructive to the host machine.
const BROWSER_TOOLS: &[&str] = &[
    "browser_navigate",
    "browser_navigate_back",
    "browser_snapshot",
    "browser_click",
    "browser_type",
    "browser_fill_form",
    "browser_select_option",
    "browser_press_key",
    "browser_hover",
    "browser_wait_for",
    "browser_take_screenshot",
    "browser_tabs",
    "browser_resize",
];

/// Minimal ReadonlyContext for Toolset::tools() calls outside a live session.
pub struct AmosContext {
    user_content: Content,
}

impl AmosContext {
    pub fn new() -> Self {
        Self { user_content: Content::new("user").with_text("amos") }
    }
}

impl ReadonlyContext for AmosContext {
    fn invocation_id(&self) -> &str {
        "amos-invocation"
    }
    fn agent_name(&self) -> &str {
        "amos"
    }
    fn user_id(&self) -> &str {
        "amos-user"
    }
    fn app_name(&self) -> &str {
        "amos"
    }
    fn session_id(&self) -> &str {
        "amos-session"
    }
    fn branch(&self) -> &str {
        "main"
    }
    fn user_content(&self) -> &Content {
        &self.user_content
    }
}

fn erp_server_config(showcase_dir: &std::path::Path) -> HashMap<String, McpServerConfig> {
    let erp_bin = std::env::var("AMOS_MCP_ERP_BIN").unwrap_or_else(|_| {
        format!("{}/../../mcp-servers/mcp-erp/target/release/mcp-erp", env!("CARGO_MANIFEST_DIR"))
    });

    let mut erp_env = HashMap::new();
    for key in ["ZAVORA_API_URL", "ZAVORA_EMAIL", "ZAVORA_PASSWORD"] {
        if let Ok(v) = std::env::var(key) {
            erp_env.insert(key.to_string(), v);
        }
    }

    let headless = std::env::var("AMOS_BROWSER_HEADLESS").is_ok_and(|v| v == "1" || v == "true");
    let mut browser_args: Vec<String> = vec![
        "--yes".into(),
        "@playwright/mcp@latest".into(),
        // Ephemeral profile: never collide with the user's own browser or other
        // Playwright MCP instances holding the shared persistent profile.
        "--isolated".into(),
        "--viewport-size".into(),
        "1440,900".into(),
        "--output-dir".into(),
        showcase_dir.to_string_lossy().into_owned(),
    ];
    if headless {
        browser_args.push("--headless".into());
    }

    HashMap::from([
        (
            "erp".to_string(),
            McpServerConfig {
                command: erp_bin,
                args: vec![],
                env: erp_env,
                disabled: false,
                auto_approve: vec![],
                restart_policy: None,
            },
        ),
        (
            "browser".to_string(),
            McpServerConfig {
                command: "npx".to_string(),
                args: browser_args,
                env: HashMap::new(),
                disabled: false,
                auto_approve: vec![],
                restart_policy: None,
            },
        ),
    ])
}

/// Spawn the MCP servers from mcp.json (with `${VAR}` expansion), falling back
/// to the built-in config, and wait until their tools are visible.
pub async fn start_manager(showcase_dir: &std::path::Path) -> Result<Arc<McpServerManager>> {
    let defaults = HashMap::from([
        (
            "AMOS_MCP_ERP_BIN",
            format!("{}/../../mcp-servers/mcp-erp/target/release/mcp-erp", env!("CARGO_MANIFEST_DIR")),
        ),
        ("AMOS_SHOWCASE_DIR", showcase_dir.to_string_lossy().into_owned()),
        ("ZAVORA_API_URL", "http://localhost:8080".to_string()),
        // Pin in production images so npx never re-resolves at boot.
        ("AMOS_PLAYWRIGHT_MCP_VERSION", "latest".to_string()),
        // Dev machines have Google Chrome; the container ships playwright's
        // chromium (the image sets AMOS_BROWSER_CHANNEL=chromium).
        ("AMOS_BROWSER_CHANNEL", "chrome".to_string()),
    ]);
    let manager = match crate::config::mcp_config(&defaults) {
        Some(json) => McpServerManager::from_json(&json)
            .map_err(|e| anyhow::anyhow!("invalid mcp.json: {e}"))?,
        None => McpServerManager::new(erp_server_config(showcase_dir)),
    }
    .with_name("amos-mcp".to_string());
    for (id, result) in manager.start_all().await {
        result.map_err(|e| anyhow::anyhow!("failed to start MCP server '{id}': {e}"))?;
    }
    Ok(Arc::new(manager))
}

/// All tools Amos may use: the built-in allowlists plus anything an installed
/// skill declares in its `allowed-tools` (so new skills can unlock tools).
pub async fn agent_tools(
    manager: &McpServerManager,
    skill_allowed: &std::collections::HashSet<String>,
) -> Result<Vec<Arc<dyn Tool>>> {
    let ctx: Arc<dyn ReadonlyContext> = Arc::new(AmosContext::new());
    let all = manager
        .tools(ctx)
        .await
        .map_err(|e| anyhow::anyhow!("failed to list MCP tools: {e}"))?;
    Ok(all
        .into_iter()
        .filter(|t| {
            ERP_TOOLS.contains(&t.name())
                || BROWSER_TOOLS.contains(&t.name())
                || skill_allowed.contains(t.name())
        })
        .collect())
}

/// Look up a single tool by name (used by the native showcase_step tool to
/// drive the browser screenshot internally).
pub async fn find_tool(manager: &McpServerManager, name: &str) -> Option<Arc<dyn Tool>> {
    let ctx: Arc<dyn ReadonlyContext> = Arc::new(AmosContext::new());
    manager
        .tools(ctx)
        .await
        .ok()?
        .into_iter()
        .find(|t| t.name() == name)
}
