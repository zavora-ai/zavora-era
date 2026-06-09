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

impl WhtCategory {
    /// Returns the WHT rate as (resident_rate, non_resident_rate)
    pub fn rates(&self) -> (Decimal, Decimal) {
        match self {
            Self::Consultancy | Self::ManagementFees => {
                (Decimal::new(5, 2), Decimal::new(20, 2))
            }
            Self::Rent => (Decimal::new(10, 2), Decimal::new(30, 2)),
            Self::Royalties => (Decimal::new(5, 2), Decimal::new(20, 2)),
            Self::Interest => (Decimal::new(15, 2), Decimal::new(15, 2)),
            Self::Contractual => (Decimal::new(3, 2), Decimal::new(20, 2)),
            Self::Dividends => (Decimal::new(5, 2), Decimal::new(15, 2)),
            Self::Insurance => (Decimal::new(5, 2), Decimal::new(20, 2)),
            Self::Transport => (Decimal::new(2, 2), Decimal::new(20, 2)),
            Self::Other(_) => (Decimal::new(5, 2), Decimal::new(20, 2)),
        }
    }

    pub fn rate_for(&self, resident: bool) -> Decimal {
        let (r, nr) = self.rates();
        if resident { r } else { nr }
    }
}

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
