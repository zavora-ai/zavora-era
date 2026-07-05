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

pub(crate) async fn next_number(engine: &ErpEngine, entity_id: Uuid, key: &str, prefix: &str, date: chrono::NaiveDate) -> ErpResult<String> {
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
    let _ = crate::services::audit::record_event(engine, entity_id, "Awarded", "tender", tender_id,
        &AgentOrUserId::User(created_by), Some(serde_json::json!({ "lpo": po.number, "bid_id": req.bid_id }))).await;
    Ok(po)
}

// ── Direct procurement (raise an LPO without a tender) ──────────────────────

/// Raise a purchase order directly against a vendor master — no tender/bid.
/// Used for single-source or spot purchases and for vendors that are not on the
/// portal. Mirrors the award path (net LPO, VAT applied later at billing);
/// `tender_id`/`bid_id` stay NULL to mark it as direct.
pub async fn create_purchase_order(
    engine: &ErpEngine,
    entity_id: Uuid,
    req: CreatePurchaseOrderRequest,
    created_by: Uuid,
) -> ErpResult<PurchaseOrderRow> {
    if req.lines.is_empty() {
        return Err(ErpError::ValidationFailed { message: "a purchase order needs at least one line".into() });
    }

    // Vendor must exist under this entity (any master — portal login not required).
    let vendor = sqlx::query_as::<_, crate::parties::VendorRow>(
        "SELECT * FROM vendors WHERE id=$1 AND entity_id=$2",
    )
    .bind(req.vendor_id).bind(entity_id)
    .fetch_optional(engine.pool()).await?
    .ok_or_else(|| ErpError::ValidationFailed { message: "vendor not found".into() })?;

    let currency = req.currency.filter(|c| !c.trim().is_empty()).unwrap_or(vendor.currency);

    let today = Utc::now().date_naive();
    let number = next_number(engine, entity_id, "lpo_next", "LPO", today).await?;
    let po_id = Uuid::new_v4();

    // Net LPO: VAT is applied downstream when the bill is raised, matching the
    // award-from-tender path.
    let line_totals: Vec<Decimal> = req.lines.iter().map(|l| (l.quantity * l.unit_price).round_dp(2)).collect();
    let subtotal: Decimal = line_totals.iter().copied().sum();

    let po = sqlx::query_as::<_, PurchaseOrderRow>(
        r#"INSERT INTO purchase_orders
           (id, entity_id, number, vendor_id, tender_id, bid_id, currency, fx_rate, subtotal, tax_total, gross_total,
            status, issue_date, delivery_date, notes, created_by)
           VALUES ($1,$2,$3,$4,NULL,NULL,$5,1,$6,0,$6,'issued',$7,$8,$9,$10) RETURNING *"#,
    )
    .bind(po_id).bind(entity_id).bind(&number).bind(req.vendor_id)
    .bind(&currency).bind(subtotal).bind(today).bind(req.delivery_date).bind(&req.notes).bind(created_by)
    .fetch_one(engine.pool()).await?;

    for (i, (l, total)) in req.lines.iter().zip(line_totals.iter()).enumerate() {
        sqlx::query(
            "INSERT INTO purchase_order_lines (po_id, description, quantity, uom, unit_price, tax_treatment, account_code, line_total, line_no)
             VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9)",
        )
        .bind(po_id).bind(&l.description).bind(l.quantity).bind(&l.uom).bind(l.unit_price)
        .bind(&l.tax_treatment).bind(&l.account_code).bind(*total).bind(i as i32)
        .execute(engine.pool()).await?;
    }

    let _ = crate::services::audit::record_event(engine, entity_id, "Created", "purchase_order", po.id,
        &AgentOrUserId::User(created_by), Some(serde_json::json!({ "number": po.number, "vendor_id": req.vendor_id, "gross": po.gross_total, "direct": true }))).await;
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
    let _ = crate::services::audit::record_event(engine, entity_id, "Lodged", "purchase_order", po_id,
        &AgentOrUserId::Agent(format!("vendor:{vendor_id}")), Some(serde_json::json!({ "bill": bill.number }))).await;
    Ok(bill)
}

// ── Purchase-order document (legal LPO: preview + PDF) ───────────────────────

/// Format an address JSONB blob (line1/line2/city/country) into one line, like
/// the invoice builder does for customers.
fn address_line(v: Option<&serde_json::Value>) -> Option<String> {
    let a = v?;
    let parts: Vec<String> = ["line1", "line2", "city", "country"]
        .iter()
        .filter_map(|k| a.get(*k).and_then(|x| x.as_str()).filter(|s| !s.is_empty()).map(|s| s.to_string()))
        .collect();
    if parts.is_empty() { None } else { Some(parts.join(", ")) }
}

