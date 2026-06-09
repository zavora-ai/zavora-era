use chrono::NaiveDate;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

use crate::types::{AgentOrUserId, CurrencyCode};

/// Type of exchange rate.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum RateType {
    Spot,
    Revaluation,
    Budget,
}

/// An exchange rate record.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExchangeRate {
    pub id: Uuid,
    pub entity_id: Uuid,
    pub from_ccy: CurrencyCode,
    pub to_ccy: CurrencyCode,
    pub rate_date: NaiveDate,
    pub rate_type: RateType,
    pub rate: Decimal,
    pub source: String,
}

/// Database row for exchange rate.
#[derive(Debug, Clone, serde::Serialize, FromRow)]
pub struct ExchangeRateRow {
    pub id: Uuid,
    pub entity_id: Uuid,
    pub from_ccy: String,
    pub to_ccy: String,
    pub rate_date: NaiveDate,
    pub rate_type: String,
    pub rate: Decimal,
    pub source: String,
}

/// Result of FX revaluation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FxRevaluationReport {
    pub period_id: Uuid,
    pub rate_date: NaiveDate,
    pub revaluations: Vec<FxRevaluationLine>,
    pub total_gain: Decimal,
    pub total_loss: Decimal,
    pub net_impact: Decimal,
    pub journal_entry_id: Uuid,
    pub reversal_entry_id: Uuid,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FxRevaluationLine {
    pub account_code: String,
    pub account_name: String,
    pub currency: CurrencyCode,
    pub balance_fcy: Decimal,
    pub old_rate: Decimal,
    pub new_rate: Decimal,
    pub old_value_lcy: Decimal,
    pub new_value_lcy: Decimal,
    pub gain_loss: Decimal,
}

/// Request to run FX revaluation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunRevaluationRequest {
    pub period_id: Uuid,
    pub rate_date: NaiveDate,
    pub triggered_by: AgentOrUserId,
}

/// Request to create/update an exchange rate.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpsertRateRequest {
    pub from_ccy: CurrencyCode,
    pub to_ccy: CurrencyCode,
    pub rate_date: NaiveDate,
    pub rate_type: RateType,
    pub rate: Decimal,
    pub source: String,
}
