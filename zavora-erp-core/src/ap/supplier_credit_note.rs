use chrono::{DateTime, NaiveDate, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

use crate::invoicing::line::{CreateInvoiceLineRequest, InvoiceLine, TaxLine};

/// Status of an AP document (supplier credit note).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum ApDocStatus {
    Draft,
    Posted,
    Applied,
    Cancelled,
}

/// A supplier credit note — reduces or reverses a prior bill.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SupplierCreditNote {
    pub id: Uuid,
    pub entity_id: Uuid,
    pub vendor_id: Uuid,
    pub credit_note_number: String,
    pub credit_note_date: NaiveDate,
    pub applies_to_bill: Option<Uuid>,
    pub lines: Vec<InvoiceLine>,
    pub tax_lines: Vec<TaxLine>,
    pub gross_total: Decimal,
    pub status: ApDocStatus,
    pub journal_entry_id: Option<Uuid>,
    pub created_at: DateTime<Utc>,
}

/// Database row for supplier credit note.
#[derive(Debug, Clone, serde::Serialize, FromRow)]
pub struct SupplierCreditNoteRow {
    pub id: Uuid,
    pub entity_id: Uuid,
    pub vendor_id: Uuid,
    pub credit_note_number: String,
    pub credit_note_date: NaiveDate,
    pub applies_to_bill: Option<Uuid>,
    pub gross_total: Decimal,
    pub status: String,
    pub journal_entry_id: Option<Uuid>,
    pub created_at: DateTime<Utc>,
}

/// Request to create a supplier credit note.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateSupplierCreditNoteRequest {
    pub vendor_id: Uuid,
    pub credit_note_number: Option<String>,
    pub credit_note_date: Option<NaiveDate>,
    pub applies_to_bill: Option<Uuid>,
    pub lines: Vec<CreateInvoiceLineRequest>,
    pub reason: String,
    /// Document currency. Defaults to the vendor's currency when omitted.
    #[serde(default)]
    pub currency: Option<String>,
    /// FX rate to base currency. Defaults to 1.0 when omitted.
    #[serde(default)]
    pub fx_rate: Option<Decimal>,
}
