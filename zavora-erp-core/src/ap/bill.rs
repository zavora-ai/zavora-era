use chrono::{DateTime, NaiveDate, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

use crate::invoicing::line::{CreateInvoiceLineRequest, InvoiceLine, TaxLine};
use crate::types::CurrencyCode;

/// Status of a bill (AP document) through its lifecycle.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum BillStatus {
    Draft,
    PendingApproval,
    Approved,
    Posted,
    PartiallyPaid,
    Paid,
    Disputed,
    Cancelled,
}

/// A bill (accounts payable document) from a vendor.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Bill {
    pub id: Uuid,
    pub entity_id: Uuid,
    pub number: String,
    pub vendor_id: Uuid,
    pub vendor_invoice_number: Option<String>,
    pub issue_date: NaiveDate,
    pub due_date: NaiveDate,
    pub currency: CurrencyCode,
    pub fx_rate: Decimal,
    pub lines: Vec<InvoiceLine>,
    pub tax_lines: Vec<TaxLine>,
    pub subtotal: Decimal,
    pub tax_total: Decimal,
    pub wht_amount: Decimal,
    pub gross_total: Decimal,
    pub amount_paid: Decimal,
    pub balance_due: Decimal,
    pub status: BillStatus,
    pub journal_entry_id: Option<Uuid>,
    pub approved_by: Option<Uuid>,
    pub approved_at: Option<DateTime<Utc>>,
    pub notes: Option<String>,
    pub created_at: DateTime<Utc>,
}

impl Bill {
    /// Recalculate totals from lines.
    pub fn recalculate(&mut self) {
        self.subtotal = self.lines.iter().map(|l| l.line_total).sum();
        self.tax_total = self.lines.iter().map(|l| l.vat_amount).sum();
        self.gross_total = self.subtotal + self.tax_total - self.wht_amount;
        self.balance_due = self.gross_total - self.amount_paid;
    }

    /// Check if bill is overdue.
    pub fn is_overdue(&self, today: NaiveDate) -> bool {
        self.balance_due > Decimal::ZERO
            && today > self.due_date
            && !matches!(
                self.status,
                BillStatus::Paid | BillStatus::Cancelled | BillStatus::Disputed
            )
    }
}

/// Database row for bill.
#[derive(Debug, Clone, serde::Serialize, FromRow)]
pub struct BillRow {
    pub id: Uuid,
    pub entity_id: Uuid,
    pub number: String,
    pub vendor_id: Uuid,
    pub vendor_invoice_number: Option<String>,
    pub issue_date: NaiveDate,
    pub due_date: NaiveDate,
    pub currency: String,
    pub fx_rate: Decimal,
    pub subtotal: Decimal,
    pub tax_total: Decimal,
    pub wht_amount: Decimal,
    pub gross_total: Decimal,
    pub amount_paid: Decimal,
    pub balance_due: Decimal,
    pub status: String,
    pub journal_entry_id: Option<Uuid>,
    pub approved_by: Option<Uuid>,
    pub approved_at: Option<DateTime<Utc>>,
    pub notes: Option<String>,
    pub created_at: DateTime<Utc>,
}

/// Request to create a bill.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateBillRequest {
    pub vendor_id: Uuid,
    pub vendor_invoice_number: Option<String>,
    pub issue_date: Option<NaiveDate>,
    pub due_date: Option<NaiveDate>,
    pub currency: Option<CurrencyCode>,
    pub fx_rate: Option<Decimal>,
    pub lines: Vec<CreateInvoiceLineRequest>,
    pub notes: Option<String>,
}

/// Request to approve a bill.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApproveBillRequest {
    pub bill_id: Uuid,
    pub approved_by: Uuid,
}
