use chrono::{DateTime, NaiveDate, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

use crate::types::{AccountCode, AgentOrUserId};

/// Status of an imported transaction in the categorisation queue.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum CategoryStatus {
    Uncategorised,
    Suggested,
    Categorised,
    Posted,
    Excluded,
}

impl CategoryStatus {
    /// The lowercase string stored in `imported_transactions.category_status`.
    pub fn as_db_str(&self) -> &'static str {
        match self {
            Self::Uncategorised => "uncategorised",
            Self::Suggested => "suggested",
            Self::Categorised => "categorised",
            Self::Posted => "posted",
            Self::Excluded => "excluded",
        }
    }

    /// Parse the lowercase DB/query string (case-insensitive) into a status.
    pub fn from_db_str(s: &str) -> Option<Self> {
        match s.trim().to_ascii_lowercase().as_str() {
            "uncategorised" | "uncategorized" => Some(Self::Uncategorised),
            "suggested" => Some(Self::Suggested),
            "categorised" | "categorized" => Some(Self::Categorised),
            "posted" => Some(Self::Posted),
            "excluded" => Some(Self::Excluded),
            _ => None,
        }
    }
}

/// AI-generated account suggestion for categorisation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccountSuggestion {
    pub account_code: AccountCode,
    pub account_name: String,
    pub confidence: f32,
    pub reason: String,
}

/// A part of a split transaction.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransactionSplit {
    pub id: Uuid,
    pub amount: Decimal,
    pub account_code: AccountCode,
    pub description: String,
}

/// An imported bank transaction in the categorisation queue.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportedTransaction {
    pub id: Uuid,
    pub entity_id: Uuid,
    pub bank_account: Uuid,
    pub value_date: NaiveDate,
    pub description: String,
    pub reference: String,
    pub debit: Option<Decimal>,
    pub credit: Option<Decimal>,
    pub running_bal: Decimal,
    pub category_status: CategoryStatus,
    pub assigned_account: Option<AccountCode>,
    pub split_parts: Vec<TransactionSplit>,
    pub merged_into: Option<Uuid>,
    pub journal_entry_id: Option<Uuid>,
    pub suggestion: Option<AccountSuggestion>,
    pub import_batch_id: Option<Uuid>,
    pub created_at: DateTime<Utc>,
}

/// Database row for imported transaction.
#[derive(Debug, Clone, serde::Serialize, FromRow)]
pub struct ImportedTransactionRow {
    pub id: Uuid,
    pub entity_id: Uuid,
    pub bank_account: Uuid,
    pub value_date: NaiveDate,
    pub description: String,
    pub reference: String,
    pub debit: Option<Decimal>,
    pub credit: Option<Decimal>,
    pub running_bal: Decimal,
    pub category_status: String,
    pub assigned_account: Option<String>,
    pub merged_into: Option<Uuid>,
    pub journal_entry_id: Option<Uuid>,
    pub suggestion: Option<serde_json::Value>,
    pub import_batch_id: Option<Uuid>,
    pub created_at: DateTime<Utc>,
}

/// Request to categorise a transaction.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CategoriseRequest {
    #[serde(default = "Uuid::nil")]
    pub transaction_id: Uuid,
    pub account_code: AccountCode,
    pub description: Option<String>,
    /// Set from the authenticated user by the API route; the client need not
    /// supply it. Defaults to a system agent so deserialization never fails.
    #[serde(default = "default_categoriser")]
    pub categorised_by: AgentOrUserId,
}

fn default_categoriser() -> AgentOrUserId {
    AgentOrUserId::Agent("system".to_string())
}

/// Request to split a transaction into multiple GL parts.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SplitRequest {
    pub transaction_id: Uuid,
    pub parts: Vec<SplitPart>,
    pub split_by: AgentOrUserId,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SplitPart {
    pub amount: Decimal,
    pub account_code: AccountCode,
    pub description: String,
}

/// Request to merge duplicate transactions.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MergeRequest {
    pub primary_id: Uuid,
    pub duplicate_ids: Vec<Uuid>,
    pub merged_by: AgentOrUserId,
}

/// Request to exclude a transaction (personal, duplicate, etc.).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExcludeRequest {
    pub transaction_id: Uuid,
    pub reason: String,
    pub excluded_by: AgentOrUserId,
}

/// Query parameters for the categorisation queue.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct TransactionQueueQuery {
    pub entity_id: Uuid,
    pub bank_account_id: Option<Uuid>,
    pub status: Option<CategoryStatus>,
    pub date_from: Option<NaiveDate>,
    pub date_to: Option<NaiveDate>,
    pub search: Option<String>,
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}
