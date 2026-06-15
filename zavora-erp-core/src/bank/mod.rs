use chrono::{DateTime, NaiveDate, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

use crate::types::{AccountCode, AgentOrUserId, CurrencyCode};

/// A bank account linked to the entity.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BankAccount {
    pub id: Uuid,
    pub entity_id: Uuid,
    pub name: String,
    pub bank_name: String,
    pub account_number: String,
    pub currency: CurrencyCode,
    pub gl_account: AccountCode,
    pub feed_enabled: bool,
    pub feed_provider: Option<BankFeedProvider>,
    pub last_sync: Option<DateTime<Utc>>,
    pub is_active: bool,
    pub created_at: DateTime<Utc>,
}

/// Database row for bank account.
#[derive(Debug, Clone, serde::Serialize, FromRow)]
pub struct BankAccountRow {
    pub id: Uuid,
    pub entity_id: Uuid,
    pub name: String,
    pub bank_name: String,
    pub account_number: String,
    pub currency: String,
    pub gl_account: String,
    pub feed_enabled: bool,
    pub feed_provider: Option<String>,
    pub last_sync: Option<DateTime<Utc>>,
    pub is_active: bool,
    pub created_at: DateTime<Utc>,
}

/// Supported bank feed providers.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum BankFeedProvider {
    Kcb,
    Equity,
    Ncba,
    Mpesa,
    Manual,
}

/// Statement import format.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum StatementFormat {
    Mt940,
    Ofx,
    Csv,
    Api,
}

/// A bank statement import batch.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StatementImport {
    pub id: Uuid,
    pub entity_id: Uuid,
    pub bank_account_id: Uuid,
    pub format: StatementFormat,
    pub filename: Option<String>,
    pub imported_at: DateTime<Utc>,
    pub line_count: u32,
    pub matched_count: u32,
    pub unmatched_count: u32,
}

/// Result of the three-pass matching algorithm.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MatchReport {
    pub statement_id: Uuid,
    pub exact_matches: Vec<MatchPair>,
    pub near_matches: Vec<NearMatch>,
    pub ai_suggestions: Vec<AiSuggestion>,
    pub unmatched: Vec<Uuid>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MatchPair {
    pub statement_line_id: Uuid,
    pub journal_entry_id: Uuid,
    pub amount: Decimal,
    pub date: NaiveDate,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NearMatch {
    pub statement_line_id: Uuid,
    pub journal_entry_id: Uuid,
    pub amount: Decimal,
    pub date_diff_days: i32,
    pub reference_similarity: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiSuggestion {
    pub statement_line_id: Uuid,
    pub suggested_account: AccountCode,
    pub confidence: f32,
    pub reason: String,
}

/// Request to confirm a reconciliation match.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfirmMatchRequest {
    pub statement_line_id: Uuid,
    pub journal_entry_id: Uuid,
    pub confirmed_by: AgentOrUserId,
}

/// Request to post an unmatched bank line as a new journal entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PostUnmatchedRequest {
    pub statement_line_id: Uuid,
    pub account_code: AccountCode,
    pub description: String,
    pub posted_by: AgentOrUserId,
}

/// Bank reconciliation summary for a statement.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReconciliationSummary {
    pub bank_account_id: Uuid,
    pub statement_id: Uuid,
    pub statement_balance: Decimal,
    pub gl_balance: Decimal,
    pub difference: Decimal,
    pub matched_lines: u32,
    pub unmatched_lines: u32,
    pub is_reconciled: bool,
}

/// Request to create a bank account.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateBankAccountRequest {
    pub name: String,
    pub bank_name: String,
    pub account_number: String,
    pub currency: Option<CurrencyCode>,
    pub gl_account: Option<AccountCode>,
    pub feed_provider: Option<BankFeedProvider>,
}

/// Request to import a bank statement file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportStatementRequest {
    pub entity_id: Uuid,
    pub bank_account_id: Uuid,
    pub filename: String,
    pub content: String,
    pub imported_by: AgentOrUserId,
}

/// A parsed transaction line from a bank statement.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParsedStatementLine {
    pub value_date: NaiveDate,
    pub description: String,
    pub reference: String,
    pub debit: Option<Decimal>,
    pub credit: Option<Decimal>,
    pub balance: Option<Decimal>,
}

/// Result of a successful statement import.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportStatementResult {
    pub import_id: Uuid,
    pub format: StatementFormat,
    pub line_count: u32,
    pub matched_count: u32,
    pub unmatched_count: u32,
}
