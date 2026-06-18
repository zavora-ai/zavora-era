use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use std::collections::HashMap;
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
    /// Analytical dimensions ({ type_code: value_code }) carried to the GL.
    #[serde(default)]
    pub dimensions: HashMap<String, String>,
}

impl InvoiceLine {
    /// Calculate totals for this line, rounding each monetary result to 2 decimal
    /// places (banker's rounding). VAT is rounded per line before any summing, so
    /// document-level totals are the sum of already-rounded line VAT (Req 5.1, 5.2).
    pub fn compute_totals(&mut self) {
        use crate::money::round_money;
        let gross = self.quantity * self.unit_price;
        let discount = gross * self.discount_percent / Decimal::new(100, 0);
        self.line_total = round_money(gross - discount);
        self.vat_amount = round_money(self.line_total * self.vat_treatment.rate());
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
#[derive(Debug, Clone, serde::Serialize, FromRow)]
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
    #[serde(default)]
    pub dimensions: serde_json::Value,
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
    #[serde(default)]
    pub dimensions: Option<HashMap<String, String>>,
}
