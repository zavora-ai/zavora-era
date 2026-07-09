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
use zavora_erp_core::reporting::{ReportContent, ReportParameters, ReportRequest, ReportType};
use zavora_erp_core::types::AgentOrUserId;
use zavora_erp_core::ErpEngine;

fn empty_params() -> ReportParameters {
    ReportParameters {
        as_at: None,
        period_from: None,
        period_to: None,
        compare_to: None,
        comparative: None,
        customer_id: None,
        vendor_id: None,
        account_code: None,
        bank_account_id: None,
        statement_id: None,
        period_id: None,
        dimension_type: None,
    }
}

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

    // Journal posting validates that line accounts exist and are active, so
    // the test tenant needs a real chart like production tenants have.
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
        source_id: None,
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
        source_id: None,
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
        source_id: None,
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
        source_id: None,
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

/// Seed a minimal account so report classification (by account_type) has rows.
async fn seed_account(engine: &ErpEngine, entity_id: Uuid, code: &str, name: &str, account_type: &str) {
    sqlx::query(
        "INSERT INTO accounts (id, entity_id, code, name, account_type) VALUES ($1, $2, $3, $4, $5) \
         ON CONFLICT (entity_id, code) DO NOTHING",
    )
    .bind(Uuid::new_v4())
    .bind(entity_id)
    .bind(code)
    .bind(name)
    .bind(account_type)
    .execute(engine.pool())
    .await
    .unwrap();
}

/// Posting DR Asset / CR Revenue must leave both the trial balance and the
/// balance sheet in balance, with current-year earnings carrying the profit.
#[tokio::test]
async fn reports_balance_after_posting() {
    let Some((engine, entity_id, today)) = setup("open").await else { return };

    // The harness seeds the Kenya-standard chart: 1020 = Main Bank (Asset),
    // 5000 = Sales Revenue. (Template 4000 is a LIABILITY — don't post sales there.)

    let req = CreateJournalEntryRequest {
        date: today,
        source: JournalSource::Manual,
        source_id: None,
        reference: format!("RPT-{}", Uuid::new_v4()),
        description: "sale".to_string(),
        lines: vec![line("1020", Some(dec!(100.00)), None), line("5000", None, Some(dec!(100.00)))],
        post_immediately: true,
    };
    create_and_post(&engine, entity_id, req, period_id(&engine, entity_id, today).await, AgentOrUserId::User(Uuid::new_v4()))
        .await
        .expect("post");

    // Trial balance must balance.
    let tb = engine
        .run_report(ReportRequest { entity_id, report_type: ReportType::TrialBalance, parameters: empty_params() })
        .await
        .unwrap();
    match tb.content {
        ReportContent::TrialBalance(r) => {
            assert!(r.is_balanced, "trial balance must balance: diff {}", r.difference);
            assert_eq!(r.total_debits, dec!(100.00));
        }
        other => panic!("expected trial balance, got {other:?}"),
    }

    // Balance sheet must balance with profit in current-year earnings.
    let bs = engine
        .run_report(ReportRequest { entity_id, report_type: ReportType::BalanceSheet, parameters: empty_params() })
        .await
        .unwrap();
    match bs.content {
        ReportContent::BalanceSheet(r) => {
            assert!(r.is_balanced, "balance sheet must balance: diff {}", r.difference);
            assert_eq!(r.total_assets, dec!(100.00));
            assert_eq!(r.current_year_earnings, dec!(100.00));
            assert_eq!(r.total_assets, r.total_liabilities + r.total_equity);
        }
        other => panic!("expected balance sheet, got {other:?}"),
    }
}

/// The period-balance snapshots must reconcile exactly to the journal lines, and
/// an as-at trial balance built from snapshot + open tail (across a prior and a
/// current period) must still balance. Drift here would mean reports lie.
#[tokio::test]
async fn snapshots_reconcile_to_ledger() {
    let Some((engine, entity_id, today)) = setup("open").await else { return };
    // The harness seeds the Kenya-standard chart: 1020 = Main Bank (Asset),
    // 5000 = Sales Revenue. (Template 4000 is a LIABILITY — don't post sales there.)

    // A closed prior-year period (its end_date is in the past relative to today).
    let prior_end = NaiveDate::from_ymd_opt(today.year() - 1, 12, 31).unwrap();
    let prior_start = NaiveDate::from_ymd_opt(today.year() - 1, 1, 1).unwrap();
    let prior_id = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO fiscal_periods (id, entity_id, name, start_date, end_date, status, fiscal_year, period_number) \
         VALUES ($1,$2,$3,$4,$5,'open',$6,1)",
    )
    .bind(prior_id).bind(entity_id).bind(format!("FY{}", today.year() - 1))
    .bind(prior_start).bind(prior_end).bind(today.year() - 1)
    .execute(engine.pool()).await.unwrap();

    let prior_date = NaiveDate::from_ymd_opt(today.year() - 1, 6, 15).unwrap();
    let mk = |date: NaiveDate, amt| CreateJournalEntryRequest {
        date, source: JournalSource::Manual, source_id: None, reference: format!("REC-{}", Uuid::new_v4()),
        description: "x".to_string(),
        lines: vec![line("1020", Some(amt), None), line("5000", None, Some(amt))],
        post_immediately: true,
    };
    // One entry in the prior period (snapshot), one today (open tail).
    create_and_post(&engine, entity_id, mk(prior_date, dec!(70.00)), prior_id, AgentOrUserId::User(Uuid::new_v4())).await.expect("prior post");
    create_and_post(&engine, entity_id, mk(today, dec!(30.00)), period_id(&engine, entity_id, today).await, AgentOrUserId::User(Uuid::new_v4())).await.expect("current post");

    // Reconcile snapshots vs raw lines, per account.
    let mismatches: i64 = sqlx::query_scalar(
        r#"SELECT COUNT(*) FROM (
               SELECT account_code, SUM(debit_total) d, SUM(credit_total) c
               FROM account_period_balances WHERE entity_id = $1 GROUP BY account_code
           ) s FULL JOIN (
               SELECT account_code, SUM(COALESCE(functional_debit,0)) d, SUM(COALESCE(functional_credit,0)) c
               FROM journal_lines WHERE entity_id = $1 GROUP BY account_code
           ) l USING (account_code)
           WHERE s.d IS DISTINCT FROM l.d OR s.c IS DISTINCT FROM l.c"#,
    )
    .bind(entity_id)
    .fetch_one(engine.pool())
    .await
    .unwrap();
    assert_eq!(mismatches, 0, "snapshots must reconcile exactly to the ledger");

    // As-at trial balance (snapshot prior period + open tail today) must balance
    // and reflect both postings (70 + 30 = 100 on each side).
    let tb = engine
        .run_report(ReportRequest { entity_id, report_type: ReportType::TrialBalance, parameters: empty_params() })
        .await.unwrap();
    if let ReportContent::TrialBalance(r) = tb.content {
        assert!(r.is_balanced, "TB must balance: diff {}", r.difference);
        assert_eq!(r.total_debits, dec!(100.00));
    } else { panic!("expected trial balance"); }
}
