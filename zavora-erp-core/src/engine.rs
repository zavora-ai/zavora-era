use std::collections::HashMap;
use std::sync::Arc;

use sqlx::PgPool;
use uuid::Uuid;

use crate::error::{ErpError, ErpResult};
use crate::ledger::journal::{
    CreateJournalEntryRequest, EntryStatus, JournalEntry, JournalLine, ValidationReport,
};
use crate::period::FiscalPeriod;
use crate::reporting::{DashboardSummary, ReportData, ReportRequest};
use crate::settings::ErpConfig;
use crate::types::AgentOrUserId;

/// The central ERP engine coordinator.
/// All public operations go through this struct.
pub struct ErpEngine {
    pool: PgPool,
    redis: tokio::sync::Mutex<redis::aio::MultiplexedConnection>,
    /// Startup/bootstrap entity config. Used by request-less paths (the
    /// background scheduler, the agentic API). Per-request work resolves the
    /// caller's tenant config via [`ErpEngine::config_for`].
    config: ErpConfig,
    /// Live posting setup for the startup entity (legacy single-tenant accessor).
    posting: std::sync::RwLock<crate::posting::PostingSetup>,
    /// Per-tenant configuration cache, keyed by `entity_id`. Populated lazily on
    /// first access and invalidated when a tenant's settings are saved. This is
    /// what makes the process genuinely multi-tenant: each request resolves its
    /// own tenant's currency, sequences, and posting setup.
    configs: tokio::sync::RwLock<HashMap<Uuid, Arc<ErpConfig>>>,
}

impl ErpEngine {
    /// Create a new ErpEngine instance.
    pub async fn new(
        pool: PgPool,
        redis: redis::aio::MultiplexedConnection,
        config: ErpConfig,
    ) -> ErpResult<Self> {
        // Pre-seed the per-tenant cache with the startup entity so the common
        // single-tenant deployment never pays a load on first request.
        let mut configs = HashMap::new();
        configs.insert(config.entity_id, Arc::new(config.clone()));

        Ok(Self {
            pool,
            redis: tokio::sync::Mutex::new(redis),
            posting: std::sync::RwLock::new(config.posting.clone()),
            config,
            configs: tokio::sync::RwLock::new(configs),
        })
    }

    /// Resolve a tenant's configuration, loading and caching it on first use.
    ///
    /// This is the multi-tenant replacement for [`ErpEngine::config`]: services
    /// pass the request's `entity_id` (from the verified JWT) and get that
    /// tenant's currency, sequences, tax, and posting setup — not the startup
    /// entity's.
    pub async fn config_for(&self, entity_id: Uuid) -> ErpResult<Arc<ErpConfig>> {
        if let Some(cfg) = self.configs.read().await.get(&entity_id) {
            return Ok(cfg.clone());
        }
        let cfg = Arc::new(crate::settings::load_or_create_config(&self.pool, entity_id).await?);
        self.configs.write().await.insert(entity_id, cfg.clone());
        Ok(cfg)
    }

    /// Resolve a tenant's posting setup (GL account determination).
    pub async fn posting_for(&self, entity_id: Uuid) -> ErpResult<crate::posting::PostingSetup> {
        Ok(self.config_for(entity_id).await?.posting.clone())
    }

    /// Drop a tenant's cached config so the next access reloads it. Called after
    /// that tenant's settings are saved.
    pub async fn invalidate_config(&self, entity_id: Uuid) {
        self.configs.write().await.remove(&entity_id);
    }

    /// Get a reference to the database pool.
    pub fn pool(&self) -> &PgPool {
        &self.pool
    }

    /// Get a clone of the Redis connection for async operations.
    pub async fn redis_conn(&self) -> redis::aio::MultiplexedConnection {
        self.redis.lock().await.clone()
    }

    /// Get the current configuration.
    pub fn config(&self) -> &ErpConfig {
        &self.config
    }

