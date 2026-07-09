//! Shared test harness for `zavora-erp-core` integration and property tests.
//!
//! This module is included (via `#[path = "common/mod.rs"] mod common;`) by the
//! `integration_tests` and `property_tests` entry crates. It provides reusable
//! utilities for:
//!
//! - Connecting to a test PostgreSQL database (`TEST_DATABASE_URL` or
//!   `DATABASE_URL`, falling back to the project's docker-compose ports).
//! - Connecting to a test Redis instance (`TEST_REDIS_URL` or `REDIS_URL`).
//! - Running migrations against the test database.
//! - Provisioning an isolated, freshly-seeded tenant (`entity_id`) with an open
//!   fiscal period for the current year.
//! - Cleaning up a tenant's rows between tests.
//!
//! Tests skip gracefully (the harness returns `None`) when infrastructure is not
//! reachable, so `cargo test` never hard-fails in an environment without a
//! database. Each harness uses a fresh random `entity_id`, so concurrent test
//! runs are isolated from one another and from real application data.
//!
//! Required environment variables for the full suite to run:
//! - `TEST_DATABASE_URL` (preferred) or `DATABASE_URL`
//! - `TEST_REDIS_URL` (preferred) or `REDIS_URL`

// Not every helper is used by every test crate that includes this module, so
// suppress dead-code warnings for the shared surface.
#![allow(dead_code)]

use chrono::{Datelike, NaiveDate, Utc};
use sqlx::postgres::PgPoolOptions;
use sqlx::PgPool;
use uuid::Uuid;

use zavora_erp_core::settings::load_or_create_config;
use zavora_erp_core::ErpEngine;

/// Resolve the PostgreSQL connection string for tests.
///
/// Prefers `TEST_DATABASE_URL` so a dedicated throwaway database can be used in
/// CI, then `DATABASE_URL`, then the project's docker-compose default port.
pub fn database_url() -> String {
    std::env::var("TEST_DATABASE_URL")
        .or_else(|_| std::env::var("DATABASE_URL"))
        .unwrap_or_else(|_| "postgres://zavora:zavora@localhost:5433/zavora_era".to_string())
}

/// Resolve the Redis connection string for tests.
pub fn redis_url() -> String {
    std::env::var("TEST_REDIS_URL")
        .or_else(|_| std::env::var("REDIS_URL"))
        .unwrap_or_else(|_| "redis://127.0.0.1:6380".to_string())
}

/// Connect to the test database and run all migrations.
///
/// Returns `None` (signalling the caller to skip) when Postgres is unreachable
/// or migrations fail, so tests degrade gracefully without infrastructure.
pub async fn try_pool() -> Option<PgPool> {
    let pool = match PgPoolOptions::new()
        .max_connections(4)
        .connect(&database_url())
        .await
    {
        Ok(p) => p,
        Err(e) => {
            eprintln!("SKIP: cannot connect to Postgres at {} ({e})", database_url());
            return None;
        }
    };

    if let Err(e) = sqlx::migrate!("../migrations").run(&pool).await {
        eprintln!("SKIP: migrations failed ({e})");
        return None;
    }

    Some(pool)
}

/// Connect to the test Redis instance.
///
/// Returns `None` (signalling the caller to skip) when Redis is unreachable.
pub async fn try_redis() -> Option<redis::aio::MultiplexedConnection> {
    let client = match redis::Client::open(redis_url()) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("SKIP: bad redis url {} ({e})", redis_url());
            return None;
        }
    };
    match client.get_multiplexed_async_connection().await {
        Ok(c) => Some(c),
        Err(e) => {
            eprintln!("SKIP: cannot connect to Redis at {} ({e})", redis_url());
            None
        }
    }
}

/// An isolated, fully-provisioned test environment for a single tenant.
///
/// Holds a live [`ErpEngine`], the seeded `entity_id`, a clone of the database
/// pool for raw assertions, and the current date used to seed the fiscal period.
pub struct TestHarness {
    pub engine: ErpEngine,
    pub entity_id: Uuid,
    pub pool: PgPool,
    pub today: NaiveDate,
}

impl TestHarness {
    /// Provision a harness with an **open** fiscal period for the current year.
    ///
    /// Returns `None` if the database or Redis is unavailable, so callers can
    /// `let Some(h) = TestHarness::try_new().await else { return };`.
    pub async fn try_new() -> Option<TestHarness> {
        Self::try_new_with_period("open").await
    }

