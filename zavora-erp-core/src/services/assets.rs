use chrono::Utc;
use rust_decimal::Decimal;
use uuid::Uuid;

use crate::assets::*;
use crate::engine::ErpEngine;
use crate::error::ErpResult;
use crate::types::AgentOrUserId;

/// Create a fixed asset.
pub async fn create_asset(
    engine: &ErpEngine,
    req: CreateAssetRequest,
    _created_by: &AgentOrUserId,
) -> ErpResult<Uuid> {
    let id = Uuid::new_v4();
    let asset_number = format!("FA-{:06}", id.as_fields().0 % 1_000_000);
    let residual = req.residual_value.unwrap_or(Decimal::ZERO);
    let useful_life = req.useful_life_months.unwrap_or(60);
    let gl_asset = req.gl_asset_account.unwrap_or_else(|| "2500".to_string());
    let gl_accum = req.gl_accum_depr_account.unwrap_or_else(|| "2600".to_string());
    let gl_expense = req.gl_depr_expense.unwrap_or_else(|| "7600".to_string());

    sqlx::query(
        r#"INSERT INTO fixed_assets 
           (id, entity_id, asset_number, description, category, acquisition_date, cost, residual_value,
            useful_life_months, depreciation_method, accumulated_depreciation, net_book_value,
            gl_asset_account, gl_accum_depr_account, gl_depr_expense, status, created_at)
           VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, 0, $7, $11, $12, $13, 'active', $14)"#,
    )
    .bind(id)
    .bind(engine.entity_id())
    .bind(&asset_number)
    .bind(&req.description)
    .bind(serde_json::to_string(&req.category).unwrap_or_default())
    .bind(req.acquisition_date)
    .bind(req.cost)
    .bind(residual)
    .bind(useful_life as i32)
    .bind(serde_json::to_value(&req.depreciation_method).unwrap_or_default())
    .bind(&gl_asset)
    .bind(&gl_accum)
    .bind(&gl_expense)
    .bind(Utc::now())
    .execute(engine.pool())
    .await?;

    Ok(id)
}
