//! Smoke property tests proving the `proptest` integration and shared harness
//! compile and run. Real domain property tests (JWT round-trip, journal balance
//! invariant, etc.) are added by later tasks alongside these scaffolding tests.

use proptest::prelude::*;
use rust_decimal::Decimal;
use zavora_erp_core::{round_money, round_paye};

proptest! {
    /// Rounding to 2dp is idempotent: rounding an already-rounded value is a
    /// no-op. This exercises the `proptest` harness without requiring any
    /// infrastructure.
    #[test]
    fn round_money_is_idempotent(mantissa in any::<i64>()) {
        let value = Decimal::from_i128_with_scale(mantissa as i128, 4);
        let once = round_money(value);
        let twice = round_money(once);
        prop_assert_eq!(once, twice);
    }

    /// PAYE rounding always yields a whole number of shillings (scale 0).
    #[test]
    fn round_paye_has_zero_scale(mantissa in any::<i64>()) {
        let value = Decimal::from_i128_with_scale(mantissa as i128, 2);
        let rounded = round_paye(value);
        prop_assert_eq!(rounded.scale(), 0);
    }
}

/// DB-backed smoke test: the shared harness can provision an isolated tenant
/// with an open fiscal period. Skips gracefully when infrastructure is absent.
#[tokio::test]
async fn harness_provisions_isolated_tenant() {
    let Some(harness) = crate::common::TestHarness::try_new().await else {
        return;
    };

    // The seeded tenant has a fiscal period covering today.
    let period_id = harness.period_id(harness.today).await;
    assert_ne!(period_id, uuid::Uuid::nil());

    harness.cleanup().await;
}
