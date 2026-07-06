//! CRM domain models (optional add-in). Row structs + request types for leads,
//! pipelines/stages, opportunities, activities, tickets, and the customer-portal
//! principal. Business logic lives in `crate::services::crm` (+ portal auth in
//! the API layer). All CRM behaviour is gated by `crm_settings.enabled`.

use chrono::{DateTime, NaiveDate, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

// ─── Settings (feature flag) ─────────────────────────────────────────────────

#[derive(Debug, Clone, FromRow, Serialize)]
pub struct CrmSettingsRow {
    pub entity_id: Uuid,
    pub enabled: bool,
    pub default_pipeline_id: Option<Uuid>,
    pub updated_at: DateTime<Utc>,
}

// ─── Pipelines & stages ──────────────────────────────────────────────────────

#[derive(Debug, Clone, FromRow, Serialize)]
pub struct PipelineRow {
    pub id: Uuid,
    pub entity_id: Uuid,
    pub name: String,
    pub is_default: bool,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, FromRow, Serialize)]
pub struct StageRow {
    pub id: Uuid,
    pub entity_id: Uuid,
    pub pipeline_id: Uuid,
    pub name: String,
    pub sort_order: i32,
    pub probability: Decimal,
    pub is_won: bool,
    pub is_lost: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreatePipelineRequest {
    pub name: String,
    #[serde(default)]
    pub is_default: bool,
    /// Optional stages to seed with the pipeline.
    #[serde(default)]
    pub stages: Vec<CreateStageRequest>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateStageRequest {
    pub name: String,
    #[serde(default)]
    pub sort_order: i32,
    #[serde(default)]
    pub probability: Decimal,
    #[serde(default)]
    pub is_won: bool,
    #[serde(default)]
    pub is_lost: bool,
}

// ─── Leads ───────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, FromRow, Serialize)]
pub struct LeadRow {
    pub id: Uuid,
    pub entity_id: Uuid,
    pub name: String,
    pub company: Option<String>,
    pub email: Option<String>,
    pub phone: Option<String>,
    pub source: Option<String>,
    pub status: String,
    pub rating: Option<String>,
    pub owner_user_id: Option<Uuid>,
    pub notes: Option<String>,
    pub converted_customer_id: Option<Uuid>,
    pub converted_opportunity_id: Option<Uuid>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateLeadRequest {
    pub name: String,
    #[serde(default)]
    pub company: Option<String>,
    #[serde(default)]
    pub email: Option<String>,
    #[serde(default)]
    pub phone: Option<String>,
    #[serde(default)]
    pub source: Option<String>,
    #[serde(default)]
    pub rating: Option<String>,
    #[serde(default)]
    pub owner_user_id: Option<Uuid>,
    #[serde(default)]
    pub notes: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct UpdateLeadRequest {
    #[serde(default)] pub name: Option<String>,
    #[serde(default)] pub company: Option<String>,
    #[serde(default)] pub email: Option<String>,
    #[serde(default)] pub phone: Option<String>,
    #[serde(default)] pub source: Option<String>,
    #[serde(default)] pub status: Option<String>,
    #[serde(default)] pub rating: Option<String>,
    #[serde(default)] pub owner_user_id: Option<Uuid>,
    #[serde(default)] pub notes: Option<String>,
}

/// Convert a lead into an opportunity (and optionally a customer account).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConvertLeadRequest {
    /// Create/link a customer account for the lead. If omitted, an opportunity
    /// is created without a customer link.
    #[serde(default = "default_true")]
    pub create_customer: bool,
    /// Existing customer to link instead of creating a new one.
    #[serde(default)]
    pub customer_id: Option<Uuid>,
    /// Opportunity to open on conversion (optional).
    #[serde(default)]
    pub opportunity_name: Option<String>,
    #[serde(default)]
    pub pipeline_id: Option<Uuid>,
    #[serde(default)]
    pub amount: Option<Decimal>,
}

// ─── Opportunities ───────────────────────────────────────────────────────────

#[derive(Debug, Clone, FromRow, Serialize)]
pub struct OpportunityRow {
    pub id: Uuid,
    pub entity_id: Uuid,
    pub name: String,
    pub pipeline_id: Uuid,
    pub stage_id: Uuid,
    pub customer_id: Option<Uuid>,
    pub lead_id: Option<Uuid>,
    pub amount: Decimal,
    pub currency: String,
    pub expected_close_date: Option<NaiveDate>,
    pub probability: Decimal,
    pub status: String,
    pub owner_user_id: Option<Uuid>,
    pub lost_reason: Option<String>,
    pub created_at: DateTime<Utc>,
    pub closed_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateOpportunityRequest {
    pub name: String,
    #[serde(default)]
    pub pipeline_id: Option<Uuid>,
    #[serde(default)]
    pub stage_id: Option<Uuid>,
    #[serde(default)]
    pub customer_id: Option<Uuid>,
    #[serde(default)]
    pub lead_id: Option<Uuid>,
    #[serde(default)]
    pub amount: Decimal,
    #[serde(default = "default_kes")]
    pub currency: String,
    #[serde(default)]
    pub expected_close_date: Option<NaiveDate>,
    #[serde(default)]
    pub owner_user_id: Option<Uuid>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MoveOpportunityRequest {
    pub stage_id: Uuid,
    #[serde(default)]
    pub note: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoseOpportunityRequest {
    #[serde(default)]
    pub reason: Option<String>,
}

// ─── Activities ──────────────────────────────────────────────────────────────

#[derive(Debug, Clone, FromRow, Serialize)]
pub struct ActivityRow {
    pub id: Uuid,
    pub entity_id: Uuid,
    pub kind: String,
    pub subject: String,
    pub notes: Option<String>,
    pub due_date: Option<DateTime<Utc>>,
    pub done: bool,
    pub done_at: Option<DateTime<Utc>>,
    pub related_type: Option<String>,
    pub related_id: Option<Uuid>,
    pub owner_user_id: Option<Uuid>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateActivityRequest {
    #[serde(default = "default_task")]
    pub kind: String,
    pub subject: String,
    #[serde(default)]
    pub notes: Option<String>,
    #[serde(default)]
    pub due_date: Option<DateTime<Utc>>,
    #[serde(default)]
    pub related_type: Option<String>,
    #[serde(default)]
    pub related_id: Option<Uuid>,
    #[serde(default)]
    pub owner_user_id: Option<Uuid>,
}

// ─── Tickets ─────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, FromRow, Serialize)]
pub struct TicketRow {
    pub id: Uuid,
    pub entity_id: Uuid,
    pub customer_id: Option<Uuid>,
    pub subject: String,
    pub description: Option<String>,
    pub status: String,
    pub priority: String,
    pub assigned_to_user_id: Option<Uuid>,
    pub created_by_customer_user_id: Option<Uuid>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, FromRow, Serialize)]
pub struct TicketMessageRow {
    pub id: Uuid,
    pub entity_id: Uuid,
    pub ticket_id: Uuid,
    pub author_kind: String,
    pub author_id: Option<Uuid>,
    pub body: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateTicketRequest {
    #[serde(default)]
    pub customer_id: Option<Uuid>,
    pub subject: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default = "default_normal")]
    pub priority: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TicketReplyRequest {
    pub body: String,
}

// ─── Customer portal principal ───────────────────────────────────────────────

#[derive(Debug, Clone, FromRow, Serialize)]
pub struct CustomerUserRow {
    pub id: Uuid,
    pub entity_id: Uuid,
    pub email: String,
    pub display_name: String,
    pub status: String,
    pub customer_id: Option<Uuid>,
    pub last_login: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
}

/// Self-onboarding registration from the customer portal.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CustomerRegisterRequest {
    pub display_name: String,
    pub email: String,
    #[serde(default)]
    pub company: Option<String>,
    #[serde(default)]
    pub phone: Option<String>,
    pub password: String,
}

/// Sales-assisted invite of a customer to the portal.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InviteCustomerRequest {
    pub email: String,
    #[serde(default)]
    pub customer_id: Option<Uuid>,
    #[serde(default)]
    pub display_name: Option<String>,
    #[serde(default)]
    pub password: Option<String>,
}

fn default_true() -> bool { true }
fn default_kes() -> String { "KES".to_string() }
fn default_task() -> String { "Task".to_string() }
fn default_normal() -> String { "Normal".to_string() }

/// CRM pipeline stage template seeded on first enable.
pub fn default_pipeline_stages() -> Vec<CreateStageRequest> {
    use rust_decimal_macros::dec;
    vec![
        CreateStageRequest { name: "Lead In".into(), sort_order: 1, probability: dec!(10), is_won: false, is_lost: false },
        CreateStageRequest { name: "Qualified".into(), sort_order: 2, probability: dec!(25), is_won: false, is_lost: false },
        CreateStageRequest { name: "Proposal".into(), sort_order: 3, probability: dec!(50), is_won: false, is_lost: false },
        CreateStageRequest { name: "Negotiation".into(), sort_order: 4, probability: dec!(75), is_won: false, is_lost: false },
        CreateStageRequest { name: "Won".into(), sort_order: 5, probability: dec!(100), is_won: true, is_lost: false },
        CreateStageRequest { name: "Lost".into(), sort_order: 6, probability: dec!(0), is_won: false, is_lost: true },
    ]
}
