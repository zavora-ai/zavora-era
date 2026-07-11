//! Manufacturing v1 — Bills of Materials + Work Orders.
//!
//! Light manufacturing layered on the existing inventory (WAC) + warehousing +
//! journal engine. A finished-good product has a **BOM** (a recipe of component
//! items + optional labour/overhead per batch); a **work order** produces N
//! finished units in two steps:
//!
//!   • start    — issue the (scaled) components out of stock into Work in
//!                Progress: `DR 1510 WIP / CR component inventory` (at WAC).
//!   • complete — receive the finished goods out of WIP at their rolled-up unit
//!                cost: `DR finished-good inventory / CR 1510 WIP (material) /
//!                CR 6300 Manufacturing Overhead (overhead)`.
//!
//! WIP carries component cost during production and nets to zero on completion.
//! All stock moves reuse `inventory::{issue,receive}_inventory_in_tx`, so the
//! warehouse ledger stays consistent (`SUM(warehouse_stock) == on_hand`) and
//! WAC is recomputed correctly.

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

use crate::engine::ErpEngine;
use crate::error::{ErpError, ErpResult};
use crate::inventory::{IssueInventoryRequest, ReceiveInventoryRequest};
use crate::ledger::journal::{CreateJournalEntryRequest, CreateJournalLineRequest, JournalSource};
use crate::types::AgentOrUserId;

type PgTx<'a> = sqlx::Transaction<'a, sqlx::Postgres>;

// ─── Models ──────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, FromRow)]
pub struct BomRow {
    pub id: Uuid,
    pub entity_id: Uuid,
    pub product_id: Uuid,
    pub output_item_id: Uuid,
    pub output_quantity: Decimal,
    pub overhead_cost: Decimal,
    pub notes: Option<String>,
    pub is_active: bool,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, FromRow)]
pub struct BomLineRow {
    pub id: Uuid,
    pub bom_id: Uuid,
    pub component_item_id: Uuid,
    pub quantity: Decimal,
    pub notes: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct Bom {
    #[serde(flatten)]
    pub row: BomRow,
    /// Finished-good product name (convenience for the UI).
    pub product_name: Option<String>,
    pub lines: Vec<BomLineView>,
}

#[derive(Debug, Clone, Serialize)]
pub struct BomLineView {
    #[serde(flatten)]
    pub row: BomLineRow,
    pub component_sku: Option<String>,
    pub component_description: Option<String>,
    pub unit_cost: Decimal,
}

#[derive(Debug, Clone, Serialize, FromRow)]
pub struct WorkOrderRow {
    pub id: Uuid,
    pub entity_id: Uuid,
    pub number: String,
    pub bom_id: Uuid,
    pub output_item_id: Uuid,
    pub quantity: Decimal,
    pub status: String,
    pub source_warehouse_id: Option<Uuid>,
    pub dest_warehouse_id: Option<Uuid>,
    pub material_cost: Decimal,
    pub overhead_cost: Decimal,
    pub total_cost: Decimal,
    pub output_unit_cost: Decimal,
    pub notes: Option<String>,
    pub start_journal_id: Option<Uuid>,
    pub complete_journal_id: Option<Uuid>,
    pub started_at: Option<DateTime<Utc>>,
    pub completed_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, FromRow)]
pub struct WorkOrderConsumption {
    pub id: Uuid,
    pub work_order_id: Uuid,
    pub component_item_id: Uuid,
    pub quantity: Decimal,
    pub unit_cost: Decimal,
    pub total_cost: Decimal,
}

#[derive(Debug, Clone, Serialize)]
pub struct WorkOrder {
    #[serde(flatten)]
    pub row: WorkOrderRow,
    pub product_name: Option<String>,
    pub output_sku: Option<String>,
    pub consumptions: Vec<WorkOrderConsumption>,
}

