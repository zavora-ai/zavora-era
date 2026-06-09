use chrono::NaiveDate;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::line::CreateInvoiceLineRequest;

/// Request to create a credit note against an invoice.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateCreditNoteRequest {
    /// The invoice this credit note applies to.
    pub invoice_id: Uuid,
    /// Date of the credit note. Defaults to today.
    pub date: Option<NaiveDate>,
    /// Lines on the credit note. If empty, full reversal of original invoice.
    pub lines: Vec<CreateInvoiceLineRequest>,
    /// Reason for the credit note.
    pub reason: String,
    /// If true, refund to original payment method.
    pub refund: bool,
}

/// Result of creating a credit note.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreditNoteResult {
    /// The newly created credit note invoice (type = CreditNote).
    pub credit_note_id: Uuid,
    pub credit_note_number: String,
    pub amount: Decimal,
    /// The journal entry reversing the original.
    pub journal_entry_id: Uuid,
    /// Updated balance on the original invoice.
    pub original_new_balance: Decimal,
}
