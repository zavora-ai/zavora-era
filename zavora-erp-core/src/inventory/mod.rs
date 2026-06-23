use chrono::{DateTime, NaiveDate, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

use crate::types::{AccountCode, AgentOrUserId, UnitOfMeasure};

/// Inventory costing method.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum CostingMethod {
    /// First In, First Out
    Fifo,
    /// Weighted Average Cost
    WeightedAvgCost,
}

/// An inventory item.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InventoryItem {
    pub id: Uuid,
    pub entity_id: Uuid,
    pub product_id: Option<Uuid>,
    pub sku: String,
    pub description: String,
    pub uom: UnitOfMeasure,
    pub costing_method: CostingMethod,
    pub gl_inventory: AccountCode,
    pub gl_cogs: AccountCode,
    pub on_hand: Decimal,
    pub committed: Decimal,  // reserved for orders
    pub available: Decimal,  // on_hand - committed
    pub unit_cost: Decimal,  // weighted avg cost or latest FIFO cost
    pub total_value: Decimal,
    pub reorder_point: Option<Decimal>,
    pub reorder_quantity: Option<Decimal>,
    pub warehouse_id: Option<Uuid>,
    pub is_active: bool,
    pub created_at: DateTime<Utc>,
}

impl InventoryItem {
    /// Recalculate available and total value.
    pub fn recalculate(&mut self) {
        self.available = self.on_hand - self.committed;
        self.total_value = (self.on_hand * self.unit_cost).round_dp(2);
    }

    /// Check if stock is below reorder point.
    pub fn needs_reorder(&self) -> bool {
        self.reorder_point
            .map_or(false, |rp| self.available <= rp)
    }
}

/// Database row for inventory item.
#[derive(Debug, Clone, serde::Serialize, FromRow)]
pub struct InventoryItemRow {
    pub id: Uuid,
    pub entity_id: Uuid,
    pub product_id: Option<Uuid>,
    pub sku: String,
    pub description: String,
    pub uom: String,
    pub costing_method: String,
    pub gl_inventory: String,
    pub gl_cogs: String,
    pub on_hand: Decimal,
    pub committed: Decimal,
    pub available: Decimal,
    pub unit_cost: Decimal,
    pub total_value: Decimal,
    pub reorder_point: Option<Decimal>,
    pub reorder_quantity: Option<Decimal>,
    pub warehouse_id: Option<Uuid>,
    pub is_active: bool,
    pub created_at: DateTime<Utc>,
}

/// Type of inventory movement.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum MovementType {
    Receipt,       // goods received (purchase)
    Issue,         // goods issued (sale)
    Adjustment,    // inventory count adjustment
    Transfer,      // between warehouses
    Return,        // customer/supplier return
}

/// An inventory stock movement.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StockMovement {
    pub id: Uuid,
    pub entity_id: Uuid,
    pub item_id: Uuid,
    pub movement_type: MovementType,
    pub date: NaiveDate,
    pub quantity: Decimal,
    pub unit_cost: Decimal,
    pub total_cost: Decimal,
    pub reference_type: Option<String>,
    pub reference_id: Option<Uuid>,
    pub warehouse_id: Option<Uuid>,
    pub notes: Option<String>,
    pub created_by: AgentOrUserId,
    pub created_at: DateTime<Utc>,
}

/// FIFO cost layer for tracking individual receipt lots.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FifoCostLayer {
    pub id: Uuid,
    pub item_id: Uuid,
    pub receipt_date: NaiveDate,
    pub original_quantity: Decimal,
    pub remaining_quantity: Decimal,
    pub unit_cost: Decimal,
}

/// Request to create an inventory item master record.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateInventoryItemRequest {
    pub sku: String,
    pub description: String,
    #[serde(default)]
    pub uom: Option<String>,
    #[serde(default)]
    pub costing_method: Option<String>,
    #[serde(default)]
    pub gl_inventory: Option<String>,
    #[serde(default)]
    pub gl_cogs: Option<String>,
    #[serde(default)]
    pub reorder_point: Option<Decimal>,
    #[serde(default)]
    pub reorder_quantity: Option<Decimal>,
    #[serde(default)]
    pub product_id: Option<Uuid>,
    #[serde(default)]
    pub warehouse_id: Option<Uuid>,
}

/// Request to receive inventory (purchase receipt).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReceiveInventoryRequest {
    pub item_id: Uuid,
    pub quantity: Decimal,
    pub unit_cost: Decimal,
    pub date: Option<NaiveDate>,
    pub reference_id: Option<Uuid>,
    pub warehouse_id: Option<Uuid>,
}

/// Request to issue inventory (sale/consumption).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IssueInventoryRequest {
    pub item_id: Uuid,
    pub quantity: Decimal,
    pub date: Option<NaiveDate>,
    pub reference_id: Option<Uuid>,
    pub warehouse_id: Option<Uuid>,
}

/// Request to adjust inventory count.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdjustInventoryRequest {
    pub item_id: Uuid,
    pub new_quantity: Decimal,
    pub reason: String,
    pub date: Option<NaiveDate>,
    pub adjusted_by: AgentOrUserId,
}