// ─── Requests ────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Deserialize)]
pub struct BomLineInput {
    pub component_item_id: Uuid,
    pub quantity: Decimal,
    #[serde(default)]
    pub notes: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CreateBomRequest {
    /// Finished-good product (must be inventory-tracked).
    pub product_id: Uuid,
    #[serde(default = "one")]
    pub output_quantity: Decimal,
    #[serde(default)]
    pub overhead_cost: Decimal,
    #[serde(default)]
    pub notes: Option<String>,
    pub lines: Vec<BomLineInput>,
}

fn one() -> Decimal {
    Decimal::ONE
}

#[derive(Debug, Clone, Deserialize)]
pub struct CreateWorkOrderRequest {
    pub bom_id: Uuid,
    /// Finished units to produce.
    pub quantity: Decimal,
    #[serde(default)]
    pub source_warehouse_id: Option<Uuid>,
    #[serde(default)]
    pub dest_warehouse_id: Option<Uuid>,
    /// Override the overhead applied (defaults to the BOM overhead scaled to
    /// this run's quantity).
    #[serde(default)]
    pub overhead_cost: Option<Decimal>,
    #[serde(default)]
    pub notes: Option<String>,
}

// ─── BOM CRUD ──────────────────────────────────────────────────────────────

/// Resolve the inventory item behind a finished-good product; errors if the
/// product isn't inventory-tracked (manufacturing produces stock).
async fn output_item_for_product(engine: &ErpEngine, entity_id: Uuid, product_id: Uuid) -> ErpResult<Uuid> {
    let item: Option<Uuid> = sqlx::query_scalar(
        "SELECT inventory_item_id FROM products WHERE id = $1 AND entity_id = $2",
    )
    .bind(product_id)
    .bind(entity_id)
    .fetch_optional(engine.pool())
    .await?
    .flatten();
    item.ok_or_else(|| ErpError::ValidationFailed {
        message: "The finished good must be an inventory-tracked product (enable 'track inventory' with a SKU first).".to_string(),
    })
}

fn validate_bom_lines(lines: &[BomLineInput]) -> ErpResult<()> {
    if lines.is_empty() {
        return Err(ErpError::ValidationFailed { message: "A BOM needs at least one component line.".to_string() });
    }
    for l in lines {
        if l.quantity <= Decimal::ZERO {
            return Err(ErpError::ValidationFailed { message: "Component quantities must be greater than zero.".to_string() });
        }
    }
    Ok(())
}

pub async fn create_bom(engine: &ErpEngine, entity_id: Uuid, req: CreateBomRequest) -> ErpResult<Uuid> {
    validate_bom_lines(&req.lines)?;
    if req.output_quantity <= Decimal::ZERO {
        return Err(ErpError::ValidationFailed { message: "Output quantity must be greater than zero.".to_string() });
    }
    let output_item_id = output_item_for_product(engine, entity_id, req.product_id).await?;

    let exists: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM boms WHERE entity_id = $1 AND product_id = $2)")
        .bind(entity_id)
        .bind(req.product_id)
        .fetch_one(engine.pool())
        .await?;
    if exists {
        return Err(ErpError::Duplicate { message: "This product already has a bill of materials.".to_string() });
    }

    let bom_id = Uuid::new_v4();
    let mut tx = engine.pool().begin().await?;
    sqlx::query(
        r#"INSERT INTO boms (id, entity_id, product_id, output_item_id, output_quantity, overhead_cost, notes, is_active, created_at)
           VALUES ($1, $2, $3, $4, $5, $6, $7, true, $8)"#,
    )
    .bind(bom_id)
    .bind(entity_id)
    .bind(req.product_id)
    .bind(output_item_id)
    .bind(req.output_quantity)
    .bind(req.overhead_cost)
    .bind(req.notes.as_deref())
    .bind(Utc::now())
    .execute(&mut *tx)
    .await?;
    insert_bom_lines(&mut tx, bom_id, &req.lines).await?;
    tx.commit().await?;
    Ok(bom_id)
}

async fn insert_bom_lines(tx: &mut PgTx<'_>, bom_id: Uuid, lines: &[BomLineInput]) -> ErpResult<()> {
    for l in lines {
        sqlx::query("INSERT INTO bom_lines (id, bom_id, component_item_id, quantity, notes) VALUES ($1, $2, $3, $4, $5)")
            .bind(Uuid::new_v4())
            .bind(bom_id)
            .bind(l.component_item_id)
            .bind(l.quantity)
            .bind(l.notes.as_deref())
            .execute(&mut **tx)
            .await?;
    }
    Ok(())
}

