//! Procurement (P2P) business logic: tenders → bids → award → LPO → lodged
//! invoice. Reuses `services::bills::create_bill` for the AP side and the
//! `entity_settings.sequences` numbering pattern.

use chrono::Utc;
use rust_decimal::Decimal;
use uuid::Uuid;

use crate::ap::CreateBillRequest;
use crate::engine::ErpEngine;
use crate::error::{ErpError, ErpResult};
use crate::invoicing::line::CreateInvoiceLineRequest;
use crate::procurement::*;
use crate::types::AgentOrUserId;

// ── Document numbering (RFQ-YYYY-#### / LPO-YYYY-####) ───────────────────────

async fn next_number(engine: &ErpEngine, entity_id: Uuid, key: &str, prefix: &str, date: chrono::NaiveDate) -> ErpResult<String> {
    let seq = sqlx::query_scalar::<_, i64>(&format!(
        r#"UPDATE entity_settings
           SET sequences = jsonb_set(sequences, '{{{key}}}',
               to_jsonb(COALESCE((sequences->>'{key}')::bigint, 1) + 1))
           WHERE entity_id = $1
           RETURNING (sequences->>'{key}')::bigint - 1"#
    ))
    .bind(entity_id)
    .fetch_one(engine.pool())
    .await?;
    let fy = crate::services::periods::fiscal_year_for_date(engine, entity_id, date).await;
    Ok(format!("{prefix}-{fy}-{seq:04}"))
}

// ── Tenders ─────────────────────────────────────────────────────────────────

pub async fn create_tender(
    engine: &ErpEngine,
    entity_id: Uuid,
    req: CreateTenderRequest,
    created_by: Uuid,
) -> ErpResult<TenderRow> {
    let today = Utc::now().date_naive();
    let number = next_number(engine, entity_id, "tender_next", "RFQ", today).await?;
    let id = Uuid::new_v4();
    let row = sqlx::query_as::<_, TenderRow>(
        r#"INSERT INTO tenders (id, entity_id, number, title, description, category, closing_date, status, created_by)
           VALUES ($1,$2,$3,$4,$5,$6,$7,'draft',$8) RETURNING *"#,
    )
    .bind(id).bind(entity_id).bind(&number).bind(&req.title)
    .bind(&req.description).bind(&req.category).bind(req.closing_date).bind(created_by)
    .fetch_one(engine.pool())
    .await?;
    for (i, l) in req.lines.iter().enumerate() {
        sqlx::query("INSERT INTO tender_lines (tender_id, description, quantity, uom, line_no) VALUES ($1,$2,$3,$4,$5)")
            .bind(id).bind(&l.description).bind(l.quantity).bind(&l.uom).bind(i as i32)
            .execute(engine.pool()).await?;
    }
    Ok(row)
}

pub async fn publish_tender(engine: &ErpEngine, entity_id: Uuid, tender_id: Uuid) -> ErpResult<()> {
    let n = sqlx::query(
        "UPDATE tenders SET status='open' WHERE id=$1 AND entity_id=$2 AND status='draft'",
    )
    .bind(tender_id).bind(entity_id)
    .execute(engine.pool()).await?;
    if n.rows_affected() == 0 {
        return Err(ErpError::ValidationFailed { message: "tender not found or not in draft".into() });
    }
    Ok(())
}

// ── Bids (vendor side) ──────────────────────────────────────────────────────