/// Build the shared LPO document model from the DB (buyer branding + supplier
/// master + priced lines). Same shape used for the on-screen preview and PDF.
pub async fn build_po_document(
    engine: &ErpEngine,
    entity_id: Uuid,
    po_id: Uuid,
) -> ErpResult<crate::procurement::document::PurchaseOrderDocument> {
    use crate::procurement::document::{PurchaseOrderDocLine, PurchaseOrderDocument};

    let po = sqlx::query_as::<_, PurchaseOrderRow>(
        "SELECT * FROM purchase_orders WHERE id=$1 AND entity_id=$2",
    )
    .bind(po_id).bind(entity_id)
    .fetch_optional(engine.pool()).await?
    .ok_or_else(|| ErpError::NotFound { entity_type: "purchase order".into(), id: po_id })?;

    let lines = sqlx::query_as::<_, PurchaseOrderLineRow>(
        "SELECT * FROM purchase_order_lines WHERE po_id=$1 ORDER BY line_no",
    )
    .bind(po_id).fetch_all(engine.pool()).await?;

    let vendor = sqlx::query_as::<_, crate::parties::VendorRow>(
        "SELECT * FROM vendors WHERE id=$1 AND entity_id=$2",
    )
    .bind(po.vendor_id).bind(entity_id)
    .fetch_optional(engine.pool()).await?;

    // Buyer branding — same source as the invoice document.
    let (org_name, kra_pin, branding_json): (Option<String>, Option<String>, Option<serde_json::Value>) =
        sqlx::query_as("SELECT organization_name, kra_pin, branding FROM entity_settings WHERE entity_id=$1")
            .bind(entity_id)
            .fetch_optional(engine.pool()).await?
            .unwrap_or((None, None, None));
    let branding = branding_json.unwrap_or_default();
    let bget = |k: &str| branding.get(k).and_then(|v| v.as_str()).map(|s| s.to_string()).filter(|s| !s.is_empty());
    let org_name = bget("company_name").or(org_name).unwrap_or_else(|| "Your Company".to_string());
    let org_address = bget("address");

    let fmt_date = |d: chrono::NaiveDate| d.format("%d %b %Y").to_string();

    Ok(PurchaseOrderDocument {
        org_name,
        org_kra_pin: kra_pin,
        org_vat_number: bget("vat_number"),
        org_address: org_address.clone(),
        org_email: bget("email"),
        org_phone: bget("phone"),
        logo_url: bget("logo_url"),
        primary_color: bget("primary_color").unwrap_or_else(|| "#1a56db".to_string()),
        footer_text: bget("footer_text"),
        number: po.number.clone(),
        issue_date: fmt_date(po.issue_date),
        delivery_date: po.delivery_date.map(fmt_date).unwrap_or_else(|| "—".to_string()),
        currency: po.currency.clone(),
        supplier_name: vendor.as_ref().map(|v| v.name.clone()).unwrap_or_else(|| po.vendor_id.to_string()),
        supplier_address: vendor.as_ref().and_then(|v| address_line(v.address.as_ref())),
        supplier_kra_pin: vendor.as_ref().and_then(|v| v.kra_pin.clone()),
        deliver_to: org_address,
        lines: lines.iter().map(|l| PurchaseOrderDocLine {
            description: l.description.clone(),
            quantity: l.quantity,
            uom: l.uom.clone(),
            unit_price: l.unit_price,
            line_total: l.line_total,
        }).collect(),
        subtotal: po.subtotal,
        tax_total: po.tax_total,
        gross_total: po.gross_total,
        status: po.status.clone(),
        notes: po.notes.clone(),
    })
}

/// Render the LPO as HTML (source of truth for the on-screen preview).
pub async fn po_document_html(engine: &ErpEngine, entity_id: Uuid, po_id: Uuid) -> ErpResult<String> {
    let doc = build_po_document(engine, entity_id, po_id).await?;
    Ok(crate::procurement::document::render_po_html(&doc))
}

/// Render the LPO as PDF bytes. Uses the same HTML → headless-Chrome path as the
/// invoice document, falling back to the built-in hand-drawn PDF when Chrome is
/// absent, so the download always works. Returns `(bytes, lpo_number)`.
pub async fn po_document_pdf(engine: &ErpEngine, entity_id: Uuid, po_id: Uuid) -> ErpResult<(Vec<u8>, String)> {
    let doc = build_po_document(engine, entity_id, po_id).await?;
    let accent = doc.primary_color.clone();
    let number = doc.number.clone();
    let html = crate::procurement::document::render_po_html(&doc);

    // Chrome conversion can block; run it on a blocking thread.
    let html_clone = html.clone();
    let pdf = tokio::task::spawn_blocking(move || crate::invoicing::htmlpdf::html_to_pdf(&html_clone))
        .await
        .ok()
        .flatten();

    let bytes = match pdf {
        Some(b) => b,
        None => {
            tracing::warn!("Chrome unavailable; using fallback PDF renderer for LPO {po_id}");
            let fb = crate::invoicing::pdf::InvoicePdfData {
                org_name: doc.org_name.clone(),
                invoice_number: doc.number.clone(),
                invoice_type_label: "PURCHASE ORDER".to_string(),
                issue_date: doc.issue_date.clone(),
                due_date: doc.delivery_date.clone(),
                currency: doc.currency.clone(),
                customer_name: doc.supplier_name.clone(),
                customer_email: None,
                lines: doc.lines.iter().map(|l| crate::invoicing::pdf::InvoicePdfLine {
                    description: l.description.clone(),
                    quantity: l.quantity,
                    unit_price: l.unit_price,
                    line_total: l.line_total,
                }).collect(),
                subtotal: doc.subtotal,
                discount_total: Decimal::ZERO,
                tax_total: doc.tax_total,
                gross_total: doc.gross_total,
                amount_paid: Decimal::ZERO,
                balance_due: doc.gross_total,
                notes: doc.notes.clone(),
                footer_text: doc.footer_text.clone(),
                accent_rgb: crate::invoicing::pdf::parse_hex_color(&accent),
            };
            crate::invoicing::pdf::render_invoice_pdf(&fb)
        }
    };
    Ok((bytes, number))
}