pub async fn update_bom(engine: &ErpEngine, entity_id: Uuid, bom_id: Uuid, req: CreateBomRequest) -> ErpResult<()> {
    validate_bom_lines(&req.lines)?;
    if req.output_quantity <= Decimal::ZERO {
        return Err(ErpError::ValidationFailed { message: "Output quantity must be greater than zero.".to_string() });
    }
    let mut tx = engine.pool().begin().await?;
    let updated = sqlx::query(
        "UPDATE boms SET output_quantity = $1, overhead_cost = $2, notes = $3 WHERE id = $4 AND entity_id = $5",
    )
    .bind(req.output_quantity)
    .bind(req.overhead_cost)
    .bind(req.notes.as_deref())
    .bind(bom_id)
    .bind(entity_id)
    .execute(&mut *tx)
    .await?;
    if updated.rows_affected() == 0 {
        return Err(ErpError::NotFound { entity_type: "Bom".to_string(), id: bom_id });
    }
    sqlx::query("DELETE FROM bom_lines WHERE bom_id = $1").bind(bom_id).execute(&mut *tx).await?;
    insert_bom_lines(&mut tx, bom_id, &req.lines).await?;
    tx.commit().await?;
    Ok(())
}

pub async fn list_boms(engine: &ErpEngine, entity_id: Uuid) -> ErpResult<Vec<Bom>> {
    let rows = sqlx::query_as::<_, BomRow>("SELECT * FROM boms WHERE entity_id = $1 ORDER BY created_at DESC")
        .bind(entity_id)
        .fetch_all(engine.pool())
        .await?;
    let mut out = Vec::with_capacity(rows.len());
    for row in rows {
        out.push(hydrate_bom(engine, entity_id, row).await?);
    }
    Ok(out)
}

pub async fn get_bom(engine: &ErpEngine, entity_id: Uuid, bom_id: Uuid) -> ErpResult<Bom> {
    let row = sqlx::query_as::<_, BomRow>("SELECT * FROM boms WHERE id = $1 AND entity_id = $2")
        .bind(bom_id)
        .bind(entity_id)
        .fetch_optional(engine.pool())
        .await?
        .ok_or_else(|| ErpError::NotFound { entity_type: "Bom".to_string(), id: bom_id })?;
    hydrate_bom(engine, entity_id, row).await
}

async fn hydrate_bom(engine: &ErpEngine, entity_id: Uuid, row: BomRow) -> ErpResult<Bom> {
    let product_name: Option<String> = sqlx::query_scalar("SELECT name FROM products WHERE id = $1 AND entity_id = $2")
        .bind(row.product_id)
        .bind(entity_id)
        .fetch_optional(engine.pool())
        .await?;
    let line_rows = sqlx::query_as::<_, BomLineRow>("SELECT * FROM bom_lines WHERE bom_id = $1 ORDER BY id")
        .bind(row.id)
        .fetch_all(engine.pool())
        .await?;
    let mut lines = Vec::with_capacity(line_rows.len());
    for lr in line_rows {
        let item: Option<(String, String, Decimal)> = sqlx::query_as(
            "SELECT sku, description, unit_cost FROM inventory_items WHERE id = $1 AND entity_id = $2",
        )
        .bind(lr.component_item_id)
        .bind(entity_id)
        .fetch_optional(engine.pool())
        .await?;
        let (sku, desc, unit_cost) = match item {
            Some((s, d, c)) => (Some(s), Some(d), c),
            None => (None, None, Decimal::ZERO),
        };
        lines.push(BomLineView { row: lr, component_sku: sku, component_description: desc, unit_cost });
    }
    Ok(Bom { row, product_name, lines })
}

// ─── Work orders ─────────────────────────────────────────────────────────────

pub async fn create_work_order(
    engine: &ErpEngine,
    entity_id: Uuid,
    req: CreateWorkOrderRequest,
    created_by: &AgentOrUserId,
) -> ErpResult<Uuid> {
    if req.quantity <= Decimal::ZERO {
        return Err(ErpError::ValidationFailed { message: "Quantity to produce must be greater than zero.".to_string() });
    }
    let bom = get_bom(engine, entity_id, req.bom_id).await?;
    let multiplier = req.quantity / bom.row.output_quantity;
    let overhead = req.overhead_cost.unwrap_or((bom.row.overhead_cost * multiplier).round_dp(2));

    let number = super::procurement::next_number(engine, entity_id, "work_order_next", "WO", Utc::now().date_naive()).await?;
    let id = Uuid::new_v4();
    sqlx::query(
        r#"INSERT INTO work_orders
           (id, entity_id, number, bom_id, output_item_id, quantity, status,
            source_warehouse_id, dest_warehouse_id, material_cost, overhead_cost,
            total_cost, output_unit_cost, notes, created_by, created_at)
           VALUES ($1,$2,$3,$4,$5,$6,'draft',$7,$8,0,$9,0,0,$10,$11,$12)"#,
    )
    .bind(id)
    .bind(entity_id)
    .bind(&number)
    .bind(req.bom_id)
    .bind(bom.row.output_item_id)
    .bind(req.quantity)
    .bind(req.source_warehouse_id)
    .bind(req.dest_warehouse_id)
    .bind(overhead)
    .bind(req.notes.as_deref())
    .bind(serde_json::to_value(created_by).unwrap_or_default())
    .bind(Utc::now())
    .execute(engine.pool())
    .await?;
    Ok(id)
}

