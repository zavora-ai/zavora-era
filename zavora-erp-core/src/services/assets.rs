use chrono::Utc;
use rust_decimal::Decimal;
use uuid::Uuid;

use crate::assets::*;
use crate::engine::ErpEngine;
use crate::error::ErpResult;
use crate::ledger::journal::{CreateJournalEntryRequest, CreateJournalLineRequest, JournalSource};
use crate::types::AgentOrUserId;

/// Create a fixed asset.
pub async fn create_asset(
    engine: &ErpEngine,
    entity_id: Uuid,
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
    .bind(entity_id)
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

/// Run monthly depreciation for all active assets.
///
/// For each active asset where net_book_value > residual_value:
/// 1. Computes monthly depreciation based on method (straight line or declining balance)
/// 2. Updates accumulated_depreciation and net_book_value on the asset
/// 3. Creates a journal entry: DR Depreciation Expense / CR Accumulated Depreciation
///
/// Returns the IDs of all assets that were depreciated.
pub async fn run_depreciation(
    engine: &ErpEngine,
    entity_id: Uuid,
    period_id: Uuid,
    triggered_by: &AgentOrUserId,
) -> ErpResult<Vec<Uuid>> {
    // Get the period for the journal entry date
    let period = crate::services::periods::get_period(engine, entity_id, period_id).await?;

    // Fetch all active assets that still have depreciable value
    let rows = sqlx::query_as::<_, FixedAssetRow>(
        r#"SELECT * FROM fixed_assets 
           WHERE entity_id = $1 
             AND status = 'active' 
             AND net_book_value > residual_value"#,
    )
    .bind(entity_id)
    .fetch_all(engine.pool())
    .await?;

    if rows.is_empty() {
        return Ok(Vec::new());
    }

    let base_ccy = engine.config().base_currency.clone();
    let mut depreciated_ids = Vec::new();
    let mut journal_lines = Vec::new();

    for row in &rows {
        // Reconstruct the asset for computation
        let asset = row_to_asset(row)?;
        let monthly_depr = asset.monthly_depreciation();

        if monthly_depr <= Decimal::ZERO {
            continue;
        }

        // Cap depreciation so NBV doesn't go below residual
        let max_depr = asset.net_book_value - asset.residual_value;
        let depr_amount = monthly_depr.min(max_depr);

        if depr_amount <= Decimal::ZERO {
            continue;
        }

        // Update the asset in the database
        let new_accum = asset.accumulated_depreciation + depr_amount;
        let new_nbv = asset.net_book_value - depr_amount;
        let new_status = if new_nbv <= asset.residual_value {
            "fully_depreciated"
        } else {
            "active"
        };

        sqlx::query(
            r#"UPDATE fixed_assets 
               SET accumulated_depreciation = $1, 
                   net_book_value = $2, 
                   status = $3
               WHERE id = $4"#,
        )
        .bind(new_accum)
        .bind(new_nbv)
        .bind(new_status)
        .bind(asset.id)
        .execute(engine.pool())
        .await?;

        // DR Depreciation Expense
        journal_lines.push(CreateJournalLineRequest {
            account_code: asset.gl_depr_expense.clone(),
            debit: Some(depr_amount),
            credit: None,
            currency: base_ccy.clone(),
            fx_rate: Some(Decimal::ONE),
            description: Some(format!("Depreciation: {} ({})", asset.description, asset.asset_number)),
            dimensions: None,
        });

        // CR Accumulated Depreciation
        journal_lines.push(CreateJournalLineRequest {
            account_code: asset.gl_accum_depr_account.clone(),
            debit: None,
            credit: Some(depr_amount),
            currency: base_ccy.clone(),
            fx_rate: Some(Decimal::ONE),
            description: Some(format!("Accum depr: {} ({})", asset.description, asset.asset_number)),
            dimensions: None,
        });

        depreciated_ids.push(asset.id);
    }

    if journal_lines.is_empty() {
        return Ok(Vec::new());
    }

    // Create a single consolidated journal entry for all depreciation
    let entry_req = CreateJournalEntryRequest {
        date: period.end_date,
        source: JournalSource::Depreciation,
        reference: format!("DEPR-{}", period.name),
        description: format!("Monthly depreciation for {}", period.name),
        lines: journal_lines,
        post_immediately: true,
    };

    crate::services::journal::create_and_post(
        engine,
        entity_id,
        entry_req,
        period_id,
        triggered_by.clone(),
    )
    .await?;

    Ok(depreciated_ids)
}

/// Convert a database row into a FixedAsset struct.
fn row_to_asset(row: &FixedAssetRow) -> ErpResult<FixedAsset> {
    let category: AssetCategory = serde_json::from_str(&row.category)
        .unwrap_or(AssetCategory::Other("Unknown".to_string()));
    let depreciation_method: DepreciationMethod =
        serde_json::from_value(row.depreciation_method.clone())
            .unwrap_or(DepreciationMethod::StraightLine);
    let status: AssetStatus = match row.status.as_str() {
        "active" => AssetStatus::Active,
        "fully_depreciated" => AssetStatus::FullyDepreciated,
        "disposed" => AssetStatus::Disposed,
        "written_off" => AssetStatus::WrittenOff,
        _ => AssetStatus::Active,
    };

    Ok(FixedAsset {
        id: row.id,
        entity_id: row.entity_id,
        asset_number: row.asset_number.clone(),
        description: row.description.clone(),
        category,
        acquisition_date: row.acquisition_date,
        cost: row.cost,
        residual_value: row.residual_value,
        useful_life_months: row.useful_life_months as u32,
        depreciation_method,
        accumulated_depreciation: row.accumulated_depreciation,
        net_book_value: row.net_book_value,
        gl_asset_account: row.gl_asset_account.clone(),
        gl_accum_depr_account: row.gl_accum_depr_account.clone(),
        gl_depr_expense: row.gl_depr_expense.clone(),
        status,
        disposal_date: row.disposal_date,
        disposal_proceeds: row.disposal_proceeds,
        created_at: row.created_at,
    })
}