// ── Purchase requisitions (self-service → approval → convert) ────────────────

/// Raise a requisition (starts in `draft`). Any staff member can create one for
/// their department; approval is a separate step by an approver role.
pub async fn create_requisition(
    engine: &ErpEngine,
    entity_id: Uuid,
    req: CreateRequisitionRequest,
    requested_by: Uuid,
) -> ErpResult<PurchaseRequisitionRow> {
    if req.lines.is_empty() {
        return Err(ErpError::ValidationFailed { message: "a requisition needs at least one line".into() });
    }
    let today = Utc::now().date_naive();
    let number = next_number(engine, entity_id, "requisition_next", "PR", today).await?;
    let pr_id = Uuid::new_v4();
    let currency = req.currency.filter(|c| !c.trim().is_empty()).unwrap_or_else(|| "KES".to_string());

    let line_totals: Vec<Decimal> = req.lines.iter().map(|l| (l.quantity * l.estimated_unit_price).round_dp(2)).collect();
    let estimated_total: Decimal = line_totals.iter().copied().sum();

    let row = sqlx::query_as::<_, PurchaseRequisitionRow>(
        r#"INSERT INTO purchase_requisitions
           (id, entity_id, number, title, justification, department, cost_center, currency, needed_by,
            estimated_total, status, requested_by, notes)
           VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,'draft',$11,$12) RETURNING *"#,
    )
    .bind(pr_id).bind(entity_id).bind(&number).bind(&req.title).bind(&req.justification)
    .bind(&req.department).bind(&req.cost_center).bind(&currency).bind(req.needed_by)
    .bind(estimated_total).bind(requested_by).bind(&req.notes)
    .fetch_one(engine.pool()).await?;

    for (i, (l, total)) in req.lines.iter().zip(line_totals.iter()).enumerate() {
        sqlx::query(
            "INSERT INTO purchase_requisition_lines (pr_id, description, quantity, uom, estimated_unit_price, account_code, line_total, line_no)
             VALUES ($1,$2,$3,$4,$5,$6,$7,$8)",
        )
        .bind(pr_id).bind(&l.description).bind(l.quantity).bind(&l.uom).bind(l.estimated_unit_price)
        .bind(&l.account_code).bind(*total).bind(i as i32)
        .execute(engine.pool()).await?;
    }
    let _ = crate::services::audit::record_event(engine, entity_id, "Created", "requisition", pr_id,
        &AgentOrUserId::User(requested_by), Some(serde_json::json!({ "number": row.number, "estimated_total": estimated_total }))).await;
    Ok(row)
}

/// Submit a draft requisition for approval.
pub async fn submit_requisition(engine: &ErpEngine, entity_id: Uuid, pr_id: Uuid) -> ErpResult<PurchaseRequisitionRow> {
    let row = sqlx::query_as::<_, PurchaseRequisitionRow>(
        "UPDATE purchase_requisitions SET status='submitted' WHERE id=$1 AND entity_id=$2 AND status='draft' RETURNING *",
    )
    .bind(pr_id).bind(entity_id)
    .fetch_optional(engine.pool()).await?
    .ok_or_else(|| ErpError::ValidationFailed { message: "requisition not found or not in draft".into() })?;
    Ok(row)
}

