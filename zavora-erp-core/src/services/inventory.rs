use chrono::Utc;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::engine::ErpEngine;
use crate::error::{ErpError, ErpResult};
use crate::inventory::*;
use crate::types::AgentOrUserId;

/// Result of issuing inventory, including cost information for COGS posting.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IssueInventoryResult {
    /// The stock movement ID created for this issue.
    pub movement_id: Uuid,
    /// The unit cost at which goods were issued (WAC).
    pub unit_cost: Decimal,
    /// The total cost of goods issued (unit_cost × quantity).
    pub total_cost: Decimal,
    /// The GL account code for Inventory (e.g. "1500").
    pub gl_inventory: String,
    /// The GL account code for COGS (e.g. "6000").
    pub gl_cogs: String,
}

type PgTx<'a> = sqlx::Transaction<'a, sqlx::Postgres>;

/// Create an inventory item master record. Opening quantity/cost are zero;
/// stock arrives via receive/adjust. SKU is unique per entity.
pub async fn create_item(
    engine: &ErpEngine,
    entity_id: Uuid,
    req: CreateInventoryItemRequest,
) -> ErpResult<Uuid> {
    let sku = req.sku.trim();
    if sku.is_empty() {
        return Err(ErpError::ValidationFailed { message: "SKU is required.".to_string() });
    }
    if req.description.trim().is_empty() {
        return Err(ErpError::ValidationFailed { message: "Description is required.".to_string() });
    }

    // Reject duplicate SKU up-front for a clean error (also enforced by the unique index).
    let exists: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM inventory_items WHERE entity_id = $1 AND sku = $2)",
    )
    .bind(entity_id)
    .bind(sku)
    .fetch_one(engine.pool())
    .await?;
    if exists {
        return Err(ErpError::Duplicate {
            message: format!("An inventory item with SKU '{sku}' already exists."),
        });
    }

    let id = Uuid::new_v4();
    let posting = engine.posting_for(entity_id).await?;
    let gl_inventory = req.gl_inventory.clone().unwrap_or_else(|| posting.inventory_asset.clone());
    let gl_cogs = req.gl_cogs.clone().unwrap_or_else(|| posting.cost_of_goods_sold.clone());
    sqlx::query(
        r#"INSERT INTO inventory_items
            (id, entity_id, product_id, sku, description, uom, costing_method,
             gl_inventory, gl_cogs, on_hand, committed, available, unit_cost, total_value,
             reorder_point, reorder_quantity, warehouse_id, is_active, created_at)
           VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, 0, 0, 0, 0, 0, $10, $11, $12, true, $13)"#,
    )
    .bind(id)
    .bind(entity_id)
    .bind(req.product_id)
    .bind(req.sku.trim())
    .bind(req.description.trim())
    .bind(req.uom.unwrap_or_else(|| "Each".to_string()))
    .bind(req.costing_method.unwrap_or_else(|| "WeightedAvgCost".to_string()))
    .bind(req.gl_inventory.unwrap_or(gl_inventory))
    .bind(req.gl_cogs.unwrap_or(gl_cogs))
    .bind(req.reorder_point)
    .bind(req.reorder_quantity)
    .bind(req.warehouse_id)
    .bind(Utc::now())
    .execute(engine.pool())
    .await?;

    Ok(id)
}

/// Receive inventory (purchase receipt).
pub async fn receive_inventory(
    engine: &ErpEngine,
    entity_id: Uuid,
    req: ReceiveInventoryRequest,
    received_by: &AgentOrUserId,
) -> ErpResult<Uuid> {
    let mut tx = engine.pool().begin().await?;
    let id = receive_inventory_in_tx(&mut tx, entity_id, req, received_by).await?;
    tx.commit().await?;
    Ok(id)
}

