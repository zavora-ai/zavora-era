use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

use crate::types::AccountCode;

/// Classification of an account in the chart of accounts.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, sqlx::Type)]
#[sqlx(type_name = "text")]
#[sqlx(rename_all = "snake_case")]
pub enum AccountType {
    Asset,
    Liability,
    Equity,
    Revenue,
    Expense,
    ContraAsset,
    ContraLiability,
    ContraRevenue,
    ContraExpense,
}

impl AccountType {
    /// Returns the normal balance side for this account type.
    pub fn normal_balance(&self) -> BalanceSide {
        match self {
            Self::Asset | Self::Expense | Self::ContraLiability | Self::ContraRevenue => {
                BalanceSide::Debit
            }
            Self::Liability | Self::Equity | Self::Revenue | Self::ContraAsset
            | Self::ContraExpense => BalanceSide::Credit,
        }
    }

    /// Returns which financial statement this account type appears on.
    pub fn statement(&self) -> FinancialStatement {
        match self {
            Self::Asset | Self::Liability | Self::Equity | Self::ContraAsset
            | Self::ContraLiability => FinancialStatement::BalanceSheet,
            Self::Revenue | Self::Expense | Self::ContraRevenue | Self::ContraExpense => {
                FinancialStatement::ProfitAndLoss
            }
        }
    }

    /// Whether this type increases on the debit side.
    pub fn increases_on_debit(&self) -> bool {
        matches!(self.normal_balance(), BalanceSide::Debit)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum BalanceSide {
    Debit,
    Credit,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum FinancialStatement {
    BalanceSheet,
    ProfitAndLoss,
}

/// A single account in the chart of accounts.
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Account {
    pub id: Uuid,
    pub entity_id: Uuid,
    pub code: AccountCode,
    pub name: String,
    pub account_type: String, // stored as text, parsed to AccountType
    pub parent_code: Option<AccountCode>,
    pub currency: Option<String>,
    pub is_control: bool,
    pub is_active: bool,
    pub tags: serde_json::Value, // stored as JSON array
    pub created_at: DateTime<Utc>,
}

impl Account {
    pub fn parsed_type(&self) -> Option<AccountType> {
        serde_json::from_value(serde_json::Value::String(self.account_type.clone())).ok()
    }

    pub fn tags_vec(&self) -> Vec<String> {
        serde_json::from_value(self.tags.clone()).unwrap_or_default()
    }
}

/// Request to create a new account.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateAccountRequest {
    pub code: AccountCode,
    pub name: String,
    pub account_type: AccountType,
    pub parent_code: Option<AccountCode>,
    pub currency: Option<String>,
    pub is_control: bool,
    pub tags: Vec<String>,
}

/// Request to update an account.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateAccountRequest {
    pub name: Option<String>,
    pub parent_code: Option<Option<AccountCode>>,
    pub currency: Option<Option<String>>,
    pub is_control: Option<bool>,
    pub is_active: Option<bool>,
    pub tags: Option<Vec<String>>,
}
