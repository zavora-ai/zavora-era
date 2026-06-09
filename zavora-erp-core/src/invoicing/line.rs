use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

use crate::types::{AccountCode, VatTreatment};

/// A line item on an invoice, estimate, or credit note.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InvoiceLine {
    pub id: Uuid,
    pub product_id: Option<Uuid>,
    pub description: String,
    pub quantity: Decimal,
    pub unit_price: Decimal,
    pub discount_percent: Decimal,
    pub account_code: AccountCode,
    pub vat_treatment: VatTreatment,
    pub line_total: Decimal,
    pub vat_amount: Decimal,
}

impl InvoiceLine {
    /// Calculate totals for this line.
    pub fn compute_totals(&mut self) {
        let gross = self.quantity * self.unit_price;
        let discount = gross * self.discount_percent / Decimal::new(100, 0);
        self.line_total = gross - discount;
        self.vat_amount = self.line_total * self.vat_treatment.rate();
    }
}

/// Tax summary line — one per VAT rate on a document.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaxLine {
    pub vat_treatment: VatTreatment,
    pub taxable_amount: Decimal,
    pub tax_amount: Decimal,
}

/// Database row for invoice lines.
#[derive(Debug, Clone, FromRow)]
pub struct InvoiceLineRow {
    pub id: Uuid,
    pub invoice_id: Uuid,
    pub product_id: Option<Uuid>,
    pub description: String,
    pub quantity: Decimal,
    pub unit_price: Decimal,
    pub discount_percent: Decimal,
    pub account_code: String,
    pub vat_treatment: String,
    pub line_total: Decimal,
    pub vat_amount: Decimal,
}

/// Request to create an invoice line.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateInvoiceLineRequest {
    pub product_id: Option<Uuid>,
    pub description: Option<String>,
    pub quantity: Decimal,
    pub unit_price: Option<Decimal>,
    pub discount_percent: Option<Decimal>,
    pub account_code: Option<AccountCode>,
    pub vat_treatment: Option<VatTreatment>,
}
