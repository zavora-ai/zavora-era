use chrono::{Datelike, NaiveDate, Utc};
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
    let posting = engine.posting_for(entity_id).await?;
    let gl_asset = req.gl_asset_account.unwrap_or_else(|| posting.fixed_asset.clone());
    let gl_accum = req.gl_accum_depr_account.unwrap_or_else(|| posting.accumulated_depreciation.clone());
    let gl_expense = req.gl_depr_expense.unwrap_or_else(|| posting.depreciation_expense.clone());

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

/// Last day of the month containing `d`.
fn month_end(d: NaiveDate) -> NaiveDate {
    let (y, m) = (d.year(), d.month());
    let first_next = if m == 12 {
        NaiveDate::from_ymd_opt(y + 1, 1, 1)
    } else {
        NaiveDate::from_ymd_opt(y, m + 1, 1)
    }
    .unwrap();
    first_next.pred_opt().unwrap()
}

/// First day of the month following `d`'s month.
fn next_month_start(d: NaiveDate) -> NaiveDate {
    month_end(d).succ_opt().unwrap()
}

/// Run depreciation for all active assets up to (and including) the month of
/// `as_of`, catching up any months not yet posted.
///
/// Idempotent: each asset tracks `depreciated_through`, so a month is never
/// depreciated twice. Catch-up: an asset acquired several months ago (or whose
/// runs were skipped) books every missing month in one call. Each month's
/// depreciation is posted into that month's fiscal period (catch-up stops at the
/// first month with no open period). The whole run is atomic.
///
/// Returns the IDs of all assets that were depreciated.
pub async fn run_depreciation(
    engine: &ErpEngine,
    entity_id: Uuid,
    as_of: NaiveDate,
    triggered_by: &AgentOrUserId,
) -> ErpResult<Vec<Uuid>> {
    let target = month_end(as_of);
    let base_ccy = engine.config_for(entity_id).await?.base_currency.clone();

    let rows = sqlx::query_as::<_, FixedAssetRow>(
        r#"SELECT * FROM fixed_assets
           WHERE entity_id = $1 AND status = 'active' AND net_book_value > residual_value"#,
    )
    .bind(entity_id)
    .fetch_all(engine.pool())
    .await?;

    // period_id -> (posting date = period end, lines)
    let mut per_period: std::collections::HashMap<Uuid, (NaiveDate, Vec<CreateJournalLineRequest>)> =
        Default::default();
    // pending asset updates (applied atomically with the journals)
    let mut updates: Vec<(Uuid, Decimal, Decimal, &'static str, NaiveDate)> = Vec::new();
    let mut depreciated_ids = Vec::new();

    for row in &rows {
        let mut asset = row_to_asset(row)?;
        // first month to book: the month after `depreciated_through`, else the
        // acquisition month (full-month convention in the month of acquisition).
        let mut cursor = match row.depreciated_through {
            Some(d) => next_month_start(d),
            None => asset.acquisition_date,
        };
        let mut new_accum = asset.accumulated_depreciation;
        let mut new_nbv = asset.net_book_value;
        let mut last_booked: Option<NaiveDate> = None;

        while month_end(cursor) <= target {
            if new_nbv <= asset.residual_value {
                break;
            }
            // declining-balance / KRA depend on the running NBV
            asset.net_book_value = new_nbv;
            let mut monthly = asset.monthly_depreciation();
            let cap = new_nbv - asset.residual_value;
            if monthly > cap {
                monthly = cap;
            }
            if monthly <= Decimal::ZERO {
                break;
            }

            let m_end = month_end(cursor);
            let period = match crate::services::periods::period_for_date(engine, entity_id, m_end).await {
                Ok(p) if p.allows_posting() => p,
                _ => break, // no open period for this month — stop catching up here
            };
            let bucket = per_period.entry(period.id).or_insert_with(|| (period.end_date, Vec::new()));
            bucket.1.push(CreateJournalLineRequest {
                account_code: asset.gl_depr_expense.clone(),
                debit: Some(monthly),
                credit: None,
                currency: base_ccy.clone(),
                fx_rate: Some(Decimal::ONE),
                description: Some(format!("Depreciation {} — {} ({})", m_end.format("%b %Y"), asset.description, asset.asset_number)),
                dimensions: None,
            });
            bucket.1.push(CreateJournalLineRequest {
                account_code: asset.gl_accum_depr_account.clone(),
                debit: None,
                credit: Some(monthly),
                currency: base_ccy.clone(),
                fx_rate: Some(Decimal::ONE),
                description: Some(format!("Accum depr {} — {} ({})", m_end.format("%b %Y"), asset.description, asset.asset_number)),
                dimensions: None,
            });

            new_accum += monthly;
            new_nbv -= monthly;
            last_booked = Some(m_end);
            cursor = next_month_start(cursor);
        }

        if let Some(through) = last_booked {
            let status = if new_nbv <= asset.residual_value { "fully_depreciated" } else { "active" };
            updates.push((asset.id, new_accum, new_nbv, status, through));
            depreciated_ids.push(asset.id);
        }
    }

    if per_period.is_empty() {
        return Ok(Vec::new());
    }

    // Atomic: post each period's journal and apply all asset updates together.
    let mut tx = engine.pool().begin().await?;
    for (period_id, (date, lines)) in per_period {
        let req = CreateJournalEntryRequest {
            date,
            source: JournalSource::Depreciation,
            source_id: None,
            reference: format!("DEPR-{}", date.format("%Y-%m")),
            description: format!("Depreciation for {}", date.format("%B %Y")),
            lines,
            post_immediately: true,
        };
        crate::services::journal::create_and_post_in_tx(&mut tx, engine, entity_id, req, period_id, triggered_by.clone()).await?;
    }
    for (id, accum, nbv, status, through) in &updates {
        sqlx::query(
            "UPDATE fixed_assets SET accumulated_depreciation=$1, net_book_value=$2, status=$3, depreciated_through=$4 WHERE id=$5",
        )
        .bind(accum)
        .bind(nbv)
        .bind(status)
        .bind(through)
        .bind(id)
        .execute(&mut *tx)
        .await?;
    }
    tx.commit().await?;

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