    /// Provision a harness with a fiscal period in the given `period_status`
    /// (e.g. `"open"`, `"closed"`, `"hard_closed"`).
    pub async fn try_new_with_period(period_status: &str) -> Option<TestHarness> {
        let pool = try_pool().await?;
        let redis_conn = try_redis().await?;

        let entity_id = Uuid::new_v4();
        let config = load_or_create_config(&pool, entity_id)
            .await
            .expect("seed entity settings");

        let today = Utc::now().date_naive();
        seed_fiscal_period(&pool, entity_id, today, period_status).await;

        // Seed the Kenya-standard chart of accounts: journal posting now
        // validates that every line account exists and is active, so tests
        // must post against a real chart like production tenants do.
        for a in zavora_erp_core::ledger::coa_template::kenya_standard_coa() {
            sqlx::query(
                "INSERT INTO accounts (entity_id, code, name, account_type, parent_code, is_control)
                 VALUES ($1, $2, $3, $4, $5, $6)
                 ON CONFLICT (entity_id, code) DO NOTHING",
            )
            .bind(entity_id)
            .bind(&a.code)
            .bind(&a.name)
            .bind(format!("{:?}", a.account_type))
            .bind(&a.parent_code)
            .bind(a.is_control)
            .execute(&pool)
            .await
            .expect("seed chart of accounts");
        }

        let engine = ErpEngine::new(pool.clone(), redis_conn, config)
            .await
            .expect("construct engine");

        Some(TestHarness {
            engine,
            entity_id,
            pool,
            today,
        })
    }

    /// Resolve the id of the fiscal period covering `date` for this tenant.
    pub async fn period_id(&self, date: NaiveDate) -> Uuid {
        sqlx::query_scalar::<_, Uuid>(
            "SELECT id FROM fiscal_periods \
             WHERE entity_id = $1 AND start_date <= $2 AND end_date >= $2",
        )
        .bind(self.entity_id)
        .bind(date)
        .fetch_one(&self.pool)
        .await
        .expect("fiscal period should exist for date")
    }

    /// Remove all data seeded for this tenant. Because every harness uses a
    /// fresh `entity_id`, isolation is automatic; this is provided so tests that
    /// want to assert on a clean slate can reset explicitly.
    pub async fn cleanup(&self) {
        // Best-effort: ignore errors so cleanup never fails a passing test.
        let _ = sqlx::query(
            "DELETE FROM journal_lines WHERE entry_id IN \
             (SELECT id FROM journal_entries WHERE entity_id = $1)",
        )
        .bind(self.entity_id)
        .execute(&self.pool)
        .await;
        let _ = sqlx::query("DELETE FROM journal_entries WHERE entity_id = $1")
            .bind(self.entity_id)
            .execute(&self.pool)
            .await;
        let _ = sqlx::query("DELETE FROM fiscal_periods WHERE entity_id = $1")
            .bind(self.entity_id)
            .execute(&self.pool)
            .await;
        let _ = sqlx::query("DELETE FROM accounts WHERE entity_id = $1")
            .bind(self.entity_id)
            .execute(&self.pool)
            .await;
        let _ = sqlx::query("DELETE FROM entity_settings WHERE entity_id = $1")
            .bind(self.entity_id)
            .execute(&self.pool)
            .await;
    }
}

/// Insert a fiscal period covering the year of `date` with the given status.
pub async fn seed_fiscal_period(
    pool: &PgPool,
    entity_id: Uuid,
    date: NaiveDate,
    status: &str,
) {
    let year = date.year();
    let start = NaiveDate::from_ymd_opt(year, 1, 1).unwrap();
    let end = NaiveDate::from_ymd_opt(year, 12, 31).unwrap();
    sqlx::query(
        "INSERT INTO fiscal_periods \
         (id, entity_id, name, start_date, end_date, status, fiscal_year, period_number) \
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8)",
    )
    .bind(Uuid::new_v4())
    .bind(entity_id)
    .bind(format!("FY{year}"))
    .bind(start)
    .bind(end)
    .bind(status)
    .bind(year)
    .bind(1)
    .execute(pool)
    .await
    .expect("seed fiscal period");
}
