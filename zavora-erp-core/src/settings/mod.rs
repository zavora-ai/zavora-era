use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::ledger::CoaTemplate;
use crate::posting::PostingSetup;
use crate::types::{CurrencyCode, VatTreatment};

/// Complete engine configuration for an entity.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErpConfig {
    pub entity_id: Uuid,
    pub base_currency: CurrencyCode,
    pub fiscal_year_end: MonthDay,
    pub coa_template: CoaTemplate,
    pub branding: BrandingConfig,
    pub sequences: DocumentSequences,
    pub tax_config: TaxConfig,
    pub payment_config: PaymentConfig,
    /// GL account determination (control/clearing/default accounts).
    pub posting: PostingSetup,
}

/// Month and day (e.g. December 31 = { month: 12, day: 31 })
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MonthDay {
    pub month: u32,
    pub day: u32,
}

/// Branding configuration for documents.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BrandingConfig {
    pub company_name: String,
    pub logo_url: Option<String>,
    pub primary_color: String,
    pub secondary_color: Option<String>,
    pub font: String,
    pub footer_text: Option<String>,
    pub website: Option<String>,
    pub phone: Option<String>,
    pub email: Option<String>,
    pub address: Option<String>,
    pub kra_pin: Option<String>,
    pub vat_number: Option<String>,
}

/// Document numbering sequences.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocumentSequences {
    pub invoice_prefix: String,
    pub invoice_next: u64,
    pub estimate_prefix: String,
    pub estimate_next: u64,
    pub credit_note_prefix: String,
    pub credit_note_next: u64,
    pub bill_prefix: String,
    pub bill_next: u64,
    pub journal_prefix: String,
    pub journal_next: u64,
    pub payment_prefix: String,
    pub payment_next: u64,
    pub year_reset: bool,
}

impl DocumentSequences {
    /// Generate the next number for a given document type and advance the counter.
    pub fn next_number(&mut self, doc_type: SeqType, fiscal_year: i32) -> String {
        let (prefix, next) = match doc_type {
            SeqType::Invoice => (&self.invoice_prefix, &mut self.invoice_next),
            SeqType::Estimate => (&self.estimate_prefix, &mut self.estimate_next),
            SeqType::CreditNote => (&self.credit_note_prefix, &mut self.credit_note_next),
            SeqType::Bill => (&self.bill_prefix, &mut self.bill_next),
            SeqType::Journal => (&self.journal_prefix, &mut self.journal_next),
            SeqType::Payment => (&self.payment_prefix, &mut self.payment_next),
        };
        let number = if self.year_reset {
            format!("{}-{}-{:04}", prefix, fiscal_year, next)
        } else {
            format!("{}-{:06}", prefix, next)
        };
        *next += 1;
        number
    }
}

impl Default for DocumentSequences {
    fn default() -> Self {
        Self {
            invoice_prefix: "INV".to_string(),
            invoice_next: 1,
            estimate_prefix: "EST".to_string(),
            estimate_next: 1,
            credit_note_prefix: "CN".to_string(),
            credit_note_next: 1,
            bill_prefix: "BILL".to_string(),
            bill_next: 1,
            journal_prefix: "JE".to_string(),
            journal_next: 1,
            payment_prefix: "PAY".to_string(),
            payment_next: 1,
            year_reset: true,
        }
    }
}

/// Document sequence type.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum SeqType {
    Invoice,
    Estimate,
    CreditNote,
    Bill,
    Journal,
    Payment,
}

/// Tax configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaxConfig {
    pub vat_registered: bool,
    pub vat_number: Option<String>,
    pub vat_period: VatPeriod,
    pub standard_vat_rate: Decimal,
    pub default_vat_treatment: VatTreatment,
    pub wht_enabled: bool,
    pub paye_enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum VatPeriod {
    Monthly,
    Quarterly,
}

