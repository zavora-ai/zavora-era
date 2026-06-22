use axum::{
    response::IntoResponse,
    routing::{delete, get, post, put},
    Json, Router,
};
use sqlx::postgres::PgPoolOptions;
use std::sync::Arc;
use tower_http::cors::CorsLayer;
use tower_http::trace::TraceLayer;
use uuid::Uuid;

use zavora_erp_core::auth::{JwtConfig, DEFAULT_ACCESS_TTL_SECS, DEFAULT_REFRESH_TTL_SECS};
use zavora_erp_core::ErpEngine;

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

    let config = zavora_erp_core::settings::load_or_create_config(&pool, entity_id).await?;

    // Initialise the JWT auth layer (fails fast in production if secrets are missing).
    let jwt_config = load_jwt_config()?;
    middleware::auth::init_auth(jwt_config, entity_id);
    tracing::info!("Auth layer initialised for entity {}", entity_id);

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
            if let Err(e) = zavora_erp_core::services::scheduler::process_report_schedules(&scheduler_engine).await {
                tracing::error!("Report schedule error: {}", e);
            }
            if let Err(e) = zavora_erp_core::services::scheduler::process_recurring_journals(&scheduler_engine, scheduler_engine.entity_id()).await {
                tracing::error!("Recurring journal error: {}", e);
            }
        }
    });

    // Build router — public routes need no authentication.
    // NOTE: `#[allow(deprecated)]` covers the legacy `/api/v1/auth/register`
    // bootstrap route below. `register` is deprecated in favour of the
    // supported tenant-creation path `/api/v1/auth/signup` (Requirement 9.3);
    // it is kept wired for backward compatibility with single-tenant
    // deployments (Requirement 9.2).
    #[allow(deprecated)]
    let public = Router::new()
        .route("/health", get(health))
        .route("/api/v1/auth/login", post(routes::users::login))
        .route("/api/v1/auth/refresh", post(routes::users::refresh))
        // DEPRECATED: legacy single-tenant Owner bootstrap. Use
        // `/api/v1/auth/signup` to create new tenants. Retained unchanged for
        // backward compatibility (Requirement 9.2, 9.3).
        .route("/api/v1/auth/register", post(routes::users::register))
        // Public tenant signup — creates a new tenant + first Owner.
        // Supported path for creating new tenants (Requirement 9.3).
        .route("/api/v1/auth/signup", post(routes::auth_signup::signup))
        .route("/api/v1/auth/logout", post(routes::users::logout))
        // M-Pesa Daraja webhook (server-to-server; cannot carry a user JWT).
        .route("/api/v1/payments/mpesa-callback", post(routes::payments::mpesa_callback));

    // Protected routes — gated by the auth middleware applied below.
    let protected = Router::new()
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
        .route("/api/v1/journal-entries/{id}", get(routes::journal::get))
        .route("/api/v1/journal-entries/{id}/reverse", post(routes::journal::reverse))
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
        .route("/api/v1/invoices/{id}", get(routes::invoices::get_one).put(routes::invoices::update).delete(routes::invoices::delete))
        .route("/api/v1/invoices/{id}/post", post(routes::invoices::post_invoice))
        .route("/api/v1/invoices/{id}/send", post(routes::invoices::send))
        .route("/api/v1/invoices/{id}/write-off", post(routes::invoices::write_off))
        .route("/api/v1/invoices/{id}/credit-note", post(routes::invoices::create_credit_note))
        .route("/api/v1/invoices/{id}/etims-transmit", post(routes::invoices::etims_transmit))
        // Estimates
        .route("/api/v1/estimates", get(routes::estimates::list).post(routes::estimates::create))
        .route("/api/v1/estimates/{id}", get(routes::estimates::get_one))
        .route("/api/v1/estimates/{id}/convert", post(routes::estimates::convert))
        .route("/api/v1/estimates/{id}/send", post(routes::estimates::send))
        .route("/api/v1/estimates/{id}/accept", post(routes::estimates::accept))
        .route("/api/v1/estimates/{id}/decline", post(routes::estimates::decline))
        // Recurring Invoices
        .route("/api/v1/recurring-invoices", get(routes::invoices::list_recurring).post(routes::invoices::create_recurring))
        // Bills
        .route("/api/v1/bills", get(routes::bills::list).post(routes::bills::create))
        .route("/api/v1/bills/{id}", get(routes::bills::get_one).put(routes::bills::update).delete(routes::bills::delete))
        .route("/api/v1/bills/{id}/approve", post(routes::bills::approve))
        .route("/api/v1/bills/{id}/post", post(routes::bills::post_bill))

        // Supplier credit notes (AP)
        .route("/api/v1/supplier-credit-notes", get(routes::supplier_credit_notes::list).post(routes::supplier_credit_notes::create))
        .route("/api/v1/supplier-credit-notes/{id}", get(routes::supplier_credit_notes::get_one))
        // Payments
        .route("/api/v1/payments", get(routes::payments::list).post(routes::payments::record))
        .route("/api/v1/payments/apply", post(routes::payments::apply_unapplied))
        .route("/api/v1/payments/mpesa-stk-push", post(routes::payments::mpesa_stk_push))
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
        .route("/api/v1/bank/reconciliations", get(routes::reconciliation::list))
        .route("/api/v1/bank/reconciliations/compute", post(routes::reconciliation::compute))
        .route("/api/v1/bank/reconciliations/complete", post(routes::reconciliation::complete))
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
        .route("/api/v1/inventory/adjust", post(routes::inventory::adjust))
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
        .route("/api/v1/budgets", get(routes::budgets::list).put(routes::budgets::set))
        .route("/api/v1/custom-reports", get(routes::custom_reports::list).post(routes::custom_reports::save))
        .route("/api/v1/custom-reports/{id}", get(routes::custom_reports::get).delete(routes::custom_reports::delete))
        .route("/api/v1/custom-reports/{id}/run", get(routes::custom_reports::run))
        .route("/api/v1/consolidation/entities", get(routes::consolidation::my_entities))
        .route("/api/v1/consolidation/trial-balance", post(routes::consolidation::trial_balance))
        .route("/api/v1/report-schedules", get(routes::report_schedules::list).post(routes::report_schedules::save))
        .route("/api/v1/report-schedules/{id}", axum::routing::delete(routes::report_schedules::delete))
        .route("/api/v1/wht-rates", get(routes::wht::list).put(routes::wht::update))
        .route("/api/v1/tax-filings", get(routes::tax_filings::list).post(routes::tax_filings::file))
        .route("/api/v1/tax-filings/{id}/remit", post(routes::tax_filings::remit))
        .route("/api/v1/opening-balances", post(routes::onboarding::post_opening_balances))
        .route("/api/v1/recurring-journals", get(routes::recurring_journals::list).post(routes::recurring_journals::save))
        .route("/api/v1/recurring-journals/run", post(routes::recurring_journals::run_now))
        .route("/api/v1/recurring-journals/{id}", axum::routing::delete(routes::recurring_journals::delete))
        .route("/api/v1/dimensions", get(routes::dimensions::list))
        .route("/api/v1/dimension-types", post(routes::dimensions::create_type))
        .route("/api/v1/dimension-values", post(routes::dimensions::create_value))
        // Users (auth/* live on the public router)
        .route("/api/v1/users", get(routes::users::list).post(routes::users::create))
        .route("/api/v1/users/{id}", put(routes::users::update))
        // Every route above requires a valid access token.
        .route_layer(axum::middleware::from_fn(middleware::auth::require_authenticated));

    let app = public
        .merge(protected)
        .layer(TraceLayer::new_for_http())
        .layer(CorsLayer::permissive())
        .with_state(state);

    tracing::info!("Starting Zavora ERP API on {}", bind_addr);
    let listener = tokio::net::TcpListener::bind(&bind_addr).await?;
    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<std::net::SocketAddr>(),
    )
    .await?;

    Ok(())
}