/// Receive inventory within a caller-provided transaction.
pub async fn receive_inventory_in_tx(
    tx: &mut PgTx<'_>,
    entity_id: Uuid,
    req: ReceiveInventoryRequest,
    received_by: &AgentOrUserId,
) -> ErpResult<Uuid> {
    let movement_id = Uuid::new_v4();
    let today = req.date.unwrap_or_else(|| Utc::now().date_naive());

    // Update on-hand and recalculate weighted average cost
    sqlx::query(
        r#"UPDATE inventory_items SET 
           unit_cost = ((on_hand * unit_cost) + ($1 * $2)) / NULLIF(on_hand + $1, 0),
           on_hand = on_hand + $1,
           available = available + $1,
           total_value = (on_hand + $1) * (((on_hand * unit_cost) + ($1 * $2)) / NULLIF(on_hand + $1, 0))
           WHERE id = $3 AND entity_id = $4"#,
    )
    .bind(req.quantity)
    .bind(req.unit_cost)
    .bind(req.item_id)
    .bind(entity_id)
    .execute(&mut **tx)
    .await?;

    // Record stock movement
    sqlx::query(
        r#"INSERT INTO stock_movements
           (id, entity_id, item_id, movement_type, date, quantity, unit_cost, total_cost, reference_id, created_by, created_at)
           VALUES ($1, $2, $3, 'receipt', $4, $5, $6, $7, $8, $9, $10)"#,
    )
    .bind(movement_id)
    .bind(entity_id)
    .bind(req.item_id)
    .bind(today)
    .bind(req.quantity)
    .bind(req.unit_cost)
    .bind(req.quantity * req.unit_cost)
    .bind(req.reference_id)
    .bind(serde_json::to_value(received_by).unwrap_or_default())
    .bind(Utc::now())
    .execute(&mut **tx)
    .await?;

    Ok(movement_id)
}

/// Request for a stock-take adjustment.
#[derive(Debug, Clone, Deserialize)]
pub struct AdjustInventoryRequest {
    pub item_id: Uuid,
    /// The physical count; on-hand is set to this.
    pub counted_quantity: Decimal,
    /// GL account for the value variance (caller-supplied; not hardcoded).
    pub adjustment_account: String,
    #[serde(default)]
    pub reason: Option<String>,
    #[serde(default)]
    pub date: Option<chrono::NaiveDate>,
}

/// Stock-take adjustment: set on-hand to the counted quantity and post the value
/// variance (variance × unit cost) between the item's inventory GL account and a
/// caller-supplied adjustment account — a gain credits the adjustment account, a
/// loss debits it. Records an 'adjustment' stock movement.
pub async fn adjust_inventory(
    engine: &ErpEngine,
    entity_id: Uuid,
    req: AdjustInventoryRequest,
    actor: AgentOrUserId,
) -> ErpResult<Uuid> {
    use crate::ledger::journal::{CreateJournalEntryRequest, CreateJournalLineRequest, JournalSource};

    let mut tx = engine.pool().begin().await?;
    let item = sqlx::query_as::<_, InventoryItemRow>("SELECT * FROM inventory_items WHERE id = $1 AND entity_id = $2")
        .bind(req.item_id)
        .bind(entity_id)
        .fetch_optional(&mut *tx)
        .await?
        .ok_or_else(|| ErpError::NotFound { entity_type: "InventoryItem".to_string(), id: req.item_id })?;

    let today = req.date.unwrap_or_else(|| Utc::now().date_naive());
    let variance = req.counted_quantity - item.on_hand;
    if variance == Decimal::ZERO {
        return Err(ErpError::ValidationFailed { message: format!("Counted quantity matches on-hand for {}", item.sku) });
    }
    let value = (variance * item.unit_cost).round_dp(2);

    sqlx::query("UPDATE inventory_items SET on_hand = $1, available = available + $2, total_value = $1 * unit_cost WHERE id = $3 AND entity_id = $4")
        .bind(req.counted_quantity)
        .bind(variance)
        .bind(req.item_id)
        .bind(entity_id)
        .execute(&mut *tx)
        .await?;

    sqlx::query(
        r#"INSERT INTO stock_movements (id, entity_id, item_id, movement_type, date, quantity, unit_cost, total_cost, notes, created_by, created_at)
           VALUES ($1, $2, $3, 'adjustment', $4, $5, $6, $7, $8, $9, $10)"#,
    )
    .bind(Uuid::new_v4())
    .bind(entity_id)
    .bind(req.item_id)
    .bind(today)
    .bind(variance)
    .bind(item.unit_cost)
    .bind(value)
    .bind(req.reason.clone())
    .bind(serde_json::to_value(&actor).unwrap_or_default())
    .bind(Utc::now())
    .execute(&mut *tx)
    .await?;

    let abs_value = value.abs();
    if abs_value >= Decimal::new(1, 2) {
        let base_ccy = engine.config().base_currency.clone();
        let (dr_acct, cr_acct) = if variance > Decimal::ZERO {
            (item.gl_inventory.clone(), req.adjustment_account.clone())
        } else {
            (req.adjustment_account.clone(), item.gl_inventory.clone())
        };
        let lines = vec![
            CreateJournalLineRequest { account_code: dr_acct, debit: Some(abs_value), credit: None, currency: base_ccy.clone(), fx_rate: Some(Decimal::ONE), description: Some(format!("Stock adjustment {}", item.sku)), dimensions: None },
            CreateJournalLineRequest { account_code: cr_acct, debit: None, credit: Some(abs_value), currency: base_ccy.clone(), fx_rate: Some(Decimal::ONE), description: Some(format!("Stock adjustment {}", item.sku)), dimensions: None },
        ];
        let entry_req = CreateJournalEntryRequest {
            date: today,
            source: JournalSource::InventoryAdjustment,
            source_id: Some(req.item_id),
            reference: format!("STOCKADJ-{}", item.sku),
            description: req.reason.clone().unwrap_or_else(|| format!("Stock adjustment {}", item.sku)),
            lines,
            post_immediately: true,
        };
        let period = crate::services::periods::period_for_date(engine, entity_id, today).await?;
        crate::services::journal::create_and_post_in_tx(&mut tx, engine, entity_id, entry_req, period.id, actor).await?;
    }

    tx.commit().await?;
    Ok(req.item_id)
}