/// Start a draft work order: issue the scaled components into WIP and post
/// `DR WIP / CR component inventory`.
pub async fn start_work_order(engine: &ErpEngine, entity_id: Uuid, id: Uuid, actor: AgentOrUserId) -> ErpResult<WorkOrder> {
    let wo = load_work_order_row(engine, entity_id, id).await?;
    if wo.status != "draft" {
        return Err(ErpError::ValidationFailed { message: format!("Work order {} is {}, not draft — cannot start.", wo.number, wo.status) });
    }
    let bom = get_bom(engine, entity_id, wo.bom_id).await?;
    let multiplier = wo.quantity / bom.row.output_quantity;
    let today = Utc::now().date_naive();
    let posting = engine.posting_for(entity_id).await?;
    let base_ccy = engine.config_for(entity_id).await?.base_currency.clone();

    let mut tx = engine.pool().begin().await?;

    // Consume each component at WAC; aggregate the credit per inventory account.
    let mut material_cost = Decimal::ZERO;
    let mut credit_by_account: std::collections::BTreeMap<String, Decimal> = std::collections::BTreeMap::new();
    for line in &bom.lines {
        let qty = (line.row.quantity * multiplier).round_dp(4);
        if qty <= Decimal::ZERO {
            continue;
        }
        let issue = crate::services::inventory::issue_inventory_in_tx(
            &mut tx,
            entity_id,
            IssueInventoryRequest { item_id: line.row.component_item_id, quantity: qty, date: Some(today), reference_id: Some(id), warehouse_id: wo.source_warehouse_id },
            &actor,
        )
        .await?;
        material_cost += issue.total_cost;
        *credit_by_account.entry(issue.gl_inventory.clone()).or_insert(Decimal::ZERO) += issue.total_cost;
        sqlx::query(
            "INSERT INTO work_order_consumptions (id, work_order_id, component_item_id, quantity, unit_cost, total_cost) VALUES ($1,$2,$3,$4,$5,$6)",
        )
        .bind(Uuid::new_v4())
        .bind(id)
        .bind(line.row.component_item_id)
        .bind(qty)
        .bind(issue.unit_cost)
        .bind(issue.total_cost)
        .execute(&mut *tx)
        .await?;
    }
    let material_cost = material_cost.round_dp(2);

    // DR WIP (material) / CR component inventory accounts.
    if material_cost >= Decimal::new(1, 2) {
        let mut lines = vec![CreateJournalLineRequest {
            account_code: posting.work_in_progress.clone(),
            debit: Some(material_cost),
            credit: None,
            currency: base_ccy.clone(),
            fx_rate: Some(Decimal::ONE),
            description: Some(format!("WIP: components issued to {}", wo.number)),
            dimensions: None,
        }];
        for (acct, amt) in &credit_by_account {
            let amt = amt.round_dp(2);
            if amt <= Decimal::ZERO {
                continue;
            }
            lines.push(CreateJournalLineRequest {
                account_code: acct.clone(),
                debit: None,
                credit: Some(amt),
                currency: base_ccy.clone(),
                fx_rate: Some(Decimal::ONE),
                description: Some(format!("Components consumed for {}", wo.number)),
                dimensions: None,
            });
        }
        let entry = post_je(engine, entity_id, &mut tx, today, id, &wo.number, "Components issued to production", lines, actor.clone()).await?;
        sqlx::query("UPDATE work_orders SET status = 'in_progress', material_cost = $1, started_at = $2, start_journal_id = $3 WHERE id = $4 AND entity_id = $5")
            .bind(material_cost)
            .bind(Utc::now())
            .bind(entry)
            .bind(id)
            .bind(entity_id)
            .execute(&mut *tx)
            .await?;
    } else {
        // Zero-cost components (e.g. all at zero WAC) — still advance state.
        sqlx::query("UPDATE work_orders SET status = 'in_progress', material_cost = $1, started_at = $2 WHERE id = $3 AND entity_id = $4")
            .bind(material_cost)
            .bind(Utc::now())
            .bind(id)
            .bind(entity_id)
            .execute(&mut *tx)
            .await?;
    }

    tx.commit().await?;
    get_work_order(engine, entity_id, id).await
}

