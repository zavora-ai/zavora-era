//! Amos's system instruction, assembled from externalized configuration:
//! `system.md` (template) + `AGENTS.md` (operating rules) + the skills catalog.

use crate::config;
use crate::state::AppState;

pub fn system_instruction(state: &AppState) -> String {
    config::system_template()
        .replace("{ui_url}", &state.erp_ui_url)
        .replace("{skills_catalog}", &state.skills.catalog_block())
        .replace("{agents_rules}", &config::agents_rules())
}
