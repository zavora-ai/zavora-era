use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// M-Pesa Daraja STK Push callback data.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MpesaCallback {
    pub merchant_request_id: String,
    pub checkout_request_id: String,
    pub result_code: i32,
    pub result_desc: String,
    pub amount: Option<Decimal>,
    pub mpesa_receipt_number: Option<String>,
    pub transaction_date: Option<DateTime<Utc>>,
    pub phone_number: Option<String>,
}

impl MpesaCallback {
    /// Returns true if the M-Pesa transaction was successful.
    pub fn is_success(&self) -> bool {
        self.result_code == 0
    }
}

/// Request to initiate an M-Pesa STK Push for an invoice.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MpesaStkPushRequest {
    pub invoice_id: Uuid,
    pub phone_number: String,
    pub amount: Option<Decimal>, // if None, uses invoice balance_due
}

/// Response from STK Push initiation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MpesaStkPushResponse {
    pub merchant_request_id: String,
    pub checkout_request_id: String,
    pub response_code: String,
    pub response_description: String,
    pub customer_message: String,
}

/// M-Pesa payment link embedded in invoice delivery.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MpesaPaymentLink {
    pub invoice_id: Uuid,
    pub paybill_number: String,
    pub account_number: String, // typically invoice number
    pub amount: Decimal,
    pub url: Option<String>, // deep-link if available
}

/// Record of an M-Pesa transaction for reconciliation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MpesaTransactionRecord {
    pub id: Uuid,
    pub entity_id: Uuid,
    pub receipt_number: String,
    pub transaction_type: String,
    pub amount: Decimal,
    pub phone_number: String,
    pub timestamp: DateTime<Utc>,
    pub invoice_id: Option<Uuid>,
    pub payment_id: Option<Uuid>,
    pub reconciled: bool,
}
