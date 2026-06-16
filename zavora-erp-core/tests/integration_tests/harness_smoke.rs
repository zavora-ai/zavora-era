//! Smoke integration test proving the shared harness can connect, migrate, and
//! provision an isolated tenant. Domain integration tests (payment recording,
//! period close, FX revaluation) are added by later tasks using this harness.

use crate::common::TestHarness;

/// The harness connects, runs migrations, and seeds entity settings plus an
/// open fiscal period for an isolated tenant. Skips when infrastructure is
/// unavailable.
#[tokio::test]
async fn harness_connects_and_seeds_open_period() {
    let Some(harness) = TestHarness::try_new().await else {
        return;
    };

    // entity_settings row exists for the seeded tenant.
    let settings_exist: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM entity_settings WHERE entity_id = $1)",
    )
    .bind(harness.entity_id)
    .fetch_one(&harness.pool)
    .await
    .expect("query entity_settings");
    assert!(settings_exist, "harness should seed entity_settings");

    // The fiscal period covering today is open.
    let status: String = sqlx::query_scalar(
        "SELECT status FROM fiscal_periods \
         WHERE entity_id = $1 AND start_date <= $2 AND end_date >= $2",
    )
    .bind(harness.entity_id)
    .bind(harness.today)
    .fetch_one(&harness.pool)
    .await
    .expect("query fiscal period");
    assert_eq!(status, "open");

    harness.cleanup().await;
}

/// A second harness uses a distinct `entity_id`, demonstrating isolation.
#[tokio::test]
async fn harnesses_are_isolated_by_entity_id() {
    let Some(a) = TestHarness::try_new().await else {
        return;
    };
    let Some(b) = TestHarness::try_new().await else {
        return;
    };
    assert_ne!(a.entity_id, b.entity_id, "each harness must be isolated");

    a.cleanup().await;
    b.cleanup().await;
}
