use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

use crate::types::AgentOrUserId;

/// Types of audit events tracked by the system.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum AuditEventType {
    Created,
    Updated,
    Deleted,
    Posted,
    Reversed,
    Approved,
    Rejected,
    Sent,
    Viewed,
    Paid,
    PeriodClosed,
    PeriodReopened,
    Login,
    PermissionChanged,
    SettingsUpdated,
    Import,
    Export,
}

/// A single audit event recording a state change.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditEvent {
    pub id: Uuid,
    pub entity_id: Uuid,
    pub event_type: AuditEventType,
    pub object_type: String,
    pub object_id: Uuid,
    pub actor: AgentOrUserId,
    pub before: Option<serde_json::Value>,
    pub after: Option<serde_json::Value>,
    pub metadata: Option<serde_json::Value>,
    pub timestamp: DateTime<Utc>,
}

/// Database row for audit events.
#[derive(Debug, Clone, serde::Serialize, FromRow)]
pub struct AuditEventRow {
    pub id: Uuid,
    pub entity_id: Uuid,
    pub event_type: String,
    pub object_type: String,
    pub object_id: Uuid,
    pub actor: serde_json::Value,
    pub before_state: Option<serde_json::Value>,
    pub after_state: Option<serde_json::Value>,
    pub metadata: Option<serde_json::Value>,
    pub timestamp: DateTime<Utc>,
}

/// Request to query audit events.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditQuery {
    pub entity_id: Uuid,
    pub object_type: Option<String>,
    pub object_id: Option<Uuid>,
    pub actor: Option<AgentOrUserId>,
    pub event_type: Option<AuditEventType>,
    pub from: Option<DateTime<Utc>>,
    pub to: Option<DateTime<Utc>>,
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}

/// Paginated response for audit queries.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditEventPage {
    pub events: Vec<AuditEvent>,
    pub total: u64,
    pub limit: u32,
    pub offset: u32,
}
