use chrono::Utc;
use uuid::Uuid;

use crate::engine::ErpEngine;
use crate::error::ErpResult;
use crate::settings::*;
use crate::types::AgentOrUserId;

/// Get current settings for a tenant.
pub async fn get_settings(engine: &ErpEngine, entity_id: Uuid) -> ErpResult<ErpConfig> {
    Ok((*engine.config_for(entity_id).await?).clone())
}

/// Update settings — persists the patch to the database and returns the updated config.
pub async fn update_settings(
    engine: &ErpEngine,
    entity_id: Uuid,
    patch: SettingsPatch,
    updated_by: &AgentOrUserId,
) -> ErpResult<ErpConfig> {
    let mut config = (*engine.config_for(entity_id).await?).clone();
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
    if let Some(posting) = &patch.posting {
        config.posting = posting.clone();
    }
    if let Some(sequences) = &patch.sequences {
        config.sequences = sequences.clone();
    }

    // Persist to database — update individual JSONB columns
    let branding_json = serde_json::to_value(&config.branding)?;
    let tax_config_json = serde_json::to_value(&config.tax_config)?;
    let payment_config_json = serde_json::to_value(&config.payment_config)?;
    let posting_json = serde_json::to_value(&config.posting)?;
    let sequences_json = serde_json::to_value(&config.sequences)?;
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
               posting_setup = $6,
               sequences = $7,
               updated_at = $8,
               updated_by = $9
           WHERE entity_id = $10"#,
    )
    .bind(&config.base_currency)
    .bind(&fiscal_year_end_str)
    .bind(&branding_json)
    .bind(&tax_config_json)
    .bind(&payment_config_json)
    .bind(&posting_json)
    .bind(&sequences_json)
    .bind(now)
    .bind(updated_by_id)
    .bind(entity_id)
    .execute(engine.pool())
    .await?;

    // Drop the cached config so the next access reloads the saved values, and
    // keep the legacy single-tenant posting accessor in sync for the startup entity.
    engine.invalidate_config(entity_id).await;
    if entity_id == engine.entity_id() {
        engine.set_posting(config.posting.clone());
    }

    Ok(config)
}