pub async fn submit_bid(
    engine: &ErpEngine,
    entity_id: Uuid,
    tender_id: Uuid,
    vendor_id: Uuid,
    req: SubmitBidRequest,
) -> ErpResult<BidRow> {
    // Tender must be open (and, if a closing date is set, not past it).
    let open: Option<Option<chrono::NaiveDate>> = sqlx::query_scalar(
        "SELECT closing_date FROM tenders WHERE id=$1 AND entity_id=$2 AND status='open'",
    )
    .bind(tender_id).bind(entity_id)
    .fetch_optional(engine.pool()).await?;
    let closing = open.ok_or_else(|| ErpError::ValidationFailed { message: "tender is not open for bids".into() })?;
    if let Some(cd) = closing {
        if Utc::now().date_naive() > cd {
            return Err(ErpError::ValidationFailed { message: "tender has closed".into() });
        }
    }
    let total: Decimal = req.lines.iter().map(|l| l.unit_price * l.quantity).sum();
    let currency = req.currency.clone().unwrap_or_else(|| "KES".into());
    let id = Uuid::new_v4();
    // One bid per vendor per tender: replace an existing (unless already awarded).
    sqlx::query(
        "DELETE FROM bids WHERE tender_id=$1 AND vendor_id=$2 AND status NOT IN ('awarded')",
    )
    .bind(tender_id).bind(vendor_id).execute(engine.pool()).await?;
    let row = sqlx::query_as::<_, BidRow>(
        r#"INSERT INTO bids (id, entity_id, tender_id, vendor_id, currency, total_amount, notes, status)
           VALUES ($1,$2,$3,$4,$5,$6,$7,'submitted') RETURNING *"#,
    )
    .bind(id).bind(entity_id).bind(tender_id).bind(vendor_id).bind(&currency).bind(total).bind(&req.notes)
    .fetch_one(engine.pool()).await
    .map_err(|_| ErpError::ValidationFailed { message: "you have already bid on this tender".into() })?;
    for (i, l) in req.lines.iter().enumerate() {
        sqlx::query(
            "INSERT INTO bid_lines (bid_id, tender_line_id, description, quantity, unit_price, amount, line_no)
             VALUES ($1,$2,$3,$4,$5,$6,$7)",
        )
        .bind(id).bind(l.tender_line_id).bind(&l.description).bind(l.quantity)
        .bind(l.unit_price).bind(l.unit_price * l.quantity).bind(i as i32)
        .execute(engine.pool()).await?;
    }
    Ok(row)
}

// ── Award → build the LPO ───────────────────────────────────────────────────

pub async fn award_tender(
    engine: &ErpEngine,
    entity_id: Uuid,
    tender_id: Uuid,
    req: AwardTenderRequest,
    created_by: Uuid,
) -> ErpResult<PurchaseOrderRow> {
    let bid = sqlx::query_as::<_, BidRow>(
        "SELECT * FROM bids WHERE id=$1 AND tender_id=$2 AND entity_id=$3",
    )
    .bind(req.bid_id).bind(tender_id).bind(entity_id)
    .fetch_optional(engine.pool()).await?
    .ok_or_else(|| ErpError::ValidationFailed { message: "bid not found for this tender".into() })?;

    let today = Utc::now().date_naive();
    let number = next_number(engine, entity_id, "lpo_next", "LPO", today).await?;
    let po_id = Uuid::new_v4();

    // Build the LPO from the winning bid's lines.
    let bid_lines = sqlx::query_as::<_, BidLineRow>("SELECT * FROM bid_lines WHERE bid_id=$1 ORDER BY line_no")
        .bind(req.bid_id).fetch_all(engine.pool()).await?;
    let subtotal: Decimal = bid_lines.iter().map(|l| l.amount).sum();

    let po = sqlx::query_as::<_, PurchaseOrderRow>(
        r#"INSERT INTO purchase_orders
           (id, entity_id, number, vendor_id, tender_id, bid_id, currency, fx_rate, subtotal, tax_total, gross_total,
            status, issue_date, delivery_date, notes, created_by)
           VALUES ($1,$2,$3,$4,$5,$6,$7,1,$8,0,$8,'issued',$9,$10,$11,$12) RETURNING *"#,
    )
    .bind(po_id).bind(entity_id).bind(&number).bind(bid.vendor_id).bind(tender_id).bind(req.bid_id)
    .bind(&bid.currency).bind(subtotal).bind(today).bind(req.delivery_date).bind(&req.notes).bind(created_by)
    .fetch_one(engine.pool()).await?;
    for (i, l) in bid_lines.iter().enumerate() {
        sqlx::query(
            "INSERT INTO purchase_order_lines (po_id, description, quantity, uom, unit_price, line_total, line_no)
             VALUES ($1,$2,$3,'unit',$4,$5,$6)",
        )
        .bind(po_id).bind(&l.description).bind(l.quantity).bind(l.unit_price).bind(l.amount).bind(i as i32)
        .execute(engine.pool()).await?;
    }

    // Award the winner, reject the rest, close the tender.
    sqlx::query("UPDATE bids SET status='awarded' WHERE id=$1").bind(req.bid_id).execute(engine.pool()).await?;
    sqlx::query("UPDATE bids SET status='rejected' WHERE tender_id=$1 AND id<>$2 AND status NOT IN ('withdrawn')")
        .bind(tender_id).bind(req.bid_id).execute(engine.pool()).await?;
    sqlx::query("UPDATE tenders SET status='awarded' WHERE id=$1").bind(tender_id).execute(engine.pool()).await?;
    Ok(po)
}

