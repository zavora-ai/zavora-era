use axum::{
    response::IntoResponse,
    routing::{delete, get, post},
    Json, Router,
};
use sqlx::postgres::PgPoolOptions;
use std::sync::Arc;
use tower_http::cors::CorsLayer;
use tower_http::trace::TraceLayer;
use uuid::Uuid;

use zavora_erp_core::{ErpConfig, ErpEngine};

pub mod middleware;
mod routes;

/// Application state shared across handlers.
pub struct AppState {
    pub engine: ErpEngine,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "zavora_erp_api=debug,zavora_erp_core=debug,tower_http=info".into()),
        )
        .init();

    // Load environment
    dotenvy::dotenv().ok();

    let database_url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgres://zavora:zavora@localhost:5432/zavora_era".to_string());
    let redis_url =
        std::env::var("REDIS_URL").unwrap_or_else(|_| "redis://127.0.0.1:6379".to_string());
    let bind_addr = std::env::var("BIND_ADDR").unwrap_or_else(|_| "0.0.0.0:8080".to_string());

    // Database pool
    let pool = PgPoolOptions::new()
        .max_connections(20)
        .connect(&database_url)
        .await?;

    tracing::info!("Connected to database");

    // Run migrations
    sqlx::migrate!("../migrations").run(&pool).await?;
    tracing::info!("Migrations applied");

    // Redis connection
    let redis_client = redis::Client::open(redis_url)?;
    let redis_conn = redis_client.get_multiplexed_async_connection().await?;
    tracing::info!("Connected to Redis");

    // Load or create entity config
    let entity_id = std::env::var("ENTITY_ID")
        .ok()
        .and_then(|s| s.parse::<Uuid>().ok())
        .unwrap_or_else(Uuid::new_v4);

    let config = load_or_create_config(&pool, entity_id).await?;

    // Create scheduler engine (uses its own clones)
    let scheduler_pool = pool.clone();
    let scheduler_redis = redis_client.get_multiplexed_async_connection().await?;
    let scheduler_config = config.clone();

    // Create engine
    let engine = ErpEngine::new(pool, redis_conn, config).await?;

    let state = Arc::new(AppState { engine });

    // Spawn background scheduler
    let scheduler_engine = ErpEngine::new(scheduler_pool, scheduler_redis, scheduler_config).await?;
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(3600));
        loop {
            interval.tick().await;
            tracing::info!("Running scheduled tasks...");
            if let Err(e) = zavora_erp_core::services::scheduler::process_recurring_invoices(&scheduler_engine).await {
                tracing::error!("Recurring invoice error: {}", e);
            }
            if let Err(e) = zavora_erp_core::services::scheduler::process_invoice_reminders(&scheduler_engine).await {
                tracing::error!("Reminder scheduler error: {}", e);
            }
        }
    });

    // Build router
    let app = Router::new()
        // Health check
        .route("/health", get(health))
        // Dashboard
        .route("/api/v1/dashboard", get(routes::dashboard::summary))
        // Accounts
        .route("/api/v1/accounts", get(routes::accounts::list).post(routes::accounts::create))
        .route("/api/v1/accounts/{code}", get(routes::accounts::get).put(routes::accounts::update))
        .route("/api/v1/accounts/seed", post(routes::accounts::seed))
        // Periods
        .route("/api/v1/periods", get(routes::periods::list).post(routes::periods::generate))
        .route("/api/v1/periods/{id}/close", post(routes::periods::close))
        .route("/api/v1/periods/{id}/reopen", post(routes::periods::reopen))
        // Journal entries
        .route("/api/v1/journal-entries", get(routes::journal::list).post(routes::journal::create))
        .route("/api/v1/journal-entries/validate", post(routes::journal::validate))
        // Customers
        .route("/api/v1/customers", get(routes::parties::list_customers).post(routes::parties::create_customer))
        .route("/api/v1/customers/{id}", get(routes::parties::get_customer).put(routes::parties::update_customer))
        .route("/api/v1/customers/{id}/statement", get(routes::parties::customer_statement))
        // Vendors
        .route("/api/v1/vendors", get(routes::parties::list_vendors).post(routes::parties::create_vendor))
        .route("/api/v1/vendors/{id}", get(routes::parties::get_vendor).put(routes::parties::update_vendor))
        // Employees
        .route("/api/v1/employees", get(routes::parties::list_employees).post(routes::parties::create_employee))
        .route("/api/v1/employees/{id}", get(routes::parties::get_employee).put(routes::parties::update_employee))
        // Products
        .route("/api/v1/products", get(routes::catalog::list_products).post(routes::catalog::create_product))
        .route("/api/v1/products/{id}", get(routes::catalog::get_product).put(routes::catalog::update_product))
        // Invoices
        .route("/api/v1/invoices", get(routes::invoices::list).post(routes::invoices::create))
        .route("/api/v1/invoices/{id}", get(routes::invoices::get_one))
        .route("/api/v1/invoices/{id}/post", post(routes::invoices::post_invoice))
        .route("/api/v1/invoices/{id}/send", post(routes::invoices::send))
        .route("/api/v1/invoices/{id}/credit-note", post(routes::invoices::create_credit_note))
        // Estimates
        .route("/api/v1/estimates", get(routes::estimates::list).post(routes::estimates::create))
        .route("/api/v1/estimates/{id}", get(routes::estimates::get_one))
        .route("/api/v1/estimates/{id}/convert", post(routes::estimates::convert))
        // Recurring Invoices
        .route("/api/v1/recurring-invoices", get(routes::invoices::list_recurring).post(routes::invoices::create_recurring))
        // Bills
        .route("/api/v1/bills", get(routes::bills::list).post(routes::bills::create))
        .route("/api/v1/bills/{id}", get(routes::bills::get_one))
        .route("/api/v1/bills/{id}/approve", post(routes::bills::approve))
        .route("/api/v1/bills/{id}/post", post(routes::bills::post_bill))
        // Payments
        .route("/api/v1/payments", get(routes::payments::list).post(routes::payments::record))
        .route("/api/v1/payments/apply", post(routes::payments::apply_unapplied))
        .route("/api/v1/payments/mpesa-stk-push", post(routes::payments::mpesa_stk_push))
        .route("/api/v1/payments/mpesa-callback", post(routes::payments::mpesa_callback))
        // Transactions (categorisation queue)
        .route("/api/v1/transactions", get(routes::transactions::list))
        .route("/api/v1/transactions/{id}/categorise", post(routes::transactions::categorise))
        .route("/api/v1/transactions/{id}/split", post(routes::transactions::split))
        .route("/api/v1/transactions/merge", post(routes::transactions::merge))
        .route("/api/v1/transactions/{id}/exclude", post(routes::transactions::exclude))
        // Bank
        .route("/api/v1/bank-accounts", get(routes::bank::list_accounts).post(routes::bank::create_account))
        .route("/api/v1/bank-accounts/{id}", delete(routes::bank::delete_account))
        .route("/api/v1/bank/import", post(routes::bank::import_statement))
        .route("/api/v1/bank/reconcile/{id}", post(routes::bank::reconcile))
        .route("/api/v1/bank/confirm-match", post(routes::bank::confirm_match))
        // Payroll
        .route("/api/v1/payroll/run", post(routes::payroll::run))
        .route("/api/v1/payroll/{id}/approve", post(routes::payroll::approve))
        .route("/api/v1/payroll/{id}/post", post(routes::payroll::post_run))
        .route("/api/v1/payroll/{id}/paid", post(routes::payroll::mark_paid))
        // Inventory
        .route("/api/v1/inventory", get(routes::inventory::list).post(routes::inventory::create))
        .route("/api/v1/inventory/receive", post(routes::inventory::receive))
        .route("/api/v1/inventory/issue", post(routes::inventory::issue))
        // Assets
        .route("/api/v1/assets", get(routes::assets::list).post(routes::assets::create))
        .route("/api/v1/assets/depreciation/run", post(routes::assets::run_depreciation))
        // FX Rates
        .route("/api/v1/fx-rates", get(routes::fx::list).post(routes::fx::upsert))
        .route("/api/v1/fx/revaluation", post(routes::fx::revaluation))
        // Audit
        .route("/api/v1/audit", get(routes::audit::query))
        .route("/api/v1/audit/{object_type}/{object_id}", get(routes::audit::for_object))
        // Reports
        .route("/api/v1/reports", post(routes::reports::generate))
        .route("/api/v1/reports/export", post(routes::reports::export))
        // Receipts (OCR capture and confirm)
        .route("/api/v1/receipts/capture", post(routes::receipts::capture))
        .route("/api/v1/receipts/confirm", post(routes::receipts::confirm))
        // Agent API
        .route("/api/v1/agent/post", post(routes::agent::post_from_agent))
        .route("/api/v1/agent/report", post(routes::agent::run_report))
        // Settings
        .route("/api/v1/settings", get(routes::settings::get).put(routes::settings::update))
        // Auth & Users
        .route("/api/v1/auth/login", post(routes::users::login))
        .route("/api/v1/users", get(routes::users::list).post(routes::users::create))
        // Middleware
        .layer(TraceLayer::new_for_http())
        .layer(CorsLayer::permissive())
        .with_state(state);

    tracing::info!("Starting Zavora ERP API on {}", bind_addr);
    let listener = tokio::net::TcpListener::bind(&bind_addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}