/// Issue inventory (sale/consumption).
///
/// Returns an `IssueInventoryResult` containing the movement ID and the cost
/// of goods issued, which callers (e.g. invoice posting) use for COGS journal lines.
pub async fn issue_inventory(
    engine: &ErpEngine,
    entity_id: Uuid,
    req: IssueInventoryRequest,
    issued_by: &AgentOrUserId,
) -> ErpResult<IssueInventoryResult> {
    let mut tx = engine.pool().begin().await?;
    let result = issue_inventory_in_tx(&mut tx, entity_id, req, issued_by).await?;
    tx.commit().await?;
    Ok(result)
}

/// Issue inventory within a caller-provided transaction.
pub async fn issue_inventory_in_tx(
    tx: &mut PgTx<'_>,
    entity_id: Uuid,
    req: IssueInventoryRequest,
    issued_by: &AgentOrUserId,
) -> ErpResult<IssueInventoryResult> {
    // Check available stock
    let item = sqlx::query_as::<_, InventoryItemRow>(
        "SELECT * FROM inventory_items WHERE id = $1 AND entity_id = $2",
    )
    .bind(req.item_id)
    .bind(entity_id)
    .fetch_optional(&mut **tx)
    .await?
    .ok_or_else(|| ErpError::NotFound {
        entity_type: "InventoryItem".to_string(),
        id: req.item_id,
    })?;

    if item.available < req.quantity {
        return Err(ErpError::InsufficientStock {
            sku: item.sku,
            available: item.available,
            requested: req.quantity,
        });
    }

    let movement_id = Uuid::new_v4();
    let today = req.date.unwrap_or_else(|| Utc::now().date_naive());
    let total_cost = req.quantity * item.unit_cost;

    // Update on-hand
    sqlx::query(
        r#"UPDATE inventory_items SET 
           on_hand = on_hand - $1,
           available = available - $1,
           total_value = (on_hand - $1) * unit_cost
           WHERE id = $2 AND entity_id = $3"#,
    )
    .bind(req.quantity)
    .bind(req.item_id)
    .bind(entity_id)
    .execute(&mut **tx)
    .await?;

    // Record movement
    sqlx::query(
        r#"INSERT INTO stock_movements
           (id, entity_id, item_id, movement_type, date, quantity, unit_cost, total_cost, reference_id, created_by, created_at)
           VALUES ($1, $2, $3, 'issue', $4, $5, $6, $7, $8, $9, $10)"#,
    )
    .bind(movement_id)
    .bind(entity_id)
    .bind(req.item_id)
    .bind(today)
    .bind(req.quantity)
    .bind(item.unit_cost)
    .bind(total_cost)
    .bind(req.reference_id)
    .bind(serde_json::to_value(issued_by).unwrap_or_default())
    .bind(Utc::now())
    .execute(&mut **tx)
    .await?;

    Ok(IssueInventoryResult {
        movement_id,
        unit_cost: item.unit_cost,
        total_cost,
        gl_inventory: item.gl_inventory,
        gl_cogs: item.gl_cogs,
    })
}
