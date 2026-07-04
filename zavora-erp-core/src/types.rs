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

    /// Parse a payment-terms value as stored in the DB.
    ///
    /// Vendors/customers persist terms via `serde_json::to_string`, which yields
    /// a JSON-quoted string like `"Net30"`. Parsing must therefore treat the
    /// stored value as JSON directly. (The previous call sites wrapped it in an
    /// extra pair of quotes — `""Net30""` — which never parsed and silently fell
    /// back to Net30 for every party, so non-Net30 terms were ignored.) Falls
    /// back to a bare variant name, then to Net30.
    pub fn parse_stored(s: &str) -> Self {
        serde_json::from_str::<Self>(s)
            .or_else(|_| serde_json::from_str::<Self>(&format!("\"{}\"", s)))
            .unwrap_or(Self::Net30)
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
mod payment_terms_tests {
    use super::PaymentTerms;
    use chrono::NaiveDate;

    #[test]
    fn parse_stored_handles_json_quoted_form() {
        // This is how vendors/customers actually store terms
        // (serde_json::to_string → `"Net60"`). The old double-quoting bug made
        // every non-Net30 party fall back to Net30.
        assert_eq!(PaymentTerms::parse_stored("\"Net60\"").days(), 60);
        assert_eq!(PaymentTerms::parse_stored("\"Net14\"").days(), 14);
        assert_eq!(PaymentTerms::parse_stored("\"DueOnReceipt\"").days(), 0);
    }

    #[test]
    fn parse_stored_handles_bare_form_and_garbage() {
        assert_eq!(PaymentTerms::parse_stored("Net45").days(), 45);
        // Unknown → safe default.
        assert_eq!(PaymentTerms::parse_stored("garbage").days(), 30);
    }

    #[test]
    fn due_date_reflects_terms() {
        let issue = NaiveDate::from_ymd_opt(2025, 1, 1).unwrap();
        assert_eq!(
            PaymentTerms::parse_stored("\"Net60\"").due_date(issue),
            NaiveDate::from_ymd_opt(2025, 3, 2).unwrap()
        );
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
