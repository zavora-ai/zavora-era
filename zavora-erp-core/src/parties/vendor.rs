use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

use crate::types::{
    AccountCode, Address, BankDetails, ContactEmail, ContactPhone, CurrencyCode, PaymentTerms,
    WhtCategory,
};

/// A vendor — party from whom the entity receives bills.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Vendor {
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
    pub wht_category: Option<WhtCategory>,
    pub resident: bool,
    pub ap_account: AccountCode,
    pub default_expense_account: Option<AccountCode>,
    pub bank_details: Option<BankDetails>,
    pub notes: Option<String>,
    pub is_active: bool,
    pub created_at: DateTime<Utc>,
}

/// Database row for vendor.
#[derive(Debug, Clone, FromRow, Serialize)]
pub struct VendorRow {
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
    pub wht_category: Option<String>,
    pub resident: bool,
    pub ap_account: String,
    pub default_expense_account: Option<String>,
    pub bank_details: Option<serde_json::Value>,
    pub notes: Option<String>,
    pub is_active: bool,
    pub created_at: DateTime<Utc>,
    #[serde(default)]
    pub general_business_group_id: Option<Uuid>,
    #[serde(default)]
    pub vat_business_group_id: Option<Uuid>,
}

/// Request to create a vendor.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateVendorRequest {
    pub name: String,
    pub kra_pin: Option<String>,
    pub vat_number: Option<String>,
    pub email: Vec<ContactEmail>,
    pub phone: Vec<ContactPhone>,
    pub address: Option<Address>,
    pub currency: Option<CurrencyCode>,
    pub payment_terms: Option<PaymentTerms>,
    pub wht_category: Option<WhtCategory>,
    pub resident: Option<bool>,
    pub ap_account: Option<AccountCode>,
    pub default_expense_account: Option<AccountCode>,
    pub bank_details: Option<BankDetails>,
    pub notes: Option<String>,
}

/// Request to update a vendor.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct UpdateVendorRequest {
    pub name: Option<String>,
    pub kra_pin: Option<Option<String>>,
    pub vat_number: Option<Option<String>>,
    pub email: Option<Vec<ContactEmail>>,
    pub phone: Option<Vec<ContactPhone>>,
    pub address: Option<Option<Address>>,
    pub currency: Option<CurrencyCode>,
    pub payment_terms: Option<PaymentTerms>,
    pub wht_category: Option<Option<WhtCategory>>,
    pub resident: Option<bool>,
    pub ap_account: Option<AccountCode>,
    pub default_expense_account: Option<Option<AccountCode>>,
    pub bank_details: Option<Option<BankDetails>>,
    pub notes: Option<Option<String>>,
    pub is_active: Option<bool>,
}