/// Approve a submitted requisition (approver role enforced at the route).
pub async fn approve_requisition(engine: &ErpEngine, entity_id: Uuid, pr_id: Uuid, approver: Uuid) -> ErpResult<PurchaseRequisitionRow> {
    // Delegation of Authority: check the approver's limit before committing.
    let pending = sqlx::query_as::<_, PurchaseRequisitionRow>(
        "SELECT * FROM purchase_requisitions WHERE id=$1 AND entity_id=$2 AND status='submitted'",
    )
    .bind(pr_id).bind(entity_id)
    .fetch_optional(engine.pool()).await?
    .ok_or_else(|| ErpError::ValidationFailed { message: "requisition not found or not awaiting approval".into() })?;
    crate::services::approval::assert_within_limit(engine, entity_id, approver, pending.estimated_total, "requisition").await?;

    let row = sqlx::query_as::<_, PurchaseRequisitionRow>(
        "UPDATE purchase_requisitions SET status='approved', approved_by=$3, approved_at=now() \
         WHERE id=$1 AND entity_id=$2 AND status='submitted' RETURNING *",
    )
    .bind(pr_id).bind(entity_id).bind(approver)
    .fetch_optional(engine.pool()).await?
    .ok_or_else(|| ErpError::ValidationFailed { message: "requisition not found or not awaiting approval".into() })?;
    let _ = crate::services::audit::record_event(engine, entity_id, "Approved", "requisition", pr_id,
        &AgentOrUserId::User(approver), Some(serde_json::json!({ "number": row.number }))).await;
    Ok(row)
}

/// Reject a submitted requisition with a reason.
pub async fn reject_requisition(engine: &ErpEngine, entity_id: Uuid, pr_id: Uuid, approver: Uuid, reason: Option<String>) -> ErpResult<PurchaseRequisitionRow> {
    let row = sqlx::query_as::<_, PurchaseRequisitionRow>(
        "UPDATE purchase_requisitions SET status='rejected', approved_by=$3, approved_at=now(), rejection_reason=$4 \
         WHERE id=$1 AND entity_id=$2 AND status='submitted' RETURNING *",
    )
    .bind(pr_id).bind(entity_id).bind(approver).bind(&reason)
    .fetch_optional(engine.pool()).await?
    .ok_or_else(|| ErpError::ValidationFailed { message: "requisition not found or not awaiting approval".into() })?;
    Ok(row)
}

/// Convert an **approved** requisition into a tender or a direct PO. Links the
/// new sourcing doc back to the requisition and marks the requisition converted.
pub async fn convert_requisition(
    engine: &ErpEngine,
    entity_id: Uuid,
    pr_id: Uuid,
    req: ConvertRequisitionRequest,
    created_by: Uuid,
) -> ErpResult<serde_json::Value> {
    let pr = sqlx::query_as::<_, PurchaseRequisitionRow>(
        "SELECT * FROM purchase_requisitions WHERE id=$1 AND entity_id=$2",
    )
    .bind(pr_id).bind(entity_id)
    .fetch_optional(engine.pool()).await?
    .ok_or_else(|| ErpError::ValidationFailed { message: "requisition not found".into() })?;
    if pr.status != "approved" {
        return Err(ErpError::ValidationFailed { message: "only an approved requisition can be converted".into() });
    }

    let lines = sqlx::query_as::<_, PurchaseRequisitionLineRow>(
        "SELECT * FROM purchase_requisition_lines WHERE pr_id=$1 ORDER BY line_no",
    )
    .bind(pr_id).fetch_all(engine.pool()).await?;

    match req.target.as_str() {
        "tender" => {
            let tender = create_tender(engine, entity_id, CreateTenderRequest {
                title: pr.title.clone(),
                description: pr.justification.clone(),
                category: pr.department.clone(),
                closing_date: req.closing_date,
                lines: lines.iter().map(|l| CreateTenderLineRequest {
                    description: l.description.clone(), quantity: l.quantity, uom: l.uom.clone(),
                }).collect(),
            }, created_by).await?;
            sqlx::query("UPDATE tenders SET requisition_id=$1 WHERE id=$2").bind(pr_id).bind(tender.id).execute(engine.pool()).await?;
            sqlx::query("UPDATE purchase_requisitions SET status='converted', converted_to_type='tender', converted_to_id=$1 WHERE id=$2")
                .bind(tender.id).bind(pr_id).execute(engine.pool()).await?;
            Ok(serde_json::json!({ "target": "tender", "tender": tender }))
        }
        "purchase_order" => {
            let vendor_id = req.vendor_id.ok_or_else(|| ErpError::ValidationFailed { message: "a vendor is required to raise a direct PO".into() })?;
            let po = create_purchase_order(engine, entity_id, CreatePurchaseOrderRequest {
                vendor_id,
                currency: Some(pr.currency.clone()),
                delivery_date: req.delivery_date,
                notes: Some(format!("From requisition {}", pr.number)),
                lines: lines.iter().map(|l| CreatePurchaseOrderLineRequest {
                    description: l.description.clone(), quantity: l.quantity, uom: l.uom.clone(),
                    unit_price: l.estimated_unit_price, account_code: l.account_code.clone(), tax_treatment: None,
                }).collect(),
            }, created_by).await?;
            sqlx::query("UPDATE purchase_orders SET requisition_id=$1 WHERE id=$2").bind(pr_id).bind(po.id).execute(engine.pool()).await?;
            sqlx::query("UPDATE purchase_requisitions SET status='converted', converted_to_type='purchase_order', converted_to_id=$1 WHERE id=$2")
                .bind(po.id).bind(pr_id).execute(engine.pool()).await?;
            Ok(serde_json::json!({ "target": "purchase_order", "purchase_order": po }))
        }
        other => Err(ErpError::ValidationFailed { message: format!("unknown conversion target '{other}'") }),
    }
}

