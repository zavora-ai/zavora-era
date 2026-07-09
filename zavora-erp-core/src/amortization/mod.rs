//! Amortisation schedules — prepayments and deferred revenue.

use chrono::{DateTime, NaiveDate, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// What the schedule releases over time.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AmortizationKind {
    /// Prepaid expense: DR expense / CR prepaid asset each period.
    PrepaidExpense,
    /// Deferred revenue: DR deferred-rev liability / CR revenue each period.
    DeferredRevenue,
}

impl AmortizationKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            AmortizationKind::PrepaidExpense => "prepaid_expense",
            AmortizationKind::DeferredRevenue => "deferred_revenue",
        }
    }
    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "prepaid_expense" => Some(AmortizationKind::PrepaidExpense),
            "deferred_revenue" => Some(AmortizationKind::DeferredRevenue),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateScheduleRequest {
    pub kind: AmortizationKind,
    pub description: String,
    /// Balance-sheet holding account (prepaid asset / deferred-revenue liability).
    pub balance_account: String,
    /// P&L account released into (expense for prepaid, revenue for deferred).
    pub pnl_account: String,
    pub total_amount: Decimal,
    pub periods: u32,
    pub start_date: NaiveDate,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct ScheduleRow {
    pub id: Uuid,
    pub entity_id: Uuid,
    pub kind: String,
    pub description: String,
    pub balance_account: String,
    pub pnl_account: String,
    pub total_amount: Decimal,
    pub periods: i32,
    pub start_date: NaiveDate,
    pub amortized_periods: i32,
    pub status: String,
    pub created_at: DateTime<Utc>,
}
