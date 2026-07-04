//! Externalized agent configuration: system prompt template (system.md),
//! operating rules (AGENTS.md), and MCP server config (mcp.json). All are
//! plain files next to the crate so Amos's capabilities can be customised
//! without recompiling; embedded copies keep the binary self-sufficient.

use std::collections::HashMap;
use std::path::PathBuf;
use tracing::{info, warn};

fn crate_file(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(name)
}

fn load_or_embedded(env_override: &str, file_name: &str, embedded: &'static str) -> String {
    let path = std::env::var(env_override).map(PathBuf::from).unwrap_or_else(|_| crate_file(file_name));
    match std::fs::read_to_string(&path) {
        Ok(text) => {
            info!("Loaded {file_name} from {}", path.display());
            text
        }
        Err(_) => {
            warn!("{file_name} not found at {} — using embedded copy", path.display());
            embedded.to_string()
        }
    }
}

/// The system prompt template (placeholders: {ui_url}, {skills_catalog}, {agents_rules}).
pub fn system_template() -> String {
    load_or_embedded("AMOS_SYSTEM_MD", "system.md", include_str!("../system.md"))
}

/// Operating rules appended into the prompt via {agents_rules}.
pub fn agents_rules() -> String {
    load_or_embedded("AMOS_AGENTS_MD", "AGENTS.md", include_str!("../AGENTS.md"))
}

/// Expand `${VAR}` placeholders from the process environment, falling back to
/// `defaults` and finally to an empty string (with a warning).
pub fn expand_env(text: &str, defaults: &HashMap<&str, String>) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(start) = rest.find("${") {
        out.push_str(&rest[..start]);
        let after = &rest[start + 2..];
        match after.find('}') {
            Some(end) => {
                let var = &after[..end];
                match std::env::var(var).ok().or_else(|| defaults.get(var).cloned()) {
                    Some(value) => out.push_str(&value),
                    None => warn!("mcp.json references ${{{var}}} but it is not set"),
                }
                rest = &after[end + 1..];
            }
            None => {
                out.push_str(&rest[start..]);
                rest = "";
            }
        }
    }
    out.push_str(rest);
    out
}

/// mcp.json (Kiro format) with `${VAR}` expansion. None when absent — the
/// caller falls back to the built-in server config.
pub fn mcp_config(defaults: &HashMap<&str, String>) -> Option<String> {
    let path = std::env::var("AMOS_MCP_JSON").map(PathBuf::from).unwrap_or_else(|_| crate_file("mcp.json"));
    match std::fs::read_to_string(&path) {
        Ok(text) => {
            info!("Loaded MCP config from {}", path.display());
            Some(expand_env(&text, defaults))
        }
        Err(_) => None,
    }
}
