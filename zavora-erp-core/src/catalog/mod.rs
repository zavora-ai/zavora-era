use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

use crate::types::{AccountCode, CurrencyCode, UnitOfMeasure, VatTreatment};

/// Product/service type classification.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum ProductType {
    /// A service (e.g. consulting hours)
    Service,
    /// Physical goods (may be tracked in inventory)
    Goods,
    /// An expense item (for bill lines)
    Expense,
}

/// A product or service in the catalog.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Product {
    pub id: Uuid,
    pub entity_id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub product_type: ProductType,
    pub unit_price: Option<Decimal>,
    pub currency: CurrencyCode,
    pub uom: UnitOfMeasure,
    pub sales_account: AccountCode,
    pub purchase_account: AccountCode,
    pub vat_treatment: VatTreatment,
    pub track_inventory: bool,
    pub inventory_item_id: Option<Uuid>,
    pub is_active: bool,
    pub created_at: DateTime<Utc>,
}

/// Database row for product.
#[derive(Debug, Clone, serde::Serialize, FromRow)]
pub struct ProductRow {
    pub id: Uuid,
    pub entity_id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub product_type: String,
    pub unit_price: Option<Decimal>,
    pub currency: String,
    pub uom: String,
    pub sales_account: String,
    pub purchase_account: String,
    pub vat_treatment: String,
    pub track_inventory: bool,
    pub inventory_item_id: Option<Uuid>,
    pub is_active: bool,
    pub created_at: DateTime<Utc>,
    #[serde(default)]
    pub general_product_group_id: Option<Uuid>,
    #[serde(default)]
    pub vat_product_group_id: Option<Uuid>,
}

impl ProductRow {
    /// Enum-backed columns (`product_type`, `uom`, `vat_treatment`) are persisted
    /// as serde_json strings, i.e. wrapped in quotes (`"Service"`). Strip the
    /// surrounding quotes so the API exposes clean values (`Service`) that the
    /// UI can compare directly. Tolerates already-clean values.
    pub fn normalized(mut self) -> Self {
        fn unquote(s: &str) -> String {
            s.strip_prefix('"').and_then(|x| x.strip_suffix('"')).unwrap_or(s).to_string()
        }
        self.product_type = unquote(&self.product_type);
        self.uom = unquote(&self.uom);
        self.vat_treatment = unquote(&self.vat_treatment);
        self
    }
}

/// Request to create a product.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateProductRequest {
    pub name: String,
    pub description: Option<String>,
    pub product_type: ProductType,
    pub unit_price: Option<Decimal>,
    pub currency: Option<CurrencyCode>,
    pub uom: Option<UnitOfMeasure>,
    pub sales_account: Option<AccountCode>,
    pub purchase_account: Option<AccountCode>,
    pub vat_treatment: Option<VatTreatment>,
    pub track_inventory: Option<bool>,
}

/// Request to update a product.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct UpdateProductRequest {
    pub name: Option<String>,
    pub description: Option<Option<String>>,
    pub product_type: Option<ProductType>,
    pub unit_price: Option<Option<Decimal>>,
    pub currency: Option<CurrencyCode>,
    pub uom: Option<UnitOfMeasure>,
    pub sales_account: Option<AccountCode>,
    pub purchase_account: Option<AccountCode>,
    pub vat_treatment: Option<VatTreatment>,
    pub track_inventory: Option<bool>,
    pub is_active: Option<bool>,
}
