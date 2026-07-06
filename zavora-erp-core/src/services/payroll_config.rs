//! Effective-dated statutory-config loader and seeder.
//!
//! Payroll resolves the statutory ruleset applicable to a pay period from
//! `payroll_statutory_config` (the row with the greatest `effective_from` on or
//! before the period), falling back to the built-in Finance Act 2024 default.
//! The default is seeded lazily per tenant so historical runs are reproducible.

use chrono::NaiveDate;
use uuid::Uuid;

use crate::engine::ErpEngine;
use crate::error::ErpResult;
use crate::payroll::config::StatutoryConfig;

/// Resolve the statutory config effective on `as_of` for a tenant. Falls back
/// to the built-in default when the tenant has no stored config.
pub async fn resolve(
    engine: &ErpEngine,
    entity_id: Uuid,
    as_of: NaiveDate,
) -> ErpResult<StatutoryConfig> {
    let row: Option<serde_json::Value> = sqlx::query_scalar(
        "SELECT config FROM payroll_statutory_config \
         WHERE entity_id = $1 AND effective_from <= $2 \
         ORDER BY effective_from DESC LIMIT 1",
    )
    .bind(entity_id)
    .bind(as_of)
    .fetch_optional(engine.pool())
    .await?;

    Ok(match row {
        Some(v) => serde_json::from_value(v).unwrap_or_default(),
        None => StatutoryConfig::finance_act_2024(),
    })
}

/// Seed the built-in default config for a tenant if it has none. Idempotent.
pub async fn ensure_seeded(engine: &ErpEngine, entity_id: Uuid) -> ErpResult<()> {
    let count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM payroll_statutory_config WHERE entity_id = $1")
            .bind(entity_id)
            .fetch_one(engine.pool())
            .await?;
    if count == 0 {
        let cfg = StatutoryConfig::finance_act_2024();
        sqlx::query(
            "INSERT INTO payroll_statutory_config (id, entity_id, effective_from, name, config) \
             VALUES ($1, $2, $3, $4, $5) ON CONFLICT (entity_id, effective_from) DO NOTHING",
        )
        .bind(Uuid::new_v4())
        .bind(entity_id)
        .bind(NaiveDate::from_ymd_opt(2024, 7, 1).unwrap())
        .bind(&cfg.name)
        .bind(serde_json::to_value(&cfg).unwrap_or_default())
        .execute(engine.pool())
        .await?;
    }
    Ok(())
}

/// Insert or update a tenant's statutory config for an effective date. Adding a
/// new `effective_from` creates a new version (historical runs stay reproducible);
/// reusing an existing one corrects that version.
pub async fn upsert(
    engine: &ErpEngine,
    entity_id: Uuid,
    effective_from: NaiveDate,
    cfg: StatutoryConfig,
    created_by: Option<Uuid>,
) -> ErpResult<()> {
    sqlx::query(
        "INSERT INTO payroll_statutory_config (id, entity_id, effective_from, name, config, created_by) \
         VALUES ($1, $2, $3, $4, $5, $6) \
         ON CONFLICT (entity_id, effective_from) \
         DO UPDATE SET name = EXCLUDED.name, config = EXCLUDED.config",
    )
    .bind(Uuid::new_v4())
    .bind(entity_id)
    .bind(effective_from)
    .bind(&cfg.name)
    .bind(serde_json::to_value(&cfg).unwrap_or_default())
    .bind(created_by)
    .execute(engine.pool())
    .await?;
    Ok(())
}

/// Row for listing/editing statutory configs (Phase 3 admin UI).
#[derive(Debug, Clone, serde::Serialize, sqlx::FromRow)]
pub struct StatutoryConfigRow {
    pub id: Uuid,
    pub entity_id: Uuid,
    pub effective_from: NaiveDate,
    pub name: String,
    pub config: serde_json::Value,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

/// List a tenant's statutory configs, newest effective first.
pub async fn list(engine: &ErpEngine, entity_id: Uuid) -> ErpResult<Vec<StatutoryConfigRow>> {
    let rows = sqlx::query_as::<_, StatutoryConfigRow>(
        "SELECT id, entity_id, effective_from, name, config, created_at \
         FROM payroll_statutory_config WHERE entity_id = $1 ORDER BY effective_from DESC",
    )
    .bind(entity_id)
    .fetch_all(engine.pool())
    .await?;
    Ok(rows)
}