/// Complete an in-progress work order: receive the finished goods out of WIP at
/// their rolled-up unit cost and post `DR finished-good inventory / CR WIP
/// (material) / CR Manufacturing Overhead (overhead)`.
pub async fn complete_work_order(engine: &ErpEngine, entity_id: Uuid, id: Uuid, actor: AgentOrUserId) -> ErpResult<WorkOrder> {
    let wo = load_work_order_row(engine, entity_id, id).await?;
    if wo.status != "in_progress" {
        return Err(ErpError::ValidationFailed { message: format!("Work order {} is {}, not in progress — cannot complete.", wo.number, wo.status) });
    }
    let today = Utc::now().date_naive();
    let posting = engine.posting_for(entity_id).await?;
    let base_ccy = engine.config_for(entity_id).await?.base_currency.clone();
    let material = wo.material_cost.round_dp(2);
    let overhead = wo.overhead_cost.round_dp(2);
    let total = (material + overhead).round_dp(2);
    let unit_cost = if wo.quantity > Decimal::ZERO { (total / wo.quantity).round_dp(4) } else { Decimal::ZERO };

    // Finished-good inventory GL account.
    let fg_inventory: String = sqlx::query_scalar("SELECT gl_inventory FROM inventory_items WHERE id = $1 AND entity_id = $2")
        .bind(wo.output_item_id)
        .bind(entity_id)
        .fetch_optional(engine.pool())
        .await?
        .filter(|s: &String| !s.is_empty())
        .unwrap_or_else(|| posting.inventory_asset.clone());

    let mut tx = engine.pool().begin().await?;

    // Receive finished goods into stock (WAC + warehouse), GL-free.
    crate::services::inventory::receive_inventory_in_tx(
        &mut tx,
        entity_id,
        ReceiveInventoryRequest { item_id: wo.output_item_id, quantity: wo.quantity, unit_cost, date: Some(today), reference_id: Some(id), warehouse_id: wo.dest_warehouse_id },
        &actor,
    )
    .await?;

    // DR finished-good inventory (total) / CR WIP (material) / CR overhead (overhead).
    let mut complete_journal: Option<Uuid> = None;
    if total >= Decimal::new(1, 2) {
        let mut lines = vec![CreateJournalLineRequest {
            account_code: fg_inventory,
            debit: Some(total),
            credit: None,
            currency: base_ccy.clone(),
            fx_rate: Some(Decimal::ONE),
            description: Some(format!("Finished goods from {}", wo.number)),
            dimensions: None,
        }];
        if material >= Decimal::new(1, 2) {
            lines.push(CreateJournalLineRequest {
                account_code: posting.work_in_progress.clone(),
                debit: None,
                credit: Some(material),
                currency: base_ccy.clone(),
                fx_rate: Some(Decimal::ONE),
                description: Some(format!("WIP cleared for {}", wo.number)),
                dimensions: None,
            });
        }
        if overhead >= Decimal::new(1, 2) {
            lines.push(CreateJournalLineRequest {
                account_code: posting.manufacturing_overhead.clone(),
                debit: None,
                credit: Some(overhead),
                currency: base_ccy.clone(),
                fx_rate: Some(Decimal::ONE),
                description: Some(format!("Overhead applied to {}", wo.number)),
                dimensions: None,
            });
        }
        complete_journal = Some(post_je(engine, entity_id, &mut tx, today, id, &wo.number, "Finished goods received from production", lines, actor.clone()).await?);
    }

    sqlx::query("UPDATE work_orders SET status = 'completed', total_cost = $1, output_unit_cost = $2, completed_at = $3, complete_journal_id = $4 WHERE id = $5 AND entity_id = $6")
        .bind(total)
        .bind(unit_cost)
        .bind(Utc::now())
        .bind(complete_journal)
        .bind(id)
        .bind(entity_id)
        .execute(&mut *tx)
        .await?;

    tx.commit().await?;
    get_work_order(engine, entity_id, id).await
}

