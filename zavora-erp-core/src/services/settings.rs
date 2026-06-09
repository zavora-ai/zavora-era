use crate::engine::ErpEngine;
use crate::error::ErpResult;
use crate::settings::*;
use crate::types::AgentOrUserId;

/// Get current settings.
pub async fn get_settings(engine: &ErpEngine) -> ErpResult<ErpConfig> {
    Ok(engine.config().clone())
}

/// Update settings.
pub async fn update_settings(
    engine: &ErpEngine,
    _patch: SettingsPatch,
    _updated_by: &AgentOrUserId,
) -> ErpResult<ErpConfig> {
    // TODO: apply patch and persist
    Ok(engine.config().clone())
}
