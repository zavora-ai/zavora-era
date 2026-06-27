//! Integration tests for withholding-tax rate resolution.
//!
//! WHT rates are not hardcoded — they live in the `wht_rates` table (seeded by
//! migration 021) and are read via `services::wht::wht_rate_for`, which picks the
//! resident or non-resident column per category. These tests assert the seeded
//! Kenyan rates resolve correctly end-to-end against a live database.
//!
//! Skips gracefully when infrastructure is unavailable (`TestHarness::try_new`
//! returns `None`), matching the convention in this suite.

use rust_decimal_macros::dec;

use zavora_erp_core::services::wht::wht_rate_for;
use zavora_erp_core::types::WhtCategory;

use crate::common::TestHarness;

#[tokio::test]
async fn seeded_resident_rates_resolve() {
    let Some(h) = TestHarness::try_new().await else { return };

    // (category, expected resident rate) from migration 021.
    let cases = [
        (WhtCategory::Consultancy, dec!(0.05)),
        (WhtCategory::ManagementFees, dec!(0.05)),
        (WhtCategory::Rent, dec!(0.10)),
        (WhtCategory::Royalties, dec!(0.05)),
        (WhtCategory::Interest, dec!(0.15)),
        (WhtCategory::Contractual, dec!(0.03)),
        (WhtCategory::Dividends, dec!(0.05)),
        (WhtCategory::Insurance, dec!(0.05)),
        (WhtCategory::Transport, dec!(0.02)),
    ];

    for (category, expected) in cases {
        let rate = wht_rate_for(&h.engine, &category, true).await;
        assert_eq!(rate, expected, "resident rate for {category:?}");
    }

    h.cleanup().await;
}

#[tokio::test]
async fn seeded_non_resident_rates_resolve() {
    let Some(h) = TestHarness::try_new().await else { return };

    // Non-resident rates are higher (commonly 20%), with Rent at 30%.
    assert_eq!(wht_rate_for(&h.engine, &WhtCategory::Consultancy, false).await, dec!(0.20));
    assert_eq!(wht_rate_for(&h.engine, &WhtCategory::Rent, false).await, dec!(0.30));
    assert_eq!(wht_rate_for(&h.engine, &WhtCategory::Interest, false).await, dec!(0.15));
    assert_eq!(wht_rate_for(&h.engine, &WhtCategory::Dividends, false).await, dec!(0.15));

    h.cleanup().await;
}

#[tokio::test]
async fn resident_status_changes_the_rate() {
    let Some(h) = TestHarness::try_new().await else { return };

    // Same category, different residency → different rate (5% vs 20%).
    let resident = wht_rate_for(&h.engine, &WhtCategory::Consultancy, true).await;
    let non_resident = wht_rate_for(&h.engine, &WhtCategory::Consultancy, false).await;
    assert!(non_resident > resident);
    assert_eq!(resident, dec!(0.05));
    assert_eq!(non_resident, dec!(0.20));

    h.cleanup().await;
}