/// Payment integration configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaymentConfig {
    pub mpesa_enabled: bool,
    pub mpesa_paybill: Option<String>,
    pub mpesa_till_number: Option<String>,
    pub flutterwave_enabled: bool,
    pub flutterwave_public_key: Option<String>,
    pub bank_transfer_enabled: bool,
    pub default_bank_account_id: Option<Uuid>,
}

/// Patch for settings update.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SettingsPatch {
    pub base_currency: Option<CurrencyCode>,
    pub fiscal_year_end: Option<MonthDay>,
    pub branding: Option<BrandingConfig>,
    pub tax_config: Option<TaxConfig>,
    pub payment_config: Option<PaymentConfig>,
    pub posting: Option<PostingSetup>,
}

/// Patch for a single sequence type.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SeqPatch {
    pub prefix: Option<String>,
    pub next: Option<u64>,
    pub year_reset: Option<bool>,
}

/// Database row for entity settings.
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct SettingsRow {
    pub entity_id: uuid::Uuid,
    pub base_currency: String,
    pub fiscal_year_end: String,
    pub coa_template: String,
    pub branding: serde_json::Value,
    pub sequences: serde_json::Value,
    pub tax_config: serde_json::Value,
    pub payment_config: serde_json::Value,
    pub posting_setup: serde_json::Value,
    pub updated_at: chrono::DateTime<chrono::Utc>,
    pub updated_by: Option<uuid::Uuid>,
}

/// Load the configuration for an entity, creating default settings if none exist.
pub async fn load_or_create_config(
    pool: &sqlx::PgPool,
    entity_id: Uuid,
) -> crate::error::ErpResult<ErpConfig> {
    let exists = sqlx::query_scalar::<_, bool>(
        "SELECT EXISTS(SELECT 1 FROM entity_settings WHERE entity_id = $1)",
    )
    .bind(entity_id)
    .fetch_one(pool)
    .await?;

    if !exists {
        sqlx::query("INSERT INTO entity_settings (entity_id) VALUES ($1)")
            .bind(entity_id)
            .execute(pool)
            .await?;
    }

    let row = sqlx::query_as::<_, SettingsRow>("SELECT * FROM entity_settings WHERE entity_id = $1")
        .bind(entity_id)
        .fetch_one(pool)
        .await?;

    let branding: BrandingConfig = serde_json::from_value(row.branding).unwrap_or_else(|_| {
        BrandingConfig {
            company_name: "My Company".to_string(),
            logo_url: None,
            primary_color: "#1a56db".to_string(),
            secondary_color: None,
            font: "Inter".to_string(),
            footer_text: None,
            website: None,
            phone: None,
            email: None,
            address: None,
            kra_pin: None,
            vat_number: None,
        }
    });
    let sequences: DocumentSequences = serde_json::from_value(row.sequences).unwrap_or_default();
    let tax_config: TaxConfig = serde_json::from_value(row.tax_config).unwrap_or_else(|_| TaxConfig {
        vat_registered: false,
        vat_number: None,
        vat_period: VatPeriod::Monthly,
        standard_vat_rate: Decimal::new(16, 2),
        default_vat_treatment: VatTreatment::Standard16,
        wht_enabled: true,
        paye_enabled: true,
    });
    let payment_config: PaymentConfig =
        serde_json::from_value(row.payment_config).unwrap_or_else(|_| PaymentConfig {
            mpesa_enabled: false,
            mpesa_paybill: None,
            mpesa_till_number: None,
            flutterwave_enabled: false,
            flutterwave_public_key: None,
            bank_transfer_enabled: true,
            default_bank_account_id: None,
        });
    let fiscal_year_end: MonthDay =
        serde_json::from_str(&row.fiscal_year_end).unwrap_or(MonthDay { month: 12, day: 31 });
    let posting: PostingSetup = serde_json::from_value(row.posting_setup).unwrap_or_default();

    Ok(ErpConfig {
        entity_id,
        base_currency: row.base_currency,
        fiscal_year_end,
        coa_template: CoaTemplate::KenyaStandard,
        branding,
        sequences,
        tax_config,
        payment_config,
        posting,
    })
}
