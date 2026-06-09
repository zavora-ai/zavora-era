use chrono::{DateTime, NaiveDate, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

use crate::types::CurrencyCode;

use super::line::{CreateInvoiceLineRequest, InvoiceLine, TaxLine};

/// Status of an estimate/quote.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum EstimateStatus {
    Draft,
    Sent,
    Accepted,
    Declined,
    Expired,
    Converted,
}

/// An estimate / quote document.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Estimate {
    pub id: Uuid,
    pub entity_id: Uuid,
    pub number: String,
    pub customer_id: Uuid,
    pub issue_date: NaiveDate,
    pub expiry_date: NaiveDate,
    pub currency: CurrencyCode,
    pub fx_rate: Decimal,
    pub lines: Vec<InvoiceLine>,
    pub tax_lines: Vec<TaxLine>,
    pub subtotal: Decimal,
    pub tax_total: Decimal,
    pub gross_total: Decimal,
    pub status: EstimateStatus,
    pub converted_to: Option<Uuid>,
    pub notes: Option<String>,
    pub template_id: Option<Uuid>,
    pub created_at: DateTime<Utc>,
}

impl Estimate {
    /// Check if estimate is expired.
    pub fn is_expired(&self, today: NaiveDate) -> bool {
        today > self.expiry_date
            && !matches!(
                self.status,
                EstimateStatus::Accepted | EstimateStatus::Converted
            )
    }
}

/// Database row for estimate.
#[derive(Debug, Clone, FromRow)]
pub struct EstimateRow {
    pub id: Uuid,
    pub entity_id: Uuid,
    pub number: String,
    pub customer_id: Uuid,
    pub issue_date: NaiveDate,
    pub expiry_date: NaiveDate,
    pub currency: String,
    pub fx_rate: Decimal,
    pub subtotal: Decimal,
    pub tax_total: Decimal,
    pub gross_total: Decimal,
    pub status: String,
    pub converted_to: Option<Uuid>,
    pub notes: Option<String>,
    pub template_id: Option<Uuid>,
    pub created_at: DateTime<Utc>,
}

/// Request to create an estimate.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateEstimateRequest {
    pub customer_id: Uuid,
    pub issue_date: Option<NaiveDate>,
    pub expiry_date: Option<NaiveDate>,
    pub currency: Option<CurrencyCode>,
    pub lines: Vec<CreateInvoiceLineRequest>,
    pub notes: Option<String>,
    pub template_id: Option<Uuid>,
}

/// Request to convert an estimate to an invoice.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConvertEstimateRequest {
    pub estimate_id: Uuid,
    pub issue_date: Option<NaiveDate>,
    pub due_date: Option<NaiveDate>,
    pub send_immediately: Option<bool>,
}