async fn health() -> impl IntoResponse {
    Json(serde_json::json!({
        "status": "healthy",
        "service": "zavora-erp-api",
        "version": env!("CARGO_PKG_VERSION")
    }))
}

async fn load_or_create_config(
    pool: &sqlx::PgPool,
    entity_id: Uuid,
) -> anyhow::Result<ErpConfig> {
    use zavora_erp_core::settings::*;

    // Check if settings exist
    let exists = sqlx::query_scalar::<_, bool>(
        "SELECT EXISTS(SELECT 1 FROM entity_settings WHERE entity_id = $1)",
    )
    .bind(entity_id)
    .fetch_one(pool)
    .await?;

    if !exists {
        // Create default settings
        sqlx::query("INSERT INTO entity_settings (entity_id) VALUES ($1)")
            .bind(entity_id)
            .execute(pool)
            .await?;
    }

    // Load settings
    let row = sqlx::query_as::<_, SettingsRow>(
        "SELECT * FROM entity_settings WHERE entity_id = $1",
    )
    .bind(entity_id)
    .fetch_one(pool)
    .await?;

    let branding: BrandingConfig = serde_json::from_value(row.branding).unwrap_or_else(|_| BrandingConfig {
        company_name: "My Company".to_string(),
        logo_url: None,
        primary_color: "#1a56db".to_string(),
        secondary_color: None,
        font: "Inter".to_string(),
        footer_text: None,
        website: None,
        phone: None,
        email: None,
        address: None,
        kra_pin: None,
        vat_number: None,
    });

    let sequences: DocumentSequences =
        serde_json::from_value(row.sequences).unwrap_or_default();
    let tax_config: TaxConfig = serde_json::from_value(row.tax_config).unwrap_or_else(|_| TaxConfig {
        vat_registered: false,
        vat_number: None,
        vat_period: VatPeriod::Monthly,
        standard_vat_rate: rust_decimal::Decimal::new(16, 2),
        default_vat_treatment: zavora_erp_core::types::VatTreatment::Standard16,
        wht_enabled: true,
        paye_enabled: true,
    });
    let payment_config: PaymentConfig = serde_json::from_value(row.payment_config).unwrap_or_else(|_| PaymentConfig {
        mpesa_enabled: false,
        mpesa_paybill: None,
        mpesa_till_number: None,
        flutterwave_enabled: false,
        flutterwave_public_key: None,
        bank_transfer_enabled: true,
        default_bank_account_id: None,
    });

    let fiscal_year_end: MonthDay = serde_json::from_str(&row.fiscal_year_end)
        .unwrap_or(MonthDay { month: 12, day: 31 });

    // Posting setup: empty object falls back to code defaults.
    let posting: zavora_erp_core::PostingSetup =
        serde_json::from_value(row.posting_setup).unwrap_or_default();

    Ok(ErpConfig {
        entity_id,
        base_currency: row.base_currency,
        fiscal_year_end,
        coa_template: zavora_erp_core::ledger::CoaTemplate::KenyaStandard,
        branding,
        sequences,
        tax_config,
        payment_config,
        posting,
    })
}
