use chrono::{DateTime, NaiveDate, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

use crate::types::{AttachmentRef, CurrencyCode};

use super::line::{CreateInvoiceLineRequest, InvoiceLine, TaxLine};

/// Type of invoice document.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum InvoiceType {
    Invoice,
    CreditNote,
}

/// Status of an invoice through its lifecycle.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum InvoiceStatus {
    Draft,
    Sent,
    Viewed,
    PartiallyPaid,
    Paid,
    Overdue,
    Voided,
}

/// A complete invoice record.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Invoice {
    pub id: Uuid,
    pub entity_id: Uuid,
    pub number: String,
    pub invoice_type: InvoiceType,
    pub customer_id: Uuid,
    pub issue_date: NaiveDate,
    pub due_date: NaiveDate,
    pub currency: CurrencyCode,
    pub fx_rate: Decimal,
    pub lines: Vec<InvoiceLine>,
    pub tax_lines: Vec<TaxLine>,
    pub subtotal: Decimal,
    pub discount_total: Decimal,
    pub tax_total: Decimal,
    pub gross_total: Decimal,
    pub amount_paid: Decimal,
    pub balance_due: Decimal,
    pub status: InvoiceStatus,
    pub source_estimate: Option<Uuid>,
    pub credit_note_for: Option<Uuid>,
    pub journal_entry_id: Option<Uuid>,
    pub sent_at: Option<DateTime<Utc>>,
    pub viewed_at: Option<DateTime<Utc>>,
    pub paid_at: Option<DateTime<Utc>>,
    pub template_id: Option<Uuid>,
    pub notes: Option<String>,
    pub attachments: Vec<AttachmentRef>,
}

impl Invoice {
    /// Recalculate all totals from line items.
    pub fn recalculate(&mut self) {
        self.subtotal = self.lines.iter().map(|l| l.line_total).sum();
        self.tax_total = self.lines.iter().map(|l| l.vat_amount).sum();
        self.discount_total = self.lines.iter().map(|l| {
            let gross = l.quantity * l.unit_price;
            gross * l.discount_percent / Decimal::new(100, 0)
        }).sum();
        self.gross_total = self.subtotal + self.tax_total;
        self.balance_due = self.gross_total - self.amount_paid;

        // Rebuild tax lines by VAT treatment
        self.rebuild_tax_lines();
    }

    fn rebuild_tax_lines(&mut self) {
        use std::collections::HashMap;
        let mut tax_map: HashMap<String, (Decimal, Decimal)> = HashMap::new();
        for line in &self.lines {
            let key = serde_json::to_string(&line.vat_treatment).unwrap_or_default();
            let entry = tax_map.entry(key).or_insert((Decimal::ZERO, Decimal::ZERO));
            entry.0 += line.line_total;
            entry.1 += line.vat_amount;
        }
        self.tax_lines = tax_map
            .into_iter()
            .map(|(key, (taxable, tax))| TaxLine {
                vat_treatment: serde_json::from_str(&key).unwrap_or(crate::types::VatTreatment::Standard16),
                taxable_amount: taxable,
                tax_amount: tax,
            })
            .collect();
    }

    /// Check if this invoice is overdue.
    pub fn is_overdue(&self, today: NaiveDate) -> bool {
        self.balance_due > Decimal::ZERO
            && today > self.due_date
            && !matches!(self.status, InvoiceStatus::Paid | InvoiceStatus::Voided)
    }

    /// Update status based on payment state.
    pub fn update_payment_status(&mut self) {
        if self.balance_due <= Decimal::ZERO {
            self.status = InvoiceStatus::Paid;
            self.paid_at = Some(Utc::now());
        } else if self.amount_paid > Decimal::ZERO {
            self.status = InvoiceStatus::PartiallyPaid;
        }
    }
}

/// Database row for invoice header.
#[derive(Debug, Clone, serde::Serialize, FromRow)]
pub struct InvoiceRow {
    pub id: Uuid,
    pub entity_id: Uuid,
    pub number: String,
    pub invoice_type: String,
    pub customer_id: Uuid,
    pub issue_date: NaiveDate,
    pub due_date: NaiveDate,
    pub currency: String,
    pub fx_rate: Decimal,
    pub subtotal: Decimal,
    pub discount_total: Decimal,
    pub tax_total: Decimal,
    pub gross_total: Decimal,
    pub amount_paid: Decimal,
    pub balance_due: Decimal,
    pub status: String,
    pub source_estimate: Option<Uuid>,
    pub credit_note_for: Option<Uuid>,
    pub journal_entry_id: Option<Uuid>,
    pub sent_at: Option<DateTime<Utc>>,
    pub viewed_at: Option<DateTime<Utc>>,
    pub paid_at: Option<DateTime<Utc>>,
    pub template_id: Option<Uuid>,
    pub notes: Option<String>,
    pub created_at: DateTime<Utc>,
}

/// Request to create an invoice.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateInvoiceRequest {
    pub customer_id: Uuid,
    pub issue_date: Option<NaiveDate>,
    pub due_date: Option<NaiveDate>,
    pub currency: Option<CurrencyCode>,
    pub fx_rate: Option<Decimal>,
    pub lines: Vec<CreateInvoiceLineRequest>,
    pub template_id: Option<Uuid>,
    pub notes: Option<String>,
    pub send_immediately: Option<bool>,
}

/// Delivery channel for sending an invoice.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum InvoiceDeliveryChannel {
    Email,
    WhatsApp,
    Sms,
}

/// Request to send an invoice to a customer.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SendInvoiceRequest {
    pub invoice_id: Uuid,
    pub channels: Vec<InvoiceDeliveryChannel>,
    pub message: Option<String>,
}
