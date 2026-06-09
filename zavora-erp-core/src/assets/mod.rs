use chrono::{DateTime, NaiveDate, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

use crate::types::AccountCode;

/// Asset category for grouping.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum AssetCategory {
    LandAndBuildings,
    MotorVehicles,
    PlantAndMachinery,
    FurnitureAndFittings,
    ComputerEquipment,
    Software,
    Other(String),
}

/// Depreciation method.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum DepreciationMethod {
    StraightLine,
    DecliningBalance { rate_percent: Decimal },
    KraTax { class: KraAssetClass },
}

/// KRA asset depreciation classes.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum KraAssetClass {
    /// Class 1: 37.5% declining balance — computers, software
    Class1,
    /// Class 2: 30% declining balance — motor vehicles
    Class2,
    /// Class 3: 25% declining balance — machinery, plant
    Class3,
    /// Class 4: 12.5% declining balance — industrial buildings
    Class4,
}

impl KraAssetClass {
    /// Returns the annual depreciation rate for this class.
    pub fn rate(&self) -> Decimal {
        match self {
            Self::Class1 => Decimal::new(375, 3), // 0.375
            Self::Class2 => Decimal::new(30, 2),  // 0.30
            Self::Class3 => Decimal::new(25, 2),  // 0.25
            Self::Class4 => Decimal::new(125, 3), // 0.125
        }
    }
}

/// Status of a fixed asset.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum AssetStatus {
    Active,
    FullyDepreciated,
    Disposed,
    WrittenOff,
}

/// A fixed asset record.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FixedAsset {
    pub id: Uuid,
    pub entity_id: Uuid,
    pub asset_number: String,
    pub description: String,
    pub category: AssetCategory,
    pub acquisition_date: NaiveDate,
    pub cost: Decimal,
    pub residual_value: Decimal,
    pub useful_life_months: u32,
    pub depreciation_method: DepreciationMethod,
    pub accumulated_depreciation: Decimal,
    pub net_book_value: Decimal,
    pub gl_asset_account: AccountCode,
    pub gl_accum_depr_account: AccountCode,
    pub gl_depr_expense: AccountCode,
    pub status: AssetStatus,
    pub disposal_date: Option<NaiveDate>,
    pub disposal_proceeds: Option<Decimal>,
    pub created_at: DateTime<Utc>,
}

impl FixedAsset {
    /// Compute monthly depreciation amount.
    pub fn monthly_depreciation(&self) -> Decimal {
        match &self.depreciation_method {
            DepreciationMethod::StraightLine => {
                if self.useful_life_months == 0 {
                    return Decimal::ZERO;
                }
                let depreciable = self.cost - self.residual_value;
                (depreciable / Decimal::from(self.useful_life_months)).round_dp(2)
            }
            DepreciationMethod::DecliningBalance { rate_percent } => {
                let annual_depr = self.net_book_value * rate_percent / Decimal::new(100, 0);
                (annual_depr / Decimal::new(12, 0)).round_dp(2)
            }
            DepreciationMethod::KraTax { class } => {
                let annual_depr = self.net_book_value * class.rate();
                (annual_depr / Decimal::new(12, 0)).round_dp(2)
            }
        }
    }

    /// Check if asset is fully depreciated.
    pub fn is_fully_depreciated(&self) -> bool {
        self.net_book_value <= self.residual_value
    }
}

/// Database row for fixed asset.
#[derive(Debug, Clone, FromRow)]
pub struct FixedAssetRow {
    pub id: Uuid,
    pub entity_id: Uuid,
    pub asset_number: String,
    pub description: String,
    pub category: String,
    pub acquisition_date: NaiveDate,
    pub cost: Decimal,
    pub residual_value: Decimal,
    pub useful_life_months: i32,
    pub depreciation_method: serde_json::Value,
    pub accumulated_depreciation: Decimal,
    pub net_book_value: Decimal,
    pub gl_asset_account: String,
    pub gl_accum_depr_account: String,
    pub gl_depr_expense: String,
    pub status: String,
    pub disposal_date: Option<NaiveDate>,
    pub disposal_proceeds: Option<Decimal>,
    pub created_at: DateTime<Utc>,
}

/// Request to create a fixed asset.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateAssetRequest {
    pub description: String,
    pub category: AssetCategory,
    pub acquisition_date: NaiveDate,
    pub cost: Decimal,
    pub residual_value: Option<Decimal>,
    pub useful_life_months: Option<u32>,
    pub depreciation_method: DepreciationMethod,
    pub gl_asset_account: Option<AccountCode>,
    pub gl_accum_depr_account: Option<AccountCode>,
    pub gl_depr_expense: Option<AccountCode>,
}

/// Request to dispose of an asset.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DisposeAssetRequest {
    pub asset_id: Uuid,
    pub disposal_date: NaiveDate,
    pub proceeds: Decimal,
    pub reason: String,
}
