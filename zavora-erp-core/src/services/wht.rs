//! Withholding-tax rate lookup.
//!
//! Rates live solely in the `wht_rates` table (single source of truth). There is
//! deliberately no hardcoded fallback: a rate that isn't configured yields 0, so
//! behaviour always matches what's stored — no silent divergence between code and
//! config.

use rust_decimal::Decimal;

use crate::engine::ErpEngine;
use crate::types::WhtCategory;

/// Stable storage key for a category (matches the seeded `wht_rates.category`).
fn category_key(category: &WhtCategory) -> &'static str {
    match category {
        WhtCategory::Consultancy => "Consultancy",
        WhtCategory::ManagementFees => "ManagementFees",
        WhtCategory::Rent => "Rent",
        WhtCategory::Royalties => "Royalties",
        WhtCategory::Interest => "Interest",
        WhtCategory::Contractual => "Contractual",
        WhtCategory::Dividends => "Dividends",
        WhtCategory::Insurance => "Insurance",
        WhtCategory::Transport => "Transport",
        WhtCategory::Other(_) => "Other",
    }
}

/// Effective WHT rate for a category from the `wht_rates` table. Returns 0 when
/// the category has no configured rate (no hardcoded statutory fallback).
pub async fn wht_rate_for(engine: &ErpEngine, category: &WhtCategory, resident: bool) -> Decimal {
    let key = category_key(category);
    let row: Option<(Decimal, Decimal)> = sqlx::query_as(
        "SELECT resident_rate, non_resident_rate FROM wht_rates WHERE category = $1",
    )
    .bind(key)
    .fetch_optional(engine.pool())
    .await
    .ok()
    .flatten();
    match row {
        Some((resident_rate, non_resident_rate)) => if resident { resident_rate } else { non_resident_rate },
        None => Decimal::ZERO,
    }
}

/// The statutory KRA WHT rates (resident, non-resident) — the single seed source
/// shared by migration 021 and the runtime backfill below. Keep in sync with the
/// migration; both use `ON CONFLICT DO NOTHING` so an admin's edits are preserved.
const STATUTORY_WHT_RATES: &[(&str, &str, &str)] = &[
    ("Consultancy", "0.05", "0.20"),
    ("ManagementFees", "0.05", "0.20"),
    ("Rent", "0.10", "0.30"),
    ("Royalties", "0.05", "0.20"),
    ("Interest", "0.15", "0.15"),
    ("Contractual", "0.03", "0.20"),
    ("Dividends", "0.05", "0.15"),
    ("Insurance", "0.05", "0.20"),
    ("Transport", "0.02", "0.20"),
    ("Other", "0.05", "0.20"),
];

/// Idempotently ensure the statutory WHT rates exist. Migration 021 seeds them
/// once, but because that INSERT is `ON CONFLICT DO NOTHING` it never re-runs —
/// so if the rows are ever lost (e.g. a restored/wiped volume) while the migration
/// ledger still shows 021 applied, every WHT lookup silently returns 0 and tax is
/// not withheld. Calling this at startup self-heals that: it re-inserts any
/// missing statutory category without overwriting an admin's customised rate.
pub async fn ensure_seeded(pool: &sqlx::PgPool) -> crate::error::ErpResult<()> {
    use std::str::FromStr;
    for (cat, res, non_res) in STATUTORY_WHT_RATES {
        sqlx::query(
            "INSERT INTO wht_rates (category, resident_rate, non_resident_rate) \
             VALUES ($1, $2, $3) ON CONFLICT (category) DO NOTHING",
        )
        .bind(cat)
        .bind(Decimal::from_str(res).unwrap_or_default())
        .bind(Decimal::from_str(non_res).unwrap_or_default())
        .execute(pool)
        .await
        .map_err(crate::error::ErpError::Database)?;
    }
    Ok(())
}