// ── Goods receipts (GRN) + 3-way match ──────────────────────────────────────

/// Record a goods receipt against a PO. Multiple (partial) GRNs per PO are fine.
pub async fn create_goods_receipt(
    engine: &ErpEngine,
    entity_id: Uuid,
    po_id: Uuid,
    req: CreateGrnRequest,
    received_by: Uuid,
) -> ErpResult<GoodsReceiptRow> {
    // PO must exist under this entity.
    let po_exists = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM purchase_orders WHERE id=$1 AND entity_id=$2",
    )
    .bind(po_id).bind(entity_id).fetch_one(engine.pool()).await?;
    if po_exists == 0 {
        return Err(ErpError::ValidationFailed { message: "purchase order not found".into() });
    }
    if req.lines.iter().all(|l| l.quantity_received <= Decimal::ZERO) {
        return Err(ErpError::ValidationFailed { message: "receive a quantity on at least one line".into() });
    }

    let date = req.receipt_date.unwrap_or_else(|| Utc::now().date_naive());
    let number = next_number(engine, entity_id, "grn_next", "GRN", date).await?;
    let grn_id = Uuid::new_v4();

    let grn = sqlx::query_as::<_, GoodsReceiptRow>(
        r#"INSERT INTO goods_receipts (id, entity_id, number, po_id, receipt_date, received_by, notes)
           VALUES ($1,$2,$3,$4,$5,$6,$7) RETURNING *"#,
    )
    .bind(grn_id).bind(entity_id).bind(&number).bind(po_id).bind(date).bind(received_by).bind(&req.notes)
    .fetch_one(engine.pool()).await?;

    for (i, l) in req.lines.iter().enumerate() {
        if l.quantity_received <= Decimal::ZERO { continue; }
        sqlx::query(
            "INSERT INTO goods_receipt_lines (grn_id, po_line_id, description, quantity_received, line_no)
             VALUES ($1,$2,$3,$4,$5)",
        )
        .bind(grn_id).bind(l.po_line_id).bind(&l.description).bind(l.quantity_received).bind(i as i32)
        .execute(engine.pool()).await?;
    }
    let _ = crate::services::audit::record_event(engine, entity_id, "Received", "purchase_order", po_id,
        &AgentOrUserId::User(received_by), Some(serde_json::json!({ "grn": grn.number }))).await;
    Ok(grn)
}

/// List GRNs recorded against a PO (headers only).
pub async fn list_goods_receipts(engine: &ErpEngine, entity_id: Uuid, po_id: Uuid) -> ErpResult<Vec<GoodsReceiptRow>> {
    let rows = sqlx::query_as::<_, GoodsReceiptRow>(
        "SELECT * FROM goods_receipts WHERE entity_id=$1 AND po_id=$2 ORDER BY receipt_date, created_at",
    )
    .bind(entity_id).bind(po_id).fetch_all(engine.pool()).await?;
    Ok(rows)
}

