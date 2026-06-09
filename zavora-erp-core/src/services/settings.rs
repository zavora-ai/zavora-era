use chrono::Utc;

use crate::engine::ErpEngine;
use crate::error::ErpResult;
use crate::settings::*;
use crate::types::AgentOrUserId;

/// Get current settings.
pub async fn get_settings(engine: &ErpEngine) -> ErpResult<ErpConfig> {
    Ok(engine.config().clone())
}

/// Update settings — persists the patch to the database and returns the updated config.
pub async fn update_settings(
    engine: &ErpEngine,
    patch: SettingsPatch,
    updated_by: &AgentOrUserId,
) -> ErpResult<ErpConfig> {
    let mut config = engine.config().clone();
    let now = Utc::now();

    // Apply patch fields to in-memory config
    if let Some(base_currency) = &patch.base_currency {
        config.base_currency = base_currency.clone();
    }
    if let Some(fiscal_year_end) = &patch.fiscal_year_end {
        config.fiscal_year_end = fiscal_year_end.clone();
    }
    if let Some(branding) = &patch.branding {
        config.branding = branding.clone();
    }
    if let Some(tax_config) = &patch.tax_config {
        config.tax_config = tax_config.clone();
    }
    if let Some(payment_config) = &patch.payment_config {
        config.payment_config = payment_config.clone();
    }

    // Persist to database — update individual JSONB columns
    let branding_json = serde_json::to_value(&config.branding)?;
    let tax_config_json = serde_json::to_value(&config.tax_config)?;
    let payment_config_json = serde_json::to_value(&config.payment_config)?;
    let fiscal_year_end_str = serde_json::to_string(&config.fiscal_year_end)?;
    let updated_by_id = match updated_by {
        AgentOrUserId::User(id) => Some(*id),
        AgentOrUserId::Agent(_) => None,
    };

    sqlx::query(
        r#"UPDATE entity_settings 
           SET base_currency = $1,
               fiscal_year_end = $2,
               branding = $3,
               tax_config = $4,
               payment_config = $5,
               updated_at = $6,
               updated_by = $7
           WHERE entity_id = $8"#,
    )
    .bind(&config.base_currency)
    .bind(&fiscal_year_end_str)
    .bind(&branding_json)
    .bind(&tax_config_json)
    .bind(&payment_config_json)
    .bind(now)
    .bind(updated_by_id)
    .bind(engine.entity_id())
    .execute(engine.pool())
    .await?;

    Ok(config)
}