    /// Get the posting setup (GL account determination) for this entity.
    /// Returns a snapshot of the live setup, which may have been updated since
    /// startup via the settings API.
    pub fn posting(&self) -> crate::posting::PostingSetup {
        self.posting
            .read()
            .expect("posting setup lock poisoned")
            .clone()
    }

    /// Replace the live posting setup (called when settings are saved).
    pub fn set_posting(&self, posting: crate::posting::PostingSetup) {
        *self.posting.write().expect("posting setup lock poisoned") = posting;
    }

    /// Get the entity ID for this engine instance.
    pub fn entity_id(&self) -> Uuid {
        self.config.entity_id
    }

    /// Create a request-scoped handle bound to a specific tenant `entity_id`.
    ///
    /// Handlers build `engine.scoped(ctx.entity_id)` from the per-request
    /// `AuthContext` so data access is scoped to the verified token's tenant
    /// rather than the process-global `engine.entity_id()`. In legacy
    /// single-tenant mode `ctx.entity_id == served_entity()`, so behaviour is
    /// identical.
    pub fn scoped(&self, entity_id: Uuid) -> TenantScope<'_> {
        TenantScope {
            engine: self,
            entity_id,
        }
    }

    /// Reload configuration from the database.
    pub async fn reload_config(&mut self) -> ErpResult<()> {
        let row = sqlx::query_as::<_, crate::settings::SettingsRow>(
            "SELECT * FROM entity_settings WHERE entity_id = $1",
        )
        .bind(self.config.entity_id)
        .fetch_optional(&self.pool)
        .await?;

        if let Some(_row) = row {
            // Parse row into ErpConfig
            tracing::info!("Configuration reloaded for entity {}", self.config.entity_id);
        }
        Ok(())
    }

    // === Agentic Layer API Surface (spec section 27) ===

    /// Post a journal entry from the agentic layer.
    /// This is one of only two public entry points for agents.
    pub async fn post_from_agent(
        &self,
        req: PostingRequest,
    ) -> ErpResult<AgentPostingResult> {
        // 1. Validate the entry
        let validation = self.validate_entry(&req.entry).await?;
        if !validation.is_valid {
            return Err(ErpError::ValidationFailed {
                message: validation.errors.join("; "),
            });
        }

        // 2. Resolve period
        let period = self.resolve_period(req.entry.date).await?;
        if !period.allows_posting() {
            return Err(ErpError::PeriodClosed {
                period_id: period.id,
                date: req.entry.date,
            });
        }

        // 3. Create journal entry
        let entry = self.create_and_post_entry(req.entry, period.id, req.posted_by).await?;

        // 4. Generate summary
        let summary = format!(
            "Posted journal entry {} ({}) for {} on {}",
            entry.number, entry.source_description(), entry.reference, entry.date
        );

        Ok(AgentPostingResult {
            entry,
            validation_report: validation,
            natural_language_summary: summary,
        })
    }

    /// Run a report from the agentic layer.
    /// This is the second of two public entry points for agents.
    pub async fn run_report(&self, req: ReportRequest) -> ErpResult<ReportData> {
        // Delegate to the reporting module
        crate::services::reporting::generate_report(self, req).await
    }

    /// Get dashboard summary.
    pub async fn dashboard_summary(&self, entity_id: Uuid) -> ErpResult<DashboardSummary> {
        crate::services::reporting::dashboard_summary(self, entity_id).await
    }

    /// Validate a journal entry without posting.
    pub async fn validate_entry(
        &self,
        req: &CreateJournalEntryRequest,
    ) -> ErpResult<ValidationReport> {
        crate::services::journal::validate_entry(self, self.entity_id(), req).await
    }

    // === Internal helpers ===

    async fn resolve_period(
        &self,
        date: chrono::NaiveDate,
    ) -> ErpResult<FiscalPeriod> {
        let period = sqlx::query_as::<_, FiscalPeriod>(
            "SELECT * FROM fiscal_periods WHERE entity_id = $1 AND start_date <= $2 AND end_date >= $2",
        )
        .bind(self.config.entity_id)
        .bind(date)
        .fetch_optional(&self.pool)
        .await?
        .ok_or_else(|| ErpError::ValidationFailed {
            message: format!("No fiscal period found for date {}", date),
        })?;
        Ok(period)
    }

    async fn create_and_post_entry(
        &self,
        req: CreateJournalEntryRequest,
        period_id: Uuid,
        posted_by: AgentOrUserId,
    ) -> ErpResult<JournalEntry> {
        crate::services::journal::create_and_post(self, self.entity_id(), req, period_id, posted_by).await
    }
}