/// Compute the 3-way match for a PO: ordered (PO) vs received (all GRNs) vs
/// billed (all linked bills), grouped by line description. Price tolerance 2%.
pub async fn three_way_match(engine: &ErpEngine, entity_id: Uuid, po_id: Uuid) -> ErpResult<ThreeWayMatch> {
    use std::collections::BTreeMap;
    let key = |s: &str| s.trim().to_lowercase();

    let po_lines = sqlx::query_as::<_, PurchaseOrderLineRow>(
        "SELECT * FROM purchase_order_lines WHERE po_id=$1 ORDER BY line_no",
    )
    .bind(po_id).fetch_all(engine.pool()).await?;

    // Received qty by description (across every GRN for this PO).
    let received: Vec<(String, Decimal)> = sqlx::query_as(
        r#"SELECT grl.description, COALESCE(SUM(grl.quantity_received),0)
           FROM goods_receipt_lines grl
           JOIN goods_receipts g ON g.id = grl.grn_id
           WHERE g.po_id = $1 AND g.entity_id = $2
           GROUP BY grl.description"#,
    )
    .bind(po_id).bind(entity_id).fetch_all(engine.pool()).await.unwrap_or_default();
    let mut received_map: BTreeMap<String, Decimal> = BTreeMap::new();
    for (d, q) in received { *received_map.entry(key(&d)).or_default() += q; }

    // Billed qty and unit price by description (across every bill linked to this PO).
    let billed: Vec<(String, Decimal, Decimal)> = sqlx::query_as(
        r#"SELECT bl.description, COALESCE(SUM(bl.quantity),0), COALESCE(MAX(bl.unit_price),0)
           FROM bill_lines bl
           JOIN bills b ON b.id = bl.bill_id
           WHERE b.po_id = $1 AND b.entity_id = $2
           GROUP BY bl.description"#,
    )
    .bind(po_id).bind(entity_id).fetch_all(engine.pool()).await.unwrap_or_default();
    let mut billed_qty: BTreeMap<String, Decimal> = BTreeMap::new();
    let mut billed_price: BTreeMap<String, Decimal> = BTreeMap::new();
    for (d, q, p) in billed { let k = key(&d); *billed_qty.entry(k.clone()).or_default() += q; billed_price.insert(k, p); }

    let tolerance = Decimal::new(2, 2); // 2%
    let mut lines = Vec::new();
    let mut exceptions = Vec::new();
    for l in &po_lines {
        let k = key(&l.description);
        let received_qty = received_map.get(&k).copied().unwrap_or(Decimal::ZERO);
        let billed = billed_qty.get(&k).copied().unwrap_or(Decimal::ZERO);
        let b_price = billed_price.get(&k).copied().unwrap_or(Decimal::ZERO);

        let (status, note) = if billed > received_qty {
            let msg = format!("{}: billed {} but only {} received", l.description, billed.normalize(), received_qty.normalize());
            exceptions.push(msg.clone());
            ("over_billed".to_string(), Some(msg))
        } else if billed > Decimal::ZERO && l.unit_price > Decimal::ZERO
            && ((b_price - l.unit_price).abs() / l.unit_price) > tolerance {
            let msg = format!("{}: billed price {} differs from PO price {}", l.description, b_price.normalize(), l.unit_price.normalize());
            exceptions.push(msg.clone());
            ("price_variance".to_string(), Some(msg))
        } else {
            ("matched".to_string(), None)
        };

        lines.push(ThreeWayMatchLine {
            description: l.description.clone(),
            ordered_qty: l.quantity,
            received_qty,
            billed_qty: billed,
            po_unit_price: l.unit_price,
            billed_unit_price: b_price,
            status,
            note,
        });
    }

    // A bill can be billed against a PO for a line not on the PO — flag those too.
    Ok(ThreeWayMatch { po_id, matched: exceptions.is_empty(), lines, exceptions })
}

/// Approval gate: a bill linked to a PO cannot be approved while it bills for
/// more than has been received (the 3-way-match "over-billed" exception). Bills
/// not linked to a PO, or fully received, pass through untouched.
pub async fn assert_bill_receivable(engine: &ErpEngine, entity_id: Uuid, po_id: Uuid) -> ErpResult<()> {
    let m = three_way_match(engine, entity_id, po_id).await?;
    let blockers: Vec<String> = m.lines.iter().filter(|l| l.status == "over_billed").filter_map(|l| l.note.clone()).collect();
    if !blockers.is_empty() {
        return Err(ErpError::ValidationFailed {
            message: format!("3-way match failed — record a goods receipt first. {}", blockers.join("; ")),
        });
    }
    Ok(())
}

// ── Procurement analytics (reports pack) ────────────────────────────────────

