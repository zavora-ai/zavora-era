use chrono::{DateTime, NaiveDate, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use std::collections::HashMap;
use uuid::Uuid;

use crate::types::{AccountCode, AgentOrUserId, CurrencyCode};

/// Source of a journal entry — identifies which subsystem created it.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum JournalSource {
    Manual,
    Invoice,
    CreditNote,
    Bill,
    SupplierCreditNote,
    Payment,
    Payroll,
    Depreciation,
    FxRevaluation,
    InventoryAdjustment,
    BankFee,
    OpeningBalance,
    YearEndClose,
    Agent(String),
}

/// Status of a journal entry.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum EntryStatus {
    Draft,
    Posted,
    Reversed,
}

/// A complete double-entry journal entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JournalEntry {
    pub id: Uuid,
    pub entity_id: Uuid,
    pub number: String,
    pub date: NaiveDate,
    pub period_id: Uuid,
    pub source: JournalSource,
    pub reference: String,
    pub description: String,
    pub lines: Vec<JournalLine>,
    pub status: EntryStatus,
    pub created_by: AgentOrUserId,
    pub created_at: DateTime<Utc>,
    pub posted_at: Option<DateTime<Utc>>,
}

/// A single line (leg) of a journal entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JournalLine {
    pub id: Uuid,
    pub account_code: AccountCode,
    pub debit: Option<Decimal>,
    pub credit: Option<Decimal>,
    pub currency: CurrencyCode,
    pub fx_rate: Decimal,
    pub functional_debit: Option<Decimal>,
    pub functional_credit: Option<Decimal>,
    pub description: Option<String>,
    pub dimensions: HashMap<String, String>,
}

impl JournalLine {
    /// Compute functional amounts from transaction amounts and FX rate.
    pub fn compute_functional(&mut self) {
        self.functional_debit = self.debit.map(|d| d * self.fx_rate);
        self.functional_credit = self.credit.map(|c| c * self.fx_rate);
    }

    /// Returns the net functional amount (debit positive, credit negative).
    pub fn functional_net(&self) -> Decimal {
        let dr = self.functional_debit.unwrap_or(Decimal::ZERO);
        let cr = self.functional_credit.unwrap_or(Decimal::ZERO);
        dr - cr
    }
}

impl JournalEntry {
    /// Validate that functional debits equal functional credits.
    pub fn is_balanced(&self) -> bool {
        let total_debits: Decimal = self
            .lines
            .iter()
            .filter_map(|l| l.functional_debit)
            .sum();
        let total_credits: Decimal = self
            .lines
            .iter()
            .filter_map(|l| l.functional_credit)
            .sum();
        total_debits == total_credits
    }

    /// Returns (total_functional_debits, total_functional_credits).
    pub fn totals(&self) -> (Decimal, Decimal) {
        let debits: Decimal = self
            .lines
            .iter()
            .filter_map(|l| l.functional_debit)
            .sum();
        let credits: Decimal = self
            .lines
            .iter()
            .filter_map(|l| l.functional_credit)
            .sum();
        (debits, credits)
    }
}

/// Database row for a journal entry header.
#[derive(Debug, Clone, serde::Serialize, FromRow)]
pub struct JournalEntryRow {
    pub id: Uuid,
    pub entity_id: Uuid,
    pub number: String,
    pub date: NaiveDate,
    pub period_id: Uuid,
    pub source: String,
    pub reference: String,
    pub description: String,
    pub status: String,
    pub created_by: serde_json::Value,
    pub created_at: DateTime<Utc>,
    pub posted_at: Option<DateTime<Utc>>,
}

/// Database row for a journal line.
#[derive(Debug, Clone, serde::Serialize, FromRow)]
pub struct JournalLineRow {
    pub id: Uuid,
    pub entry_id: Uuid,
    pub account_code: String,
    pub debit: Option<Decimal>,
    pub credit: Option<Decimal>,
    pub currency: String,
    pub fx_rate: Decimal,
    pub functional_debit: Option<Decimal>,
    pub functional_credit: Option<Decimal>,
    pub description: Option<String>,
    pub dimensions: serde_json::Value,
}

/// Request to create a journal entry (from API or agent).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateJournalEntryRequest {
    pub date: NaiveDate,
    pub source: JournalSource,
    pub reference: String,
    pub description: String,
    /// Id of the source document (invoice/bill/credit note/payment), when the
    /// entry originates from one. Lets the GL drill back to the document.
    #[serde(default)]
    pub source_id: Option<Uuid>,
    pub lines: Vec<CreateJournalLineRequest>,
    pub post_immediately: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateJournalLineRequest {
    pub account_code: AccountCode,
    pub debit: Option<Decimal>,
    pub credit: Option<Decimal>,
    pub currency: CurrencyCode,
    pub fx_rate: Option<Decimal>,
    pub description: Option<String>,
    pub dimensions: Option<HashMap<String, String>>,
}

/// Validation report for a journal entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationReport {
    pub is_valid: bool,
    pub errors: Vec<String>,
    pub warnings: Vec<String>,
}
