//! Integration tests for ledger-coupled flows (Requirement 4).
//!
//! These require a live PostgreSQL + Redis. They connect using `DATABASE_URL`
//! and `REDIS_URL` (falling back to the project's docker-compose ports) and skip
//! gracefully if the services are unavailable, so `cargo test` never hard-fails
//! in an environment without infrastructure.
//!
//! Each test isolates itself with a freshly generated `entity_id`, so runs do
//! not interfere with each other or with application data.

use chrono::{Datelike, NaiveDate, Utc};
use rust_decimal_macros::dec;
use sqlx::postgres::PgPoolOptions;
use sqlx::PgPool;
use uuid::Uuid;

use zavora_erp_core::ledger::journal::{
    CreateJournalEntryRequest, CreateJournalLineRequest, JournalSource,
};
use zavora_erp_core::services::journal::create_and_post;
use zavora_erp_core::types::AgentOrUserId;
use zavora_erp_core::ErpEngine;

fn database_url() -> String {
    std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgres://zavora:zavora@localhost:5433/zavora_era".to_string())
}

fn redis_url() -> String {
    std::env::var("REDIS_URL").unwrap_or_else(|_| "redis://127.0.0.1:6380".to_string())
}

/// Connect, run migrations, and seed a fresh entity with a fiscal period.
/// Returns `None` (test skips) if infrastructure is unavailable.
async fn setup(period_status: &str) -> Option<(ErpEngine, Uuid, NaiveDate)> {
    let pool: PgPool = match PgPoolOptions::new()
        .max_connections(2)
        .connect(&database_url())
        .await
    {
        Ok(p) => p,
        Err(e) => {
            eprintln!("SKIP: cannot connect to Postgres ({e})");
            return None;
        }
    };
    if sqlx::migrate!("../migrations").run(&pool).await.is_err() {
        eprintln!("SKIP: migrations failed");
        return None;
    }

    let redis_client = match redis::Client::open(redis_url()) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("SKIP: bad redis url ({e})");
            return None;
        }
    };
    let redis_conn = match redis_client.get_multiplexed_async_connection().await {
        Ok(c) => c,
        Err(e) => {
            eprintln!("SKIP: cannot connect to Redis ({e})");
            return None;
        }
    };

    let entity_id = Uuid::new_v4();
    let config = zavora_erp_core::settings::load_or_create_config(&pool, entity_id)
        .await
        .expect("seed settings");

    let today = Utc::now().date_naive();
    let year = today.year();
    let start = NaiveDate::from_ymd_opt(year, 1, 1).unwrap();
    let end = NaiveDate::from_ymd_opt(year, 12, 31).unwrap();
    sqlx::query(
        "INSERT INTO fiscal_periods (id, entity_id, name, start_date, end_date, status, fiscal_year, period_number) \
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8)",
    )
    .bind(Uuid::new_v4())
    .bind(entity_id)
    .bind(format!("FY{year}"))
    .bind(start)
    .bind(end)
    .bind(period_status)
    .bind(year)
    .bind(1)
    .execute(&pool)
    .await
    .expect("seed period");

    let engine = ErpEngine::new(pool, redis_conn, config).await.expect("engine");
    Some((engine, entity_id, today))
}

fn line(account: &str, debit: Option<rust_decimal::Decimal>, credit: Option<rust_decimal::Decimal>) -> CreateJournalLineRequest {
    CreateJournalLineRequest {
        account_code: account.to_string(),
        debit,
        credit,
        currency: "KES".to_string(),
        fx_rate: None,
        description: Some("test".to_string()),
        dimensions: None,
    }
}