/// A request-scoped handle that binds an [`ErpEngine`] to a single tenant's
/// `entity_id` for the duration of a request.
///
/// This is the recommended low-risk shape for threading the per-request tenant
/// through the service layer: handlers construct `engine.scoped(ctx.entity_id)`
/// and pass the `TenantScope` where a `&ErpEngine` was previously used. It
/// forwards the engine's shared resources (`pool`, `redis`, `config`,
/// `posting`) while exposing the request tenant via [`TenantScope::entity_id`].
#[derive(Clone, Copy)]
pub struct TenantScope<'a> {
    engine: &'a ErpEngine,
    entity_id: Uuid,
}

impl<'a> TenantScope<'a> {
    /// The tenant this scope is bound to (the verified token's `entity_id`).
    pub fn entity_id(&self) -> Uuid {
        self.entity_id
    }

    /// The underlying engine, for operations not yet migrated to the scope.
    pub fn engine(&self) -> &'a ErpEngine {
        self.engine
    }

    /// Get a reference to the database pool (forwarded from the engine).
    pub fn pool(&self) -> &'a PgPool {
        self.engine.pool()
    }

    /// Get a clone of the Redis connection for async operations
    /// (forwarded from the engine).
    pub async fn redis(&self) -> redis::aio::MultiplexedConnection {
        self.engine.redis_conn().await
    }

    /// Get the current configuration (forwarded from the engine).
    pub fn config(&self) -> &'a ErpConfig {
        self.engine.config()
    }

    /// Get the posting setup (GL account determination) (forwarded from the engine).
    pub fn posting(&self) -> crate::posting::PostingSetup {
        self.engine.posting()
    }
}

/// Request from the agentic layer to post a journal entry.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PostingRequest {
    pub entry: CreateJournalEntryRequest,
    pub posted_by: AgentOrUserId,
}

/// Result of an agent posting operation.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct AgentPostingResult {
    pub entry: JournalEntry,
    pub validation_report: ValidationReport,
    pub natural_language_summary: String,
}

impl JournalEntry {
    /// Human-readable source description.
    pub fn source_description(&self) -> String {
        use crate::ledger::journal::JournalSource;
        match &self.source {
            JournalSource::Manual => "Manual entry".to_string(),
            JournalSource::Invoice => "Invoice".to_string(),
            JournalSource::CreditNote => "Credit note".to_string(),
            JournalSource::Bill => "Bill".to_string(),
            JournalSource::SupplierCreditNote => "Supplier credit note".to_string(),
            JournalSource::Payment => "Payment".to_string(),
            JournalSource::Payroll => "Payroll".to_string(),
            JournalSource::Depreciation => "Depreciation".to_string(),
            JournalSource::FxRevaluation => "FX revaluation".to_string(),
            JournalSource::InventoryAdjustment => "Inventory adjustment".to_string(),
            JournalSource::BankFee => "Bank fee".to_string(),
            JournalSource::OpeningBalance => "Opening balance".to_string(),
            JournalSource::YearEndClose => "Year-end close".to_string(),
            JournalSource::Agent(name) => format!("Agent: {}", name),
        }
    }
}
