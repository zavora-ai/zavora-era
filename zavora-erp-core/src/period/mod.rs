use chrono::{DateTime, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

use crate::types::AgentOrUserId;

/// State of a fiscal period.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum PeriodStatus {
    /// Not yet open — future period.
    Future,
    /// Transactions may be posted.
    Open,
    /// Prior-period adjustments allowed; normal posting blocked.
    SoftClosed,
    /// Immutable. Enforced by DB trigger.
    HardClosed,
}

/// A fiscal period (typically one month).
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct FiscalPeriod {
    pub id: Uuid,
    pub entity_id: Uuid,
    pub name: String,
    pub start_date: NaiveDate,
    pub end_date: NaiveDate,
    pub status: String, // maps to PeriodStatus
    pub fiscal_year: i32,
    pub period_number: i32, // 1-12
    pub closed_by: Option<serde_json::Value>,
    pub closed_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
}

impl FiscalPeriod {
    pub fn parsed_status(&self) -> PeriodStatus {
        match self.status.as_str() {
            "future" => PeriodStatus::Future,
            "open" => PeriodStatus::Open,
            "soft_closed" => PeriodStatus::SoftClosed,
            "hard_closed" => PeriodStatus::HardClosed,
            _ => PeriodStatus::Future,
        }
    }

    /// Returns true if posting is allowed in this period.
    pub fn allows_posting(&self) -> bool {
        matches!(self.parsed_status(), PeriodStatus::Open)
    }

    /// Returns true if prior-period adjustments are allowed.
    pub fn allows_adjustment(&self) -> bool {
        matches!(
            self.parsed_status(),
            PeriodStatus::Open | PeriodStatus::SoftClosed
        )
    }

    /// Check if a date falls within this period.
    pub fn contains_date(&self, date: NaiveDate) -> bool {
        date >= self.start_date && date <= self.end_date
    }
}

/// Request to generate fiscal periods for a year.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeneratePeriodsRequest {
    pub fiscal_year: i32,
    pub year_start_month: u32, // 1=Jan, 7=Jul, etc.
}

/// Request to close a period.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClosePeriodRequest {
    pub period_id: Uuid,
    pub close_type: PeriodCloseType,
    pub closed_by: AgentOrUserId,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum PeriodCloseType {
    Soft,
    Hard,
}

/// Request to reopen a soft-closed period.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReopenPeriodRequest {
    pub period_id: Uuid,
    pub reopened_by: AgentOrUserId,
    pub reason: String,
}