// ── Vendor lodges an invoice against an LPO → AP bill (pending approval) ─────

pub async fn lodge_invoice(
    engine: &ErpEngine,
    entity_id: Uuid,
    vendor_id: Uuid,
    po_id: Uuid,
    req: LodgeInvoiceRequest,
) -> ErpResult<crate::ap::Bill> {
    let po = sqlx::query_as::<_, PurchaseOrderRow>(
        "SELECT * FROM purchase_orders WHERE id=$1 AND entity_id=$2 AND vendor_id=$3",
    )
    .bind(po_id).bind(entity_id).bind(vendor_id)
    .fetch_optional(engine.pool()).await?
    .ok_or_else(|| ErpError::ValidationFailed { message: "purchase order not found".into() })?;
    if po.status == "invoiced" || po.status == "closed" || po.status == "cancelled" {
        return Err(ErpError::ValidationFailed { message: "this purchase order is already invoiced or closed".into() });
    }

    // Bill lines: caller overrides, else the LPO lines as-is.
    let lines: Vec<CreateInvoiceLineRequest> = if req.lines.is_empty() {
        let po_lines = sqlx::query_as::<_, PurchaseOrderLineRow>("SELECT * FROM purchase_order_lines WHERE po_id=$1 ORDER BY line_no")
            .bind(po_id).fetch_all(engine.pool()).await?;
        po_lines.into_iter().map(|l| CreateInvoiceLineRequest {
            product_id: None,
            description: Some(l.description),
            quantity: l.quantity,
            unit_price: Some(l.unit_price),
            discount_percent: None,
            account_code: l.account_code,
            vat_treatment: None,
            dimensions: None,
        }).collect()
    } else {
        req.lines.into_iter().map(|l| CreateInvoiceLineRequest {
            product_id: None,
            description: Some(l.description),
            quantity: l.quantity,
            unit_price: Some(l.unit_price),
            discount_percent: None,
            account_code: l.account_code,
            vat_treatment: None,
            dimensions: None,
        }).collect()
    };

    let bill = crate::services::bills::create_bill(
        engine,
        entity_id,
        CreateBillRequest {
            vendor_id,
            vendor_invoice_number: req.vendor_invoice_number,
            issue_date: req.issue_date,
            due_date: None,
            currency: None,
            fx_rate: None,
            lines,
            notes: req.notes.or_else(|| Some(format!("Lodged against {}", po.number))),
        },
        &AgentOrUserId::Agent(format!("vendor:{vendor_id}")),
    )
    .await?;

    // Link the bill to its LPO and move both forward for approval.
    sqlx::query("UPDATE bills SET po_id=$1, status='pending_approval' WHERE id=$2")
        .bind(po_id).bind(bill.id).execute(engine.pool()).await?;
    sqlx::query("UPDATE purchase_orders SET status='invoiced' WHERE id=$1")
        .bind(po_id).execute(engine.pool()).await?;
    Ok(bill)
}
