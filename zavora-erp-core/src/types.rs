use chrono::{DateTime, NaiveDate, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

// === Currency ===

/// ISO 4217 currency code (e.g. "KES", "USD", "EUR")
pub type CurrencyCode = String;

// === Account Code ===

/// Account code in the chart of accounts (e.g. "1010", "5100")
pub type AccountCode = String;

// === Actor identification ===

/// Identifies the actor performing an action — either a human user or an AI agent.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "type", content = "id")]
pub enum AgentOrUserId {
    User(Uuid),
    Agent(String),
}

// === Contact types ===

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContactEmail {
    pub email: String,
    pub label: Option<String>,
    pub is_primary: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContactPhone {
    pub number: String,
    pub label: Option<String>,
    pub is_primary: bool,
    pub whatsapp_enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Address {
    pub line1: String,
    pub line2: Option<String>,
    pub city: String,
    pub county: Option<String>,
    pub postal_code: Option<String>,
    pub country: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BankDetails {
    pub bank_name: String,
    pub branch: Option<String>,
    pub account_name: String,
    pub account_number: String,
    pub swift_code: Option<String>,
}

// === Payment terms ===

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum PaymentTerms {
    DueOnReceipt,
    Net7,
    Net14,
    Net30,
    Net45,
    Net60,
    Net90,
    Custom { days: u32, description: String },
}

impl PaymentTerms {
    pub fn days(&self) -> u32 {
        match self {
            Self::DueOnReceipt => 0,
            Self::Net7 => 7,
            Self::Net14 => 14,
            Self::Net30 => 30,
            Self::Net45 => 45,
            Self::Net60 => 60,
            Self::Net90 => 90,
            Self::Custom { days, .. } => *days,
        }
    }

    pub fn due_date(&self, issue_date: NaiveDate) -> NaiveDate {
        issue_date + chrono::Duration::days(self.days() as i64)
    }
}

// === Channel ===

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum Channel {
    Email,
    WhatsApp,
    Sms,
    InApp,
}

// === Output format ===

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum ExportFormat {
    Pdf,
    Csv,
    Xlsx,
    Json,
}

// === Unit of measure ===

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum UnitOfMeasure {
    Each,
    Hour,
    Day,
    Week,
    Month,
    Kg,
    Gram,
    Litre,
    Metre,
    SquareMetre,
    Box,
    Pack,
    Custom(String),
}

// === VAT treatment ===

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum VatTreatment {
    /// Standard 16% VAT
    Standard16,
    /// 8% petroleum products
    Petroleum8,
    /// Zero-rated (exports, basic foodstuffs)
    ZeroRated,
    /// Exempt (financial services, land)
    Exempt,
    /// Out of scope
    OutOfScope,
}

impl VatTreatment {
    pub fn rate(&self) -> Decimal {
        match self {
            Self::Standard16 => Decimal::new(16, 2),
            Self::Petroleum8 => Decimal::new(8, 2),
            Self::ZeroRated | Self::Exempt | Self::OutOfScope => Decimal::ZERO,
        }
    }
}

#[cfg(test)]
mod vat_treatment_tests {
    use super::VatTreatment;
    use rust_decimal_macros::dec;

    #[test]
    fn standard_rate_is_sixteen_percent() {
        assert_eq!(VatTreatment::Standard16.rate(), dec!(0.16));
        // 16% VAT on a 1,000 line = 160.
        assert_eq!(dec!(1_000) * VatTreatment::Standard16.rate(), dec!(160.00));
    }

    #[test]
    fn petroleum_rate_is_eight_percent() {
        assert_eq!(VatTreatment::Petroleum8.rate(), dec!(0.08));
        assert_eq!(dec!(1_000) * VatTreatment::Petroleum8.rate(), dec!(80.00));
    }

    #[test]
    fn zero_rated_and_exempt_carry_no_vat() {
        // Zero-rated (exports/basic foodstuffs) and exempt (financial/land) both
        // attract no output VAT — but are distinct treatments for the VAT return.
        assert!(VatTreatment::ZeroRated.rate().is_zero());
        assert!(VatTreatment::Exempt.rate().is_zero());
        assert!(VatTreatment::OutOfScope.rate().is_zero());
    }
}

// === WHT category ===

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum WhtCategory {
    Consultancy,
    ManagementFees,
    Rent,
    Royalties,
    Interest,
    Contractual,
    Dividends,
    Insurance,
    Transport,
    Other(String),
}

// WHT rates are NOT defined here. They live solely in the `wht_rates` table
// (single source of truth) and are read via `services::wht::wht_rate_for`, so
// there is no risk of code and config diverging.

// === Monthly amount (for charts) ===

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MonthlyAmount {
    pub year: i32,
    pub month: u32,
    pub amount: Decimal,
}

// === Attachment reference ===

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AttachmentRef {
    pub id: Uuid,
    pub filename: String,
    pub mime_type: String,
    pub storage_key: String,
    pub size_bytes: u64,
    pub uploaded_by: AgentOrUserId,
    pub uploaded_at: DateTime<Utc>,
}

// === Linked type (for attachments, audit) ===

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum LinkedType {
    Invoice,
    Bill,
    Payment,
    JournalEntry,
    Receipt,
    Estimate,
    CreditNote,
    Employee,
    Asset,
    Customer,
    Vendor,
}