/// Cancel a work order. Only draft orders can be cancelled — once started,
/// components have moved to WIP and must be completed (reversal is a v2 concern).
pub async fn cancel_work_order(engine: &ErpEngine, entity_id: Uuid, id: Uuid) -> ErpResult<()> {
    let wo = load_work_order_row(engine, entity_id, id).await?;
    if wo.status != "draft" {
        return Err(ErpError::ValidationFailed { message: format!("Only draft work orders can be cancelled; {} is {}.", wo.number, wo.status) });
    }
    sqlx::query("UPDATE work_orders SET status = 'cancelled' WHERE id = $1 AND entity_id = $2")
        .bind(id)
        .bind(entity_id)
        .execute(engine.pool())
        .await?;
    Ok(())
}

pub async fn list_work_orders(engine: &ErpEngine, entity_id: Uuid) -> ErpResult<Vec<WorkOrder>> {
    let rows = sqlx::query_as::<_, WorkOrderRow>("SELECT * FROM work_orders WHERE entity_id = $1 ORDER BY created_at DESC")
        .bind(entity_id)
        .fetch_all(engine.pool())
        .await?;
    let mut out = Vec::with_capacity(rows.len());
    for row in rows {
        out.push(hydrate_work_order(engine, entity_id, row, false).await?);
    }
    Ok(out)
}

pub async fn get_work_order(engine: &ErpEngine, entity_id: Uuid, id: Uuid) -> ErpResult<WorkOrder> {
    let row = load_work_order_row(engine, entity_id, id).await?;
    hydrate_work_order(engine, entity_id, row, true).await
}

async fn load_work_order_row(engine: &ErpEngine, entity_id: Uuid, id: Uuid) -> ErpResult<WorkOrderRow> {
    sqlx::query_as::<_, WorkOrderRow>("SELECT * FROM work_orders WHERE id = $1 AND entity_id = $2")
        .bind(id)
        .bind(entity_id)
        .fetch_optional(engine.pool())
        .await?
        .ok_or_else(|| ErpError::NotFound { entity_type: "WorkOrder".to_string(), id })
}

async fn hydrate_work_order(engine: &ErpEngine, entity_id: Uuid, row: WorkOrderRow, with_consumptions: bool) -> ErpResult<WorkOrder> {
    let output_sku: Option<String> = sqlx::query_scalar("SELECT sku FROM inventory_items WHERE id = $1 AND entity_id = $2")
        .bind(row.output_item_id)
        .bind(entity_id)
        .fetch_optional(engine.pool())
        .await?;
    let product_name: Option<String> = sqlx::query_scalar(
        "SELECT p.name FROM products p JOIN boms b ON b.product_id = p.id WHERE b.id = $1 AND b.entity_id = $2",
    )
    .bind(row.bom_id)
    .bind(entity_id)
    .fetch_optional(engine.pool())
    .await?;
    let consumptions = if with_consumptions {
        sqlx::query_as::<_, WorkOrderConsumption>("SELECT * FROM work_order_consumptions WHERE work_order_id = $1 ORDER BY id")
            .bind(row.id)
            .fetch_all(engine.pool())
            .await?
    } else {
        Vec::new()
    };
    Ok(WorkOrder { row, product_name, output_sku, consumptions })
}

// ─── Posting helper ──────────────────────────────────────────────────────────

#[allow(clippy::too_many_arguments)]
async fn post_je(
    engine: &ErpEngine,
    entity_id: Uuid,
    tx: &mut PgTx<'_>,
    date: chrono::NaiveDate,
    wo_id: Uuid,
    number: &str,
    description: &str,
    lines: Vec<CreateJournalLineRequest>,
    actor: AgentOrUserId,
) -> ErpResult<Uuid> {
    let entry_req = CreateJournalEntryRequest {
        date,
        source: JournalSource::InventoryAdjustment,
        source_id: Some(wo_id),
        reference: number.to_string(),
        description: description.to_string(),
        lines,
        post_immediately: true,
    };
    let period = crate::services::periods::period_for_date(engine, entity_id, date).await?;
    let entry = crate::services::journal::create_and_post_in_tx(tx, engine, entity_id, entry_req, period.id, actor).await?;
    Ok(entry.id)
}
