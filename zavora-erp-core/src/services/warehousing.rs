//! Optional multi-warehouse + 3PL warehousing.
//!
//! A warehouse is a stock location — the company's own site or a third-party
//! logistics (3PL) provider. `warehouse_stock` tracks per-(item, warehouse)
//! quantities; the inventory item's `on_hand` stays the authoritative total, and
//! [`apply_stock_delta_tx`] (called from receive/issue/adjust) keeps
//! `SUM(warehouse_stock.quantity) == on_hand`. Transfers move stock between
//! warehouses without changing the total.

use chrono::NaiveDate;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::engine::ErpEngine;
use crate::error::{ErpError, ErpResult};
use crate::services::journal::PgTx;

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct Warehouse {
    pub id: Uuid,
    pub entity_id: Uuid,
    pub code: String,
    pub name: String,
    pub kind: String, // 'own' | '3pl'
    pub provider: Option<String>,
    pub location: Option<String>,
    pub is_default: bool,
    pub is_active: bool,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct WarehouseStockLine {
    pub warehouse_id: Uuid,
    pub code: String,
    pub name: String,
    pub kind: String,
    pub quantity: Decimal,
}

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct ItemStockLine {
    pub item_id: Uuid,
    pub sku: String,
    pub description: String,
    pub quantity: Decimal,
    pub unit_cost: Decimal,
}

#[derive(Debug, Deserialize)]
pub struct CreateWarehouseRequest {
    pub code: String,
    pub name: String,
    #[serde(default)]
    pub kind: Option<String>,
    #[serde(default)]
    pub provider: Option<String>,
    #[serde(default)]
    pub location: Option<String>,
    #[serde(default)]
    pub is_default: bool,
}

#[derive(Debug, Deserialize)]
pub struct TransferRequest {
    pub item_id: Uuid,
    pub from_warehouse_id: Uuid,
    pub to_warehouse_id: Uuid,
    pub quantity: Decimal,
    #[serde(default)]
    pub tx_date: Option<NaiveDate>,
    #[serde(default)]
    pub notes: Option<String>,
}

// ── Warehouse master ─────────────────────────────────────────────────────────

pub async fn list_warehouses(engine: &ErpEngine, entity_id: Uuid) -> ErpResult<Vec<Warehouse>> {
    Ok(sqlx::query_as::<_, Warehouse>(
        "SELECT * FROM warehouses WHERE entity_id = $1 ORDER BY is_default DESC, name",
    )
    .bind(entity_id)
    .fetch_all(engine.pool())
    .await?)
}

pub async fn create_warehouse(engine: &ErpEngine, entity_id: Uuid, req: CreateWarehouseRequest) -> ErpResult<Warehouse> {
    let code = req.code.trim().to_uppercase();
    let name = req.name.trim();
    if code.is_empty() || name.is_empty() {
        return Err(ErpError::ValidationFailed { message: "Warehouse code and name are required".into() });
    }
    let kind = match req.kind.as_deref() {
        Some("3pl") => "3pl",
        _ => "own",
    };
    // Auto-make the first OWN warehouse the default (never a 3PL — un-attributed
    // receipts/issues should settle in the company's own location).
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM warehouses WHERE entity_id = $1 AND kind = 'own'")
        .bind(entity_id)
        .fetch_one(engine.pool())
        .await?;
    let is_default = req.is_default || (kind == "own" && count == 0);
    if is_default {
        sqlx::query("UPDATE warehouses SET is_default = false WHERE entity_id = $1")
            .bind(entity_id)
            .execute(engine.pool())
            .await?;
    }
    let row = sqlx::query_as::<_, Warehouse>(
        "INSERT INTO warehouses (entity_id, code, name, kind, provider, location, is_default)
         VALUES ($1,$2,$3,$4,$5,$6,$7) RETURNING *",
    )
    .bind(entity_id)
    .bind(&code)
    .bind(name)
    .bind(kind)
    .bind(req.provider.filter(|p| !p.trim().is_empty()))
    .bind(req.location.filter(|l| !l.trim().is_empty()))
    .bind(is_default)
    .fetch_one(engine.pool())
    .await
    .map_err(|e| match e {
        sqlx::Error::Database(db) if db.constraint().is_some() => {
            ErpError::Duplicate { message: format!("A warehouse with code {code} already exists") }
        }
        other => ErpError::Database(other),
    })?;
    Ok(row)
}

#[derive(Debug, Deserialize)]
pub struct UpdateWarehouseRequest {
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub provider: Option<String>,
    #[serde(default)]
    pub location: Option<String>,
    #[serde(default)]
    pub is_active: Option<bool>,
}

pub async fn update_warehouse(engine: &ErpEngine, entity_id: Uuid, id: Uuid, req: UpdateWarehouseRequest) -> ErpResult<()> {
    sqlx::query(
        "UPDATE warehouses SET
            name = COALESCE($3, name),
            provider = COALESCE($4, provider),
            location = COALESCE($5, location),
            is_active = COALESCE($6, is_active)
         WHERE id = $1 AND entity_id = $2",
    )
    .bind(id)
    .bind(entity_id)
    .bind(req.name.filter(|s| !s.trim().is_empty()))
    .bind(req.provider)
    .bind(req.location)
    .bind(req.is_active)
    .execute(engine.pool())
    .await?;
    Ok(())
}

// ── Default-warehouse resolution + per-warehouse stock deltas ────────────────

/// Resolve (creating if needed) the entity's default warehouse id, within a tx.
pub async fn ensure_default_warehouse_tx(tx: &mut PgTx<'_>, entity_id: Uuid) -> ErpResult<Uuid> {
    if let Some(id) = sqlx::query_scalar::<_, Uuid>(
        "SELECT id FROM warehouses WHERE entity_id = $1 AND is_default = true LIMIT 1",
    )
    .bind(entity_id)
    .fetch_optional(&mut **tx)
    .await?
    {
        return Ok(id);
    }
    // Any warehouse? promote the first. Otherwise create MAIN.
    if let Some(id) = sqlx::query_scalar::<_, Uuid>("SELECT id FROM warehouses WHERE entity_id = $1 ORDER BY created_at LIMIT 1")
        .bind(entity_id)
        .fetch_optional(&mut **tx)
        .await?
    {
        sqlx::query("UPDATE warehouses SET is_default = true WHERE id = $1").bind(id).execute(&mut **tx).await?;
        return Ok(id);
    }
    let id = sqlx::query_scalar::<_, Uuid>(
        "INSERT INTO warehouses (entity_id, code, name, kind, is_default)
         VALUES ($1, 'MAIN', 'Main Warehouse', 'own', true) RETURNING id",
    )
    .bind(entity_id)
    .fetch_one(&mut **tx)
    .await?;
    Ok(id)
}

/// Apply a signed quantity delta to a warehouse's stock for an item, within a
/// tx. `warehouse_id = None` targets the entity's default warehouse. Keeps
/// warehouse_stock in step with inventory movements.
pub async fn apply_stock_delta_tx(
    tx: &mut PgTx<'_>,
    entity_id: Uuid,
    item_id: Uuid,
    warehouse_id: Option<Uuid>,
    delta: Decimal,
) -> ErpResult<()> {
    let wh = match warehouse_id {
        Some(w) => w,
        None => ensure_default_warehouse_tx(tx, entity_id).await?,
    };
    sqlx::query(
        "INSERT INTO warehouse_stock (entity_id, item_id, warehouse_id, quantity)
         VALUES ($1, $2, $3, $4)
         ON CONFLICT (item_id, warehouse_id)
         DO UPDATE SET quantity = warehouse_stock.quantity + $4",
    )
    .bind(entity_id)
    .bind(item_id)
    .bind(wh)
    .bind(delta)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

// ── Transfers + queries ──────────────────────────────────────────────────────

/// Move stock between two warehouses (no change to the item's total on_hand).
pub async fn transfer_stock(engine: &ErpEngine, entity_id: Uuid, req: TransferRequest, created_by: Uuid) -> ErpResult<()> {
    let qty = req.quantity.round_dp(4);
    if qty <= Decimal::ZERO {
        return Err(ErpError::ValidationFailed { message: "Transfer quantity must be positive".into() });
    }
    if req.from_warehouse_id == req.to_warehouse_id {
        return Err(ErpError::ValidationFailed { message: "Choose two different warehouses".into() });
    }
    let tx_date = req.tx_date.unwrap_or_else(|| chrono::Utc::now().date_naive());

    let mut tx = engine.pool().begin().await?;
    let available: Decimal = sqlx::query_scalar::<_, Option<Decimal>>(
        "SELECT quantity FROM warehouse_stock WHERE item_id = $1 AND warehouse_id = $2",
    )
    .bind(req.item_id)
    .bind(req.from_warehouse_id)
    .fetch_optional(&mut *tx)
    .await?
    .flatten()
    .unwrap_or(Decimal::ZERO);
    if available < qty {
        return Err(ErpError::InsufficientStock { sku: "item".into(), available, requested: qty });
    }

    apply_stock_delta_tx(&mut tx, entity_id, req.item_id, Some(req.from_warehouse_id), -qty).await?;
    apply_stock_delta_tx(&mut tx, entity_id, req.item_id, Some(req.to_warehouse_id), qty).await?;

    sqlx::query(
        "INSERT INTO warehouse_transfers (entity_id, item_id, from_warehouse_id, to_warehouse_id, quantity, tx_date, notes, created_by)
         VALUES ($1,$2,$3,$4,$5,$6,$7,$8)",
    )
    .bind(entity_id)
    .bind(req.item_id)
    .bind(req.from_warehouse_id)
    .bind(req.to_warehouse_id)
    .bind(qty)
    .bind(tx_date)
    .bind(req.notes.filter(|n| !n.trim().is_empty()))
    .bind(created_by)
    .execute(&mut *tx)
    .await?;

    tx.commit().await?;
    Ok(())
}

/// Where an item's stock sits, per warehouse.
pub async fn stock_for_item(engine: &ErpEngine, entity_id: Uuid, item_id: Uuid) -> ErpResult<Vec<WarehouseStockLine>> {
    Ok(sqlx::query_as::<_, WarehouseStockLine>(
        "SELECT w.id AS warehouse_id, w.code, w.name, w.kind, COALESCE(ws.quantity, 0) AS quantity
         FROM warehouses w
         LEFT JOIN warehouse_stock ws ON ws.warehouse_id = w.id AND ws.item_id = $2
         WHERE w.entity_id = $1 AND w.is_active = true
         ORDER BY w.is_default DESC, w.name",
    )
    .bind(entity_id)
    .bind(item_id)
    .fetch_all(engine.pool())
    .await?)
}

/// What's stored in a warehouse.
pub async fn stock_in_warehouse(engine: &ErpEngine, entity_id: Uuid, warehouse_id: Uuid) -> ErpResult<Vec<ItemStockLine>> {
    Ok(sqlx::query_as::<_, ItemStockLine>(
        "SELECT i.id AS item_id, i.sku, i.description, ws.quantity, i.unit_cost
         FROM warehouse_stock ws
         JOIN inventory_items i ON i.id = ws.item_id
         WHERE ws.entity_id = $1 AND ws.warehouse_id = $2 AND ws.quantity <> 0
         ORDER BY i.sku",
    )
    .bind(entity_id)
    .bind(warehouse_id)
    .fetch_all(engine.pool())
    .await?)
}