/// A procurement dashboard: spend by vendor (ordered vs billed), open
/// commitments (PO value not yet invoiced), 3-way-match exceptions, and
/// document counts by status. Returned as JSON for a flexible report UI.
pub async fn procurement_analytics(engine: &ErpEngine, entity_id: Uuid) -> ErpResult<serde_json::Value> {
    let pool = engine.pool();

    // Spend by vendor: ordered (POs) and billed (bills), merged by vendor name.
    let ordered: Vec<(String, Decimal)> = sqlx::query_as(
        r#"SELECT v.name, COALESCE(SUM(po.gross_total),0)
           FROM purchase_orders po JOIN vendors v ON v.id = po.vendor_id
           WHERE po.entity_id = $1 AND po.status <> 'cancelled'
           GROUP BY v.name"#,
    ).bind(entity_id).fetch_all(pool).await.unwrap_or_default();
    let billed: Vec<(String, Decimal)> = sqlx::query_as(
        r#"SELECT v.name, COALESCE(SUM(b.gross_total),0)
           FROM bills b JOIN vendors v ON v.id = b.vendor_id
           WHERE b.entity_id = $1
           GROUP BY v.name"#,
    ).bind(entity_id).fetch_all(pool).await.unwrap_or_default();

    use std::collections::BTreeMap;
    let mut by_vendor: BTreeMap<String, (Decimal, Decimal)> = BTreeMap::new();
    for (n, v) in ordered { by_vendor.entry(n).or_default().0 += v; }
    for (n, v) in billed { by_vendor.entry(n).or_default().1 += v; }
    let mut spend: Vec<serde_json::Value> = by_vendor.into_iter()
        .map(|(name, (o, b))| serde_json::json!({ "vendor": name, "ordered": o, "billed": b }))
        .collect();
    spend.sort_by(|a, b| {
        let ao = a["ordered"].as_str().and_then(|s| s.parse::<f64>().ok()).unwrap_or(0.0);
        let bo = b["ordered"].as_str().and_then(|s| s.parse::<f64>().ok()).unwrap_or(0.0);
        bo.partial_cmp(&ao).unwrap_or(std::cmp::Ordering::Equal)
    });

    // Open commitments — PO value issued but not yet fully invoiced.
    let open_commitments: Vec<OpenCommitmentRow> = sqlx::query_as(
        r#"SELECT po.number, v.name AS vendor, po.currency, po.gross_total, po.issue_date, po.status
           FROM purchase_orders po JOIN vendors v ON v.id = po.vendor_id
           WHERE po.entity_id = $1 AND po.status IN ('issued','acknowledged','partially_invoiced')
           ORDER BY po.issue_date"#,
    ).bind(entity_id).fetch_all(pool).await.unwrap_or_default();
    let committed_total: Decimal = open_commitments.iter().map(|c| c.gross_total).sum();

    // Counts by status for each document type.
    let counts = |rows: Vec<(String, i64)>| -> serde_json::Value {
        serde_json::Value::Object(rows.into_iter().map(|(s, c)| (s, serde_json::json!(c))).collect())
    };
    let req_counts: Vec<(String, i64)> = sqlx::query_as("SELECT status, COUNT(*) FROM purchase_requisitions WHERE entity_id=$1 GROUP BY status").bind(entity_id).fetch_all(pool).await.unwrap_or_default();
    let tender_counts: Vec<(String, i64)> = sqlx::query_as("SELECT status, COUNT(*) FROM tenders WHERE entity_id=$1 GROUP BY status").bind(entity_id).fetch_all(pool).await.unwrap_or_default();
    let po_counts: Vec<(String, i64)> = sqlx::query_as("SELECT status, COUNT(*) FROM purchase_orders WHERE entity_id=$1 GROUP BY status").bind(entity_id).fetch_all(pool).await.unwrap_or_default();

    Ok(serde_json::json!({
        "spend_by_vendor": spend,
        "open_commitments": open_commitments,
        "committed_total": committed_total,
        "counts": {
            "requisitions": counts(req_counts),
            "tenders": counts(tender_counts),
            "purchase_orders": counts(po_counts),
        },
    }))
}

#[derive(Debug, Clone, serde::Serialize, sqlx::FromRow)]
pub struct OpenCommitmentRow {
    pub number: String,
    pub vendor: String,
    pub currency: String,
    pub gross_total: Decimal,
    pub issue_date: chrono::NaiveDate,
    pub status: String,
}

// ── Budget commitments (encumbrance) ────────────────────────────────────────

/// Budget vs committed vs actual by account. **Committed** is the value of open
/// POs (issued but not yet invoiced) charged to each account — the encumbrance
/// that ordinary budget-vs-actual misses. `available = budget - actual -
/// committed`. Only accounts with a budget or an open commitment are returned.
pub async fn budget_commitments(engine: &ErpEngine, entity_id: Uuid) -> ErpResult<serde_json::Value> {
    use std::collections::BTreeMap;
    let pool = engine.pool();

    let budget: Vec<(String, Decimal)> = sqlx::query_as(
        "SELECT account_code, COALESCE(SUM(amount),0) FROM budget_entries WHERE entity_id=$1 GROUP BY account_code",
    ).bind(entity_id).fetch_all(pool).await.unwrap_or_default();

    let committed: Vec<(String, Decimal)> = sqlx::query_as(
        r#"SELECT pol.account_code, COALESCE(SUM(pol.line_total),0)
           FROM purchase_order_lines pol JOIN purchase_orders po ON po.id = pol.po_id
           WHERE po.entity_id=$1 AND po.status IN ('issued','acknowledged','partially_invoiced')
             AND pol.account_code IS NOT NULL AND pol.account_code <> ''
           GROUP BY pol.account_code"#,
    ).bind(entity_id).fetch_all(pool).await.unwrap_or_default();

    let actual: Vec<(String, Decimal)> = sqlx::query_as(
        "SELECT account_code, COALESCE(SUM(debit_total - credit_total),0) FROM account_period_balances WHERE entity_id=$1 GROUP BY account_code",
    ).bind(entity_id).fetch_all(pool).await.unwrap_or_default();

    let names: Vec<(String, String)> = sqlx::query_as("SELECT code, name FROM accounts WHERE entity_id=$1").bind(entity_id).fetch_all(pool).await.unwrap_or_default();
    let name_map: BTreeMap<String, String> = names.into_iter().collect();

    // Merge: (budget, actual, committed) per account.
    let mut m: BTreeMap<String, (Decimal, Decimal, Decimal)> = BTreeMap::new();
    for (a, v) in budget { m.entry(a).or_default().0 += v; }
    for (a, v) in actual { m.entry(a).or_default().1 += v; }
    for (a, v) in committed { m.entry(a).or_default().2 += v; }

    let mut rows: Vec<serde_json::Value> = m.into_iter()
        // Keep accounts with a budget or an open commitment (ignore pure-actual noise).
        .filter(|(_, (b, _, c))| *b != Decimal::ZERO || *c != Decimal::ZERO)
        .map(|(code, (budget, actual, committed))| {
            let available = budget - actual - committed;
            serde_json::json!({
                "account_code": code,
                "account_name": name_map.get(&code).cloned().unwrap_or_default(),
                "budget": budget,
                "actual": actual,
                "committed": committed,
                "available": available,
                "over_budget": budget != Decimal::ZERO && available < Decimal::ZERO,
            })
        })
        .collect();
    rows.sort_by(|a, b| a["account_code"].as_str().unwrap_or("").cmp(b["account_code"].as_str().unwrap_or("")));

    Ok(serde_json::json!({ "accounts": rows }))
}

