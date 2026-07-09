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

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    /// Build a minimal asset for exercising `monthly_depreciation`. Only the
    /// fields that drive the calculation matter; the rest are placeholders.
    fn asset(
        cost: Decimal,
        residual: Decimal,
        life_months: u32,
        net_book_value: Decimal,
        method: DepreciationMethod,
    ) -> FixedAsset {
        FixedAsset {
            id: Uuid::new_v4(),
            entity_id: Uuid::new_v4(),
            asset_number: "FA-0001".into(),
            description: "test asset".into(),
            category: AssetCategory::Other("test".into()),
            acquisition_date: NaiveDate::from_ymd_opt(2026, 1, 1).unwrap(),
            cost,
            residual_value: residual,
            useful_life_months: life_months,
            depreciation_method: method,
            accumulated_depreciation: cost - net_book_value,
            net_book_value,
            gl_asset_account: "2500".into(),
            gl_accum_depr_account: "2520".into(),
            gl_depr_expense: "7600".into(),
            status: AssetStatus::Active,
            disposal_date: None,
            disposal_proceeds: None,
            created_at: Utc::now(),
        }
    }

    #[test]
    fn kra_class_rates_match_finance_act() {
        // KRA wear-and-tear classes: 37.5% / 30% / 25% / 12.5%.
        assert_eq!(KraAssetClass::Class1.rate(), dec!(0.375));
        assert_eq!(KraAssetClass::Class2.rate(), dec!(0.30));
        assert_eq!(KraAssetClass::Class3.rate(), dec!(0.25));
        assert_eq!(KraAssetClass::Class4.rate(), dec!(0.125));
    }

    #[test]
    fn straight_line_spreads_cost_over_life() {
        // (120,000 - 0) / 60 months = 2,000 per month.
        let a = asset(dec!(120_000), dec!(0), 60, dec!(120_000), DepreciationMethod::StraightLine);
        assert_eq!(a.monthly_depreciation(), dec!(2_000));
    }

    #[test]
    fn straight_line_nets_off_residual() {
        // (100,000 - 10,000) / 36 = 2,500 per month.
        let a = asset(dec!(100_000), dec!(10_000), 36, dec!(100_000), DepreciationMethod::StraightLine);
        assert_eq!(a.monthly_depreciation(), dec!(2_500));
    }

    #[test]
    fn declining_balance_is_rate_on_nbv_over_twelve() {
        // 25% declining on a 100,000 NBV: 25,000 / 12 = 2,083.33.
        let a = asset(
            dec!(100_000), dec!(0), 0, dec!(100_000),
            DepreciationMethod::DecliningBalance { rate_percent: dec!(25) },
        );
        assert_eq!(a.monthly_depreciation(), dec!(2_083.33));
    }

    #[test]
    fn kra_class1_computers_at_37_5_percent() {
        // Class 1 (37.5%) on a 1,000,000 NBV: 375,000 / 12 = 31,250.
        let a = asset(
            dec!(1_000_000), dec!(0), 0, dec!(1_000_000),
            DepreciationMethod::KraTax { class: KraAssetClass::Class1 },
        );
        assert_eq!(a.monthly_depreciation(), dec!(31_250));
    }

    #[test]
    fn kra_class4_buildings_at_12_5_percent() {
        // Class 4 (12.5%) on an 800,000 NBV: 100,000 / 12 = 8,333.33.
        let a = asset(
            dec!(800_000), dec!(0), 0, dec!(800_000),
            DepreciationMethod::KraTax { class: KraAssetClass::Class4 },
        );
        assert_eq!(a.monthly_depreciation(), dec!(8_333.33));
    }

    #[test]
    fn straight_line_with_zero_life_does_not_panic() {
        let a = asset(dec!(50_000), dec!(0), 0, dec!(50_000), DepreciationMethod::StraightLine);
        assert_eq!(a.monthly_depreciation(), Decimal::ZERO);
    }
}

/// Database row for fixed asset.
#[derive(Debug, Clone, serde::Serialize, FromRow)]
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
    /// Month-end through which depreciation has already been posted (NULL = none).
    /// Makes a depreciation run idempotent and supports catch-up.
    #[sqlx(default)]
    pub depreciated_through: Option<NaiveDate>,
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
    /// When set, a capitalisation JE is posted: DR asset account / CR this
    /// account (bank for a direct purchase, AP for on-credit, opening-balance
    /// equity for takeover balances). Leave `None` when the cost already
    /// reached the GL another way — e.g. a bill line coded to the asset
    /// account — otherwise the cost would post twice.
    #[serde(default)]
    pub funding_account: Option<AccountCode>,
}

/// Request to dispose of an asset.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DisposeAssetRequest {
    pub asset_id: Uuid,
    pub disposal_date: NaiveDate,
    pub proceeds: Decimal,
    pub reason: String,
}
