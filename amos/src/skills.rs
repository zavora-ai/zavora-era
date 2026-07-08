//! Skill pack loading (agentskills.io / SKILL.md standard, via adk-skill).
//!
//! Progressive disclosure: the system prompt carries only a catalog line per
//! skill; the `use_skill` tool loads a skill's full workflow body on demand,
//! keeping the realtime context lean.

use adk_skill::{SkillIndex, load_skill_index_with_extras};
use serde::Serialize;
use std::collections::HashSet;
use std::path::PathBuf;
use tracing::{info, warn};

pub struct SkillsCatalog {
    index: SkillIndex,
}

#[derive(Serialize)]
pub struct SkillSummary {
    pub name: String,
    pub description: String,
    pub allowed_tools: Vec<String>,
}

impl SkillsCatalog {
    pub fn load() -> Self {
        let dir = std::env::var("AMOS_SKILLS_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("skills"));
        // Root == the skills dir itself (its .skills/.claude subdirs won't
        // exist); the extras entry is what actually matches our layout.
        let index = match load_skill_index_with_extras(&dir, &[dir.clone()]) {
            Ok(index) => {
                let names: Vec<_> = index.skills().iter().map(|s| s.name.as_str()).collect();
                info!("Loaded {} skill(s) from {}: {}", index.len(), dir.display(), names.join(", "));
                index
            }
            Err(e) => {
                warn!("Skill discovery failed at {}: {e}", dir.display());
                SkillIndex::new(Vec::new())
            }
        };
        Self { index }
    }

    /// Level-1 disclosure: one catalog line per skill for the system prompt.
    pub fn catalog_block(&self) -> String {
        if self.index.is_empty() {
            return "(no skills installed)".to_string();
        }
        self.index
            .skills()
            .iter()
            .map(|s| format!("- {} — {}", s.name, s.description))
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// Level-2 disclosure: the full workflow body for `use_skill`.
    pub fn body_block(&self, name: &str) -> Option<String> {
        self.index.find_by_name(name).map(|s| {
            let mut block = s.engineer_prompt_block(24_000);
            if !s.allowed_tools.is_empty() {
                block.push_str(&format!("\n(Tools this skill uses: {})", s.allowed_tools.join(", ")));
            }
            block
        })
    }

    pub fn names(&self) -> Vec<String> {
        self.index.skills().iter().map(|s| s.name.clone()).collect()
    }

    pub fn summaries(&self) -> Vec<SkillSummary> {
        self.index
            .skills()
            .iter()
            .map(|s| SkillSummary {
                name: s.name.clone(),
                description: s.description.clone(),
                allowed_tools: s.allowed_tools.clone(),
            })
            .collect()
    }

    /// Union of every skill's allowed-tools — extends the MCP tool allowlist
    /// so installing a skill can unlock the tools it needs.
    pub fn extra_allowed_tools(&self) -> HashSet<String> {
        self.index
            .skills()
            .iter()
            .flat_map(|s| s.allowed_tools.iter().cloned())
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The installed skill pack parses and unlocks the tools the coverage map
    /// promises — a regression gate against a skill file that fails to parse
    /// (which would silently drop its tools from every session).
    #[test]
    fn skill_pack_loads_and_unlocks_coverage_tools() {
        let catalog = SkillsCatalog::load();
        let names = catalog.names();
        for expected in [
            "record-vendor-bill",
            "record-customer-invoice",
            "record-payment",
            "inventory-ops",
            "bank-reconciliation",
            "tax-filing",
            "month-end-review",
            "manage-procurement",
            "financial-reporting",
            "manual-journal",
            "hr-payroll",
            "crm",
            "payment-run",
            "erp-showcase",
        ] {
            assert!(names.iter().any(|n| n == expected), "skill '{expected}' missing from catalog: {names:?}");
        }
        let tools = catalog.extra_allowed_tools();
        for expected in [
            // AR + eTIMS
            "create_invoice_draft", "post_invoice", "etims_transmit_invoice",
            // Inventory
            "get_stock_levels", "adjust_stock", "transfer_stock",
            // Banking / period-end / statutory
            "compute_reconciliation", "complete_reconciliation", "close_period",
            "list_tax_filings", "file_tax_return", "remit_tax_filing",
            // Gap-closure: CIT, statement ingestion, AR outreach, assets/FX, payment runs
            "cit_estimate", "import_bank_statement", "send_customer_statement",
            "list_fixed_assets", "run_depreciation", "run_fx_revaluation",
        ] {
            assert!(tools.contains(expected), "tool '{expected}' not unlocked by any skill");
        }
    }
}