#[tokio::test]
async fn balanced_entry_posts_and_persists_balanced() {
    let Some((engine, entity_id, today)) = setup("open").await else { return };

    let req = CreateJournalEntryRequest {
        date: today,
        source: JournalSource::Manual,
        reference: format!("TEST-{}", Uuid::new_v4()),
        description: "balanced".to_string(),
        lines: vec![line("1000", Some(dec!(100.00)), None), line("4000", None, Some(dec!(100.00)))],
        post_immediately: true,
    };

    let entry = create_and_post(&engine, entity_id, req, period_id(&engine, entity_id, today).await, AgentOrUserId::User(Uuid::new_v4()))
        .await
        .expect("post should succeed");

    // Verify persisted debits == credits.
    let (d, c): (rust_decimal::Decimal, rust_decimal::Decimal) = sqlx::query_as(
        "SELECT COALESCE(SUM(functional_debit),0), COALESCE(SUM(functional_credit),0) \
         FROM journal_lines WHERE entry_id = $1",
    )
    .bind(entry.id)
    .fetch_one(engine.pool())
    .await
    .unwrap();
    assert_eq!(d, c, "persisted journal must balance");
}

#[tokio::test]
async fn subcent_imbalance_gets_rounding_line() {
    let Some((engine, entity_id, today)) = setup("open").await else { return };

    // 100.00 dr vs 99.99 cr -> 0.01 residual, absorbed by a rounding line.
    let req = CreateJournalEntryRequest {
        date: today,
        source: JournalSource::Invoice,
        reference: format!("RND-{}", Uuid::new_v4()),
        description: "rounding".to_string(),
        lines: vec![line("1000", Some(dec!(100.00)), None), line("4000", None, Some(dec!(99.99)))],
        post_immediately: true,
    };
    let entry = create_and_post(&engine, entity_id, req, period_id(&engine, entity_id, today).await, AgentOrUserId::User(Uuid::new_v4()))
        .await
        .expect("post should succeed with rounding line");
    assert_eq!(entry.lines.len(), 3, "a rounding adjustment line should be appended");

    let (d, c): (rust_decimal::Decimal, rust_decimal::Decimal) = sqlx::query_as(
        "SELECT COALESCE(SUM(functional_debit),0), COALESCE(SUM(functional_credit),0) \
         FROM journal_lines WHERE entry_id = $1",
    )
    .bind(entry.id)
    .fetch_one(engine.pool())
    .await
    .unwrap();
    assert_eq!(d, c);
}

#[tokio::test]
async fn out_of_tolerance_entry_is_rejected() {
    let Some((engine, entity_id, today)) = setup("open").await else { return };

    let req = CreateJournalEntryRequest {
        date: today,
        source: JournalSource::Manual,
        reference: format!("BAD-{}", Uuid::new_v4()),
        description: "unbalanced".to_string(),
        lines: vec![line("1000", Some(dec!(100.00)), None), line("4000", None, Some(dec!(99.00)))],
        post_immediately: true,
    };
    let res = create_and_post(&engine, entity_id, req, period_id(&engine, entity_id, today).await, AgentOrUserId::User(Uuid::new_v4())).await;
    assert!(res.is_err(), "1.00 imbalance must be rejected");
}

#[tokio::test]
async fn posting_to_hard_closed_period_is_rejected() {
    let Some((engine, entity_id, today)) = setup("hard_closed").await else { return };

    let req = CreateJournalEntryRequest {
        date: today,
        source: JournalSource::Manual,
        reference: format!("CLOSED-{}", Uuid::new_v4()),
        description: "closed period".to_string(),
        lines: vec![line("1000", Some(dec!(10.00)), None), line("4000", None, Some(dec!(10.00)))],
        post_immediately: true,
    };
    let res = create_and_post(&engine, entity_id, req, period_id(&engine, entity_id, today).await, AgentOrUserId::User(Uuid::new_v4())).await;
    assert!(res.is_err(), "posting into a hard-closed period must be rejected");
}

async fn period_id(engine: &ErpEngine, entity_id: Uuid, date: NaiveDate) -> Uuid {
    sqlx::query_scalar::<_, Uuid>(
        "SELECT id FROM fiscal_periods WHERE entity_id = $1 AND start_date <= $2 AND end_date >= $2",
    )
    .bind(entity_id)
    .bind(date)
    .fetch_one(engine.pool())
    .await
    .unwrap()
}
