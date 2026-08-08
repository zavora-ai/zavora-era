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

pub struct McpManagers {
    /// Interactive ERP + browser. ERP calls require a per-session delegated
    /// credential reference and cannot fall back to the service account.
    pub interactive: Arc<McpServerManager>,
    /// ERP-only manager for explicitly unattended routines.
    pub service: Arc<McpServerManager>,
}

fn configured_servers(showcase_dir: &std::path::Path) -> Result<HashMap<String, McpServerConfig>> {
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
    match crate::config::mcp_config(&defaults) {
        Some(json) => {
            let value: serde_json::Value = serde_json::from_str(&json)
                .map_err(|e| anyhow::anyhow!("invalid mcp.json: {e}"))?;
            let servers = value.get("mcpServers").and_then(serde_json::Value::as_object)
                .ok_or_else(|| anyhow::anyhow!("mcp.json must contain an mcpServers object"))?;
            servers.iter().map(|(id, config)| {
                serde_json::from_value(config.clone())
                    .map(|config| (id.clone(), config))
                    .map_err(|e| anyhow::anyhow!("invalid MCP server '{id}': {e}"))
            }).collect()
        }
        None => Ok(erp_server_config(showcase_dir)),
    }
}

async fn start(configs: HashMap<String, McpServerConfig>, name: &str) -> Result<Arc<McpServerManager>> {
    let manager = McpServerManager::new(configs).with_name(name.to_string());
    for (id, result) in manager.start_all().await {
        result.map_err(|e| anyhow::anyhow!("failed to start MCP server '{id}': {e}"))?;
    }
    Ok(Arc::new(manager))
}

/// Spawn an MCP child through `env -i` with a narrow allowlist. The ADK process
/// manager otherwise inherits Amos's entire environment into every MCP server,
/// including the browser.
fn isolate_child_environment(config: &mut McpServerConfig) {
    let original_command = std::mem::take(&mut config.command);
    let original_args = std::mem::take(&mut config.args);
    let mut allowed = config.env.drain().collect::<Vec<_>>();
    for key in ["PATH", "HOME", "USER", "TMPDIR", "RUST_LOG", "PLAYWRIGHT_BROWSERS_PATH", "NODE_EXTRA_CA_CERTS", "HTTP_PROXY", "HTTPS_PROXY", "NO_PROXY"] {
        if allowed.iter().all(|(present, _)| present != key) {
            if let Ok(value) = std::env::var(key) { allowed.push((key.to_string(), value)); }
        }
    }
    allowed.sort_by(|a, b| a.0.cmp(&b.0));
    config.command = "env".to_string();
    config.args = vec!["-i".to_string()];
    config.args.extend(allowed.into_iter().map(|(key, value)| format!("{key}={value}")));
    config.args.push(original_command);
    config.args.extend(original_args);
}

/// Start separate authorization domains for interactive users and unattended
/// routines. Keeping them in different child processes makes service-account
/// fallback impossible on a user-driven call.
pub async fn start_managers(showcase_dir: &std::path::Path) -> Result<McpManagers> {
    let base = configured_servers(showcase_dir)?;
    let base_erp = base.get("erp").cloned()
        .ok_or_else(|| anyhow::anyhow!("mcp.json must configure an 'erp' server"))?;

    let mut interactive = base;
    let interactive_erp = interactive.get_mut("erp").expect("checked above");
    interactive_erp.env.insert("MCP_ERP_AUTH_MODE".into(), "delegated".into());
    interactive_erp.env.insert("MCP_ERP_CREDENTIAL_DIR".into(), crate::credential::root()?.to_string_lossy().into_owned());
    interactive_erp.env.remove("ZAVORA_EMAIL");
    interactive_erp.env.remove("ZAVORA_PASSWORD");
    for config in interactive.values_mut() { isolate_child_environment(config); }

    let mut service_erp = base_erp;
    for key in ["ZAVORA_API_URL", "ZAVORA_EMAIL", "ZAVORA_PASSWORD"] {
        if let Ok(value) = std::env::var(key) { service_erp.env.insert(key.to_string(), value); }
    }
    service_erp.env.insert("MCP_ERP_AUTH_MODE".into(), "trusted-single-user".into());
    let service = HashMap::from([("erp".to_string(), service_erp)]);

    let (interactive, service) = tokio::try_join!(
        start(interactive, "amos-interactive-mcp"),
        start(service, "amos-service-mcp"),
    )?;
    Ok(McpManagers { interactive, service })
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

/// Exactly the named MCP tools — for routine sub-agents, whose surface is the
/// routine spec's own (browser-free) list, not the session allowlists.
pub async fn named_tools(
    manager: &McpServerManager,
    names: &std::collections::HashSet<String>,
) -> Result<Vec<Arc<dyn Tool>>> {
    let ctx: Arc<dyn ReadonlyContext> = Arc::new(AmosContext::new());
    let all = manager
        .tools(ctx)
        .await
        .map_err(|e| anyhow::anyhow!("failed to list MCP tools: {e}"))?;
    Ok(all.into_iter().filter(|t| names.contains(t.name())).collect())
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

#[cfg(test)]
mod security_tests {
    use super::*;

    #[test]
    fn interactive_child_environment_excludes_parent_secrets() {
        let mut config = McpServerConfig {
            command: "/opt/mcp-erp".into(),
            args: vec!["--stdio".into()],
            env: HashMap::from([("ZAVORA_API_URL".into(), "https://erp.test".into())]),
            ..Default::default()
        };
        isolate_child_environment(&mut config);

        assert_eq!(config.command, "env");
        assert!(config.env.is_empty());
        assert_eq!(config.args.first().map(String::as_str), Some("-i"));
        assert!(config.args.iter().any(|arg| arg == "ZAVORA_API_URL=https://erp.test"));
        assert!(config.args.iter().any(|arg| arg == "/opt/mcp-erp"));
        for secret in ["GOOGLE_API_KEY", "JWT_ACCESS_SECRET", "ZAVORA_PASSWORD", "DATABASE_URL"] {
            assert!(!config.args.iter().any(|arg| arg.starts_with(secret)), "{secret} leaked into child environment");
        }
    }
}
