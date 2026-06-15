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
    config: ErpConfig,
    /// Live posting setup (GL account determination). Held behind a lock so it
    /// can be updated at runtime when settings are saved, without restarting.
    posting: std::sync::RwLock<crate::posting::PostingSetup>,
}

impl ErpEngine {
    /// Create a new ErpEngine instance.
    pub async fn new(
        pool: PgPool,
        redis: redis::aio::MultiplexedConnection,
        config: ErpConfig,
    ) -> ErpResult<Self> {
        Ok(Self {
            pool,
            redis: tokio::sync::Mutex::new(redis),
            posting: std::sync::RwLock::new(config.posting.clone()),
            config,
        })
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
        crate::services::journal::validate_entry(self, req).await
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
        crate::services::journal::create_and_post(self, req, period_id, posted_by).await
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
