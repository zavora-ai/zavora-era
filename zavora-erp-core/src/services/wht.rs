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