/// Load JWT signing configuration.
///
/// In production (`APP_ENV=production`) the secrets are mandatory and the server
/// fails fast if they are missing (Req 9.4). Outside production we fall back to
/// fixed development secrets so `cargo run` works locally, with a loud warning.
fn load_jwt_config() -> anyhow::Result<JwtConfig> {
    let is_prod = std::env::var("APP_ENV")
        .map(|v| v.eq_ignore_ascii_case("production"))
        .unwrap_or(false);

    match JwtConfig::from_env() {
        Ok(cfg) => Ok(cfg),
        Err(e) if is_prod => Err(anyhow::anyhow!(
            "refusing to start in production without JWT secrets: {e}"
        )),
        Err(e) => {
            tracing::warn!(
                "Using INSECURE development JWT secrets ({e}). \
                 Set JWT_ACCESS_SECRET and JWT_REFRESH_SECRET before deploying."
            );
            Ok(JwtConfig::new(
                "dev-access-secret-not-for-production-use".to_string(),
                "dev-refresh-secret-not-for-production-use".to_string(),
                DEFAULT_ACCESS_TTL_SECS,
                DEFAULT_REFRESH_TTL_SECS,
            ))
        }
    }
}

async fn health() -> impl IntoResponse {
    Json(serde_json::json!({
        "status": "healthy",
        "service": "zavora-erp-api",
        "version": env!("CARGO_PKG_VERSION")
    }))
}

