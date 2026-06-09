use axum::{
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use sqlx::postgres::PgPoolOptions;
use std::sync::Arc;
use tower_http::cors::CorsLayer;
use tower_http::trace::TraceLayer;
use uuid::Uuid;

use zavora_erp_core::{ErpConfig, ErpEngine};

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

    // Create engine
    let engine = ErpEngine::new(pool, redis_conn, config).await?;

    let state = Arc::new(AppState { engine });

    // Build router
    let app = Router::new()
        // Health check
        .route("/health", get(health))
        // Dashboard
        .route("/api/v1/dashboard", get(routes::dashboard::summary))
        // Accounts
        .route("/api/v1/accounts", get(routes::accounts::list).post(routes::accounts::create))
        .route("/api/v1/accounts/{code}", get(routes::accounts::get).put(routes::accounts::update))
        // Periods
        .route("/api/v1/periods", get(routes::periods::list).post(routes::periods::generate))
        .route("/api/v1/periods/{id}/close", post(routes::periods::close))
        // Journal entries
        .route("/api/v1/journal-entries", post(routes::journal::create))
        .route("/api/v1/journal-entries/validate", post(routes::journal::validate))
        // Customers
        .route("/api/v1/customers", get(routes::parties::list_customers).post(routes::parties::create_customer))
        // Vendors
        .route("/api/v1/vendors", get(routes::parties::list_vendors).post(routes::parties::create_vendor))
        // Employees
        .route("/api/v1/employees", post(routes::parties::create_employee))
        // Products
        .route("/api/v1/products", post(routes::catalog::create_product))
        // Invoices
        .route("/api/v1/invoices", post(routes::invoices::create))
        .route("/api/v1/invoices/{id}/post", post(routes::invoices::post_invoice))
        .route("/api/v1/invoices/{id}/send", post(routes::invoices::send))
        // Bills
        .route("/api/v1/bills", post(routes::bills::create))
        .route("/api/v1/bills/{id}/approve", post(routes::bills::approve))
        // Payments
        .route("/api/v1/payments", post(routes::payments::record))
        .route("/api/v1/payments/mpesa-callback", post(routes::payments::mpesa_callback))
        // Payroll
        .route("/api/v1/payroll/run", post(routes::payroll::run))
        .route("/api/v1/payroll/{id}/approve", post(routes::payroll::approve))
        .route("/api/v1/payroll/{id}/post", post(routes::payroll::post_run))
        // Reports
        .route("/api/v1/reports", post(routes::reports::generate))
        // Agent API
        .route("/api/v1/agent/post", post(routes::agent::post_from_agent))
        .route("/api/v1/agent/report", post(routes::agent::run_report))
        // Settings
        .route("/api/v1/settings", get(routes::settings::get).put(routes::settings::update))
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

    Ok(ErpConfig {
        entity_id,
        base_currency: row.base_currency,
        fiscal_year_end,
        coa_template: zavora_erp_core::ledger::CoaTemplate::KenyaStandard,
        branding,
        sequences,
        tax_config,
        payment_config,
    })
}