// ── Email the LPO to the vendor ─────────────────────────────────────────────

/// Email the legal LPO (PDF) to the vendor and stamp `sent_at`. Recipient is the
/// explicit address, else the vendor master's first email, else the vendor's
/// portal login email. Returns the address emailed (None if none on file — the
/// send is still recorded).
pub async fn send_purchase_order(
    engine: &ErpEngine,
    entity_id: Uuid,
    po_id: Uuid,
    recipient_email: Option<String>,
    message: Option<String>,
) -> ErpResult<Option<String>> {
    let po = sqlx::query_as::<_, PurchaseOrderRow>("SELECT * FROM purchase_orders WHERE id=$1 AND entity_id=$2")
        .bind(po_id).bind(entity_id).fetch_optional(engine.pool()).await?
        .ok_or_else(|| ErpError::NotFound { entity_type: "purchase order".into(), id: po_id })?;

    // Resolve recipient: explicit → vendor master email → portal login email.
    let recipient = match recipient_email.filter(|s| !s.trim().is_empty()) {
        Some(r) => Some(r),
        None => {
            let vemail: Option<serde_json::Value> = sqlx::query_scalar("SELECT email FROM vendors WHERE id=$1 AND entity_id=$2")
                .bind(po.vendor_id).bind(entity_id).fetch_optional(engine.pool()).await?.flatten();
            let from_master = vemail.and_then(|v| serde_json::from_value::<Vec<crate::types::ContactEmail>>(v).ok())
                .and_then(|es| es.into_iter().map(|e| e.email).find(|e| !e.is_empty()));
            match from_master {
                Some(e) => Some(e),
                None => sqlx::query_scalar::<_, String>("SELECT email FROM vendor_users WHERE vendor_id=$1 AND entity_id=$2 LIMIT 1")
                    .bind(po.vendor_id).bind(entity_id).fetch_optional(engine.pool()).await?,
            }
        }
    };

    // Always stamp the send action.
    sqlx::query("UPDATE purchase_orders SET sent_at=now() WHERE id=$1").bind(po_id).execute(engine.pool()).await?;

    let Some(recipient) = recipient else { return Ok(None); };

    let (pdf_bytes, number) = po_document_pdf(engine, entity_id, po_id).await?;
    let pdf_b64 = { use base64::{engine::general_purpose::STANDARD as B64, Engine as _}; B64.encode(&pdf_bytes) };

    let org_name = sqlx::query_scalar::<_, Option<String>>("SELECT organization_name FROM entity_settings WHERE entity_id=$1")
        .bind(entity_id).fetch_optional(engine.pool()).await?.flatten().unwrap_or_else(|| "Your Company".to_string());

    let intro = message.filter(|m| !m.trim().is_empty()).unwrap_or_else(||
        format!("Please find attached our Local Purchase Order {number}. Kindly acknowledge and supply strictly against the LPO number quoted."));
    let body = format!(
        "<div style=\"font-family:Helvetica,Arial,sans-serif;color:#1f2937\">\
         <p>Dear Supplier,</p><p>{}</p>\
         <p>Regards,<br/>{}</p></div>",
        html_escape(&intro), html_escape(&org_name),
    );
    let subject = format!("Purchase Order {number} from {org_name}");

    let (_enabled, mut channels) = crate::services::notification_prefs::effective_channels(
        engine, entity_id, &crate::notifications::NotificationEventType::InvoiceSent).await;
    if !channels.contains(&crate::types::Channel::Email) { channels.push(crate::types::Channel::Email); }

    let notif = crate::notifications::SendNotificationRequest {
        event_type: crate::notifications::NotificationEventType::InvoiceSent,
        channels,
        recipients: vec![recipient.clone()],
        subject: Some(subject),
        body,
        related_type: Some("PurchaseOrder".to_string()),
        related_id: Some(po_id),
        schedule_at: None,
        attachments: vec![crate::notifications::NotificationAttachment {
            filename: format!("{number}.pdf"),
            mime_type: "application/pdf".to_string(),
            content_base64: pdf_b64,
        }],
    };
    crate::services::notifications::send_notification(engine, entity_id, notif).await?;
    Ok(Some(recipient))
}

fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;")
}
