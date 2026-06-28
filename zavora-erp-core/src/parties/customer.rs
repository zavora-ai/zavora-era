use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

use crate::types::{
    AccountCode, Address, Channel, ContactEmail, ContactPhone, CurrencyCode, PaymentTerms,
};

/// Reminder policy for a customer — controls when overdue reminders are sent.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReminderPolicy {
    pub reminders: Vec<ReminderRule>,
}

impl Default for ReminderPolicy {
    fn default() -> Self {
        Self {
            reminders: vec![
                ReminderRule {
                    offset_days: -3,
                    channels: vec![Channel::Email],
                    template_id: None,
                },
                ReminderRule {
                    offset_days: 1,
                    channels: vec![Channel::Email, Channel::WhatsApp],
                    template_id: None,
                },
                ReminderRule {
                    offset_days: 7,
                    channels: vec![Channel::Email, Channel::WhatsApp],
                    template_id: None,
                },
                ReminderRule {
                    offset_days: 14,
                    channels: vec![Channel::Email, Channel::WhatsApp, Channel::Sms],
                    template_id: None,
                },
            ],
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReminderRule {
    /// Negative = before due, Positive = after due.
    pub offset_days: i32,
    pub channels: Vec<Channel>,
    pub template_id: Option<Uuid>,
}

/// A customer — party to whom the entity issues invoices.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Customer {
    pub id: Uuid,
    pub entity_id: Uuid,
    pub name: String,
    pub kra_pin: Option<String>,
    pub vat_number: Option<String>,
    pub email: Vec<ContactEmail>,
    pub phone: Vec<ContactPhone>,
    pub address: Option<Address>,
    pub currency: CurrencyCode,
    pub payment_terms: PaymentTerms,
    pub credit_limit: Option<Decimal>,
    pub ar_account: AccountCode,
    pub reminder_policy: ReminderPolicy,
    pub portal_enabled: bool,
    pub notes: Option<String>,
    pub is_active: bool,
    pub created_at: DateTime<Utc>,
}

/// Database row for customer.
#[derive(Debug, Clone, FromRow, Serialize)]
pub struct CustomerRow {
    pub id: Uuid,
    pub entity_id: Uuid,
    pub name: String,
    pub kra_pin: Option<String>,
    pub vat_number: Option<String>,
    pub email: serde_json::Value,
    pub phone: serde_json::Value,
    pub address: Option<serde_json::Value>,
    pub currency: String,
    pub payment_terms: String,
    pub credit_limit: Option<Decimal>,
    pub ar_account: String,
    pub reminder_policy: serde_json::Value,
    pub portal_enabled: bool,
    pub notes: Option<String>,
    pub is_active: bool,
    pub created_at: DateTime<Utc>,
    /// Posting-group assignments (BC-style). Surfaced so the UI can show/edit them.
    #[serde(default)]
    pub general_business_group_id: Option<Uuid>,
    #[serde(default)]
    pub vat_business_group_id: Option<Uuid>,
}

/// Request to create a customer.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateCustomerRequest {
    pub name: String,
    pub kra_pin: Option<String>,
    pub vat_number: Option<String>,
    pub email: Vec<ContactEmail>,
    pub phone: Vec<ContactPhone>,
    pub address: Option<Address>,
    pub currency: Option<CurrencyCode>,
    pub payment_terms: Option<PaymentTerms>,
    pub credit_limit: Option<Decimal>,
    pub ar_account: Option<AccountCode>,
    pub reminder_policy: Option<ReminderPolicy>,
    pub portal_enabled: Option<bool>,
    pub notes: Option<String>,
}

/// Request to update a customer.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct UpdateCustomerRequest {
    pub name: Option<String>,
    pub kra_pin: Option<Option<String>>,
    pub vat_number: Option<Option<String>>,
    pub email: Option<Vec<ContactEmail>>,
    pub phone: Option<Vec<ContactPhone>>,
    pub address: Option<Option<Address>>,
    pub currency: Option<CurrencyCode>,
    pub payment_terms: Option<PaymentTerms>,
    pub credit_limit: Option<Option<Decimal>>,
    pub ar_account: Option<AccountCode>,
    pub reminder_policy: Option<ReminderPolicy>,
    pub portal_enabled: Option<bool>,
    pub notes: Option<Option<String>>,
    pub is_active: Option<bool>,
}
