use chrono::Utc;
use rust_decimal::Decimal;
use uuid::Uuid;

use crate::engine::ErpEngine;
use crate::error::{ErpError, ErpResult};
use crate::inventory::*;
use crate::types::AgentOrUserId;

/// Receive inventory (purchase receipt).
pub async fn receive_inventory(
    engine: &ErpEngine,
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
    .bind(engine.entity_id())
    .execute(engine.pool())
    .await?;

    // Record stock movement
    sqlx::query(
        r#"INSERT INTO stock_movements 
           (id, entity_id, item_id, movement_type, date, quantity, unit_cost, total_cost, reference_id, created_by, created_at)
           VALUES ($1, $2, $3, 'receipt', $4, $5, $6, $7, $8, $9, $10)"#,
    )
    .bind(movement_id)
    .bind(engine.entity_id())
    .bind(req.item_id)
    .bind(today)
    .bind(req.quantity)
    .bind(req.unit_cost)
    .bind(req.quantity * req.unit_cost)
    .bind(req.reference_id)
    .bind(serde_json::to_value(received_by).unwrap_or_default())
    .bind(Utc::now())
    .execute(engine.pool())
    .await?;

    Ok(movement_id)
}

/// Issue inventory (sale/consumption).
pub async fn issue_inventory(
    engine: &ErpEngine,
    req: IssueInventoryRequest,
    issued_by: &AgentOrUserId,
) -> ErpResult<Uuid> {
    // Check available stock
    let item = sqlx::query_as::<_, InventoryItemRow>(
        "SELECT * FROM inventory_items WHERE id = $1 AND entity_id = $2",
    )
    .bind(req.item_id)
    .bind(engine.entity_id())
    .fetch_optional(engine.pool())
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
    .bind(engine.entity_id())
    .execute(engine.pool())
    .await?;

    // Record movement
    sqlx::query(
        r#"INSERT INTO stock_movements 
           (id, entity_id, item_id, movement_type, date, quantity, unit_cost, total_cost, reference_id, created_by, created_at)
           VALUES ($1, $2, $3, 'issue', $4, $5, $6, $7, $8, $9, $10)"#,
    )
    .bind(movement_id)
    .bind(engine.entity_id())
    .bind(req.item_id)
    .bind(today)
    .bind(req.quantity)
    .bind(item.unit_cost)
    .bind(req.quantity * item.unit_cost)
    .bind(req.reference_id)
    .bind(serde_json::to_value(issued_by).unwrap_or_default())
    .bind(Utc::now())
    .execute(engine.pool())
    .await?;

    Ok(movement_id)
}
