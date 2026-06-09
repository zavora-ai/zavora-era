use chrono::{DateTime, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

use super::invoice::CreateInvoiceRequest;

/// Recurrence frequency.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum RecurrenceFreq {
    Weekly,
    Biweekly,
    Monthly,
    Quarterly,
    SemiAnnual,
    Annual,
}

impl RecurrenceFreq {
    /// Returns the number of days (approximate) for this frequency.
    pub fn approx_days(&self) -> u32 {
        match self {
            Self::Weekly => 7,
            Self::Biweekly => 14,
            Self::Monthly => 30,
            Self::Quarterly => 90,
            Self::SemiAnnual => 182,
            Self::Annual => 365,
        }
    }

    /// Compute next run date from a given date.
    pub fn next_date(&self, from: NaiveDate) -> NaiveDate {
        use chrono::Months;
        match self {
            Self::Weekly => from + chrono::Duration::weeks(1),
            Self::Biweekly => from + chrono::Duration::weeks(2),
            Self::Monthly => from + Months::new(1),
            Self::Quarterly => from + Months::new(3),
            Self::SemiAnnual => from + Months::new(6),
            Self::Annual => from + Months::new(12),
        }
    }
}

/// A recurring invoice template.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecurringInvoice {
    pub id: Uuid,
    pub entity_id: Uuid,
    pub customer_id: Uuid,
    pub template: CreateInvoiceRequest,
    pub frequency: RecurrenceFreq,
    pub start_date: NaiveDate,
    pub end_date: Option<NaiveDate>,
    pub next_run: NaiveDate,
    pub auto_send: bool,
    pub auto_charge: bool,
    pub last_run: Option<NaiveDate>,
    pub run_count: u32,
    pub is_active: bool,
    pub created_at: DateTime<Utc>,
}

impl RecurringInvoice {
    /// Check if this recurring invoice is due to run.
    pub fn is_due(&self, today: NaiveDate) -> bool {
        self.is_active && today >= self.next_run && self.end_date.map_or(true, |end| today <= end)
    }

    /// Advance to next run date after execution.
    pub fn advance(&mut self, today: NaiveDate) {
        self.last_run = Some(today);
        self.next_run = self.frequency.next_date(today);
        self.run_count += 1;
    }
}

/// Database row for recurring invoice.
#[derive(Debug, Clone, serde::Serialize, FromRow)]
pub struct RecurringInvoiceRow {
    pub id: Uuid,
    pub entity_id: Uuid,
    pub customer_id: Uuid,
    pub template: serde_json::Value,
    pub frequency: String,
    pub start_date: NaiveDate,
    pub end_date: Option<NaiveDate>,
    pub next_run: NaiveDate,
    pub auto_send: bool,
    pub auto_charge: bool,
    pub last_run: Option<NaiveDate>,
    pub run_count: i32,
    pub is_active: bool,
    pub created_at: DateTime<Utc>,
}

/// Request to create a recurring invoice.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateRecurringInvoiceRequest {
    pub customer_id: Uuid,
    pub template: CreateInvoiceRequest,
    pub frequency: RecurrenceFreq,
    pub start_date: NaiveDate,
    pub end_date: Option<NaiveDate>,
    pub auto_send: Option<bool>,
    pub auto_charge: Option<bool>,
}
