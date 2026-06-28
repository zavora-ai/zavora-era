use chrono::{DateTime, NaiveDate, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

use crate::types::CurrencyCode;

/// Type of payment — AR (from customer) or AP (to vendor).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PaymentType {
    CustomerPayment,
    VendorPayment,
}

/// Payment method with details.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PaymentMethod {
    Mpesa {
        transaction_id: String,
        phone: String,
    },
    BankTransfer {
        reference: String,
    },
    Card {
        processor: String,
        authorization: String,
    },
    Cash,
    Cheque {
        number: String,
    },
}

/// Status of a payment record.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PaymentStatus {
    Pending,
    Completed,
    Failed,
    Reversed,
}

/// Application of a payment to a specific document (invoice or bill).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaymentApplication {
    pub document_id: Uuid,
    pub document_type: PaymentDocType,
    pub amount_applied: Decimal,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum PaymentDocType {
    Invoice,
    Bill,
}

/// A payment record.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Payment {
    pub id: Uuid,
    pub entity_id: Uuid,
    pub number: String,
    pub payment_type: PaymentType,
    pub party_id: Uuid,
    pub payment_date: NaiveDate,
    pub amount: Decimal,
    pub currency: CurrencyCode,
    pub fx_rate: Decimal,
    pub method: PaymentMethod,
    pub reference: String,
    pub bank_account_id: Option<Uuid>,
    pub applications: Vec<PaymentApplication>,
    pub unapplied: Decimal,
    pub journal_entry_id: Option<Uuid>,
    pub status: PaymentStatus,
    pub created_at: DateTime<Utc>,
}

impl Payment {
    /// Recalculate unapplied amount.
    pub fn recalculate_unapplied(&mut self) {
        let applied: Decimal = self.applications.iter().map(|a| a.amount_applied).sum();
        self.unapplied = self.amount - applied;
    }

    /// Total amount applied to documents.
    pub fn total_applied(&self) -> Decimal {
        self.applications.iter().map(|a| a.amount_applied).sum()
    }
}

/// Database row for payment.
#[derive(Debug, Clone, serde::Serialize, FromRow)]
pub struct PaymentRow {
    pub id: Uuid,
    pub entity_id: Uuid,
    pub number: String,
    pub payment_type: String,
    pub party_id: Uuid,
    pub payment_date: NaiveDate,
    pub amount: Decimal,
    pub currency: String,
    pub fx_rate: Decimal,
    pub method: serde_json::Value,
    pub reference: String,
    pub bank_account_id: Option<Uuid>,
    pub applications: serde_json::Value,
    pub unapplied: Decimal,
    pub journal_entry_id: Option<Uuid>,
    pub status: String,
    pub created_at: DateTime<Utc>,
}

/// Request to record a payment.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecordPaymentRequest {
    pub payment_type: PaymentType,
    pub party_id: Uuid,
    pub payment_date: Option<NaiveDate>,
    pub amount: Decimal,
    pub currency: Option<CurrencyCode>,
    pub fx_rate: Option<Decimal>,
    pub method: PaymentMethod,
    pub reference: Option<String>,
    pub bank_account_id: Option<Uuid>,
    pub applications: Vec<PaymentApplicationRequest>,
    /// Withholding tax withheld by the customer on this receipt, in BASE currency
    /// (KES) — KRA WHT is always denominated in KES regardless of invoice currency.
    /// On a customer receipt this books a WHT-receivable (income-tax credit) asset;
    /// the receipt clears the full AR as cash + WHT. `None`/0 = no WHT.
    #[serde(default)]
    pub wht_amount: Option<Decimal>,
    /// Optional override for the WHT account (defaults to posting `wht_receivable`).
    #[serde(default)]
    pub wht_account: Option<String>,
    /// Non-cash funding source: when set, the payment is funded from this GL
    /// account (e.g. `4200` Directors Loans for an owner-funded purchase) instead
    /// of a bank account — the "Bank" leg credits/debits this account. Lets a bill
    /// be settled by the director personally without touching a company cash
    /// account. Takes precedence over `bank_account_id` when present.
    #[serde(default)]
    pub funding_account: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaymentApplicationRequest {
    pub document_id: Uuid,
    pub amount: Decimal,
}

/// Request to apply an unapplied payment to a document.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApplyPaymentRequest {
    pub payment_id: Uuid,
    pub document_id: Uuid,
    pub amount: Decimal,
}
