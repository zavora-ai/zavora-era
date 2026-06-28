use chrono::Utc;
use rust_decimal::Decimal;
use uuid::Uuid;

use crate::ap::*;
use crate::engine::ErpEngine;
use crate::error::{ErpError, ErpResult};
use crate::period::PeriodStatus;
use crate::types::AgentOrUserId;

/// Create a new bill (AP document).
pub async fn create_bill(
    engine: &ErpEngine,
    entity_id: Uuid,
    req: CreateBillRequest,
    created_by: &AgentOrUserId,
) -> ErpResult<Bill> {
    let id = Uuid::new_v4();
    let today = Utc::now().date_naive();

    // Look up vendor for defaults (WHT category, payment terms)
    let vendor = sqlx::query_as::<_, crate::parties::VendorRow>(
        "SELECT * FROM vendors WHERE id = $1 AND entity_id = $2",
    )
    .bind(req.vendor_id)
    .bind(entity_id)
    .fetch_optional(engine.pool())
    .await?
    .ok_or_else(|| ErpError::NotFound {
        entity_type: "Vendor".to_string(),
        id: req.vendor_id,
    })?;

    let currency = req.currency.unwrap_or(vendor.currency.clone());
    let issue_date = req.issue_date.unwrap_or(today);
    let payment_terms: crate::types::PaymentTerms =
        serde_json::from_str(&format!("\"{}\"", vendor.payment_terms))
            .unwrap_or(crate::types::PaymentTerms::Net30);
    let due_date = req.due_date.unwrap_or_else(|| payment_terms.due_date(issue_date));

    // Ensure default posting groups exist + masters are assigned (idempotent),
    // so the matrix can drive purchase-account derivation below.
    let _ = crate::posting::groups::ensure_default_posting_groups(engine, entity_id).await;

    // Resolve lines
    let mut lines = Vec::new();
    for line_req in &req.lines {
        let mut line = crate::services::invoicing::resolve_bill_line(engine, entity_id, line_req, &vendor).await?;
        line.compute_totals();
        lines.push(line);
    }

    let subtotal: Decimal = lines.iter().map(|l| l.line_total).sum();
    let tax_total: Decimal = lines.iter().map(|l| l.vat_amount).sum();

    // Auto-compute WHT based on vendor category
    let wht_amount = if let Some(ref wht_cat_str) = vendor.wht_category {
        // Stored as JSON, e.g. "Rent" — parse directly (no extra quoting), then
        // read the rate from the wht_rates table (single source of truth).
        match serde_json::from_str::<crate::types::WhtCategory>(wht_cat_str) {
            Ok(cat) => {
                let rate = crate::services::wht::wht_rate_for(engine, &cat, vendor.resident).await;
                (subtotal * rate).round_dp(2)
            }
            Err(_) => Decimal::ZERO,
        }
    } else {
        Decimal::ZERO
    };

    let gross_total = subtotal + tax_total - wht_amount;
    let number = generate_bill_number(engine, entity_id).await?;

    let mut tx = engine.pool().begin().await?;

    sqlx::query(
        r#"INSERT INTO bills 
           (id, entity_id, number, vendor_id, vendor_invoice_number, issue_date, due_date, currency, fx_rate,
            subtotal, tax_total, wht_amount, gross_total, amount_paid, balance_due, status, notes, created_at)
           VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17, $18)"#,
    )
    .bind(id)
    .bind(entity_id)
    .bind(&number)
    .bind(req.vendor_id)
    .bind(&req.vendor_invoice_number)
    .bind(issue_date)
    .bind(due_date)
    .bind(&currency)
    .bind(req.fx_rate.unwrap_or(Decimal::ONE))
    .bind(subtotal)
    .bind(tax_total)
    .bind(wht_amount)
    .bind(gross_total)
    .bind(Decimal::ZERO)
    .bind(gross_total)
    .bind("draft")
    .bind(&req.notes)
    .bind(Utc::now())
    .execute(&mut *tx)
    .await?;

    // Insert bill lines
    for line in &lines {
        sqlx::query(
            r#"INSERT INTO bill_lines
               (id, bill_id, product_id, description, quantity, unit_price, discount_percent, account_code, vat_treatment, line_total, vat_amount, dimensions)
               VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12)"#,
        )
        .bind(line.id)
        .bind(id)
        .bind(line.product_id)
        .bind(&line.description)
        .bind(line.quantity)
        .bind(line.unit_price)
        .bind(line.discount_percent)
        .bind(&line.account_code)
        .bind(serde_json::to_string(&line.vat_treatment).unwrap_or_default())
        .bind(line.line_total)
        .bind(line.vat_amount)
        .bind(serde_json::to_value(&line.dimensions).unwrap_or_default())
        .execute(&mut *tx)
        .await?;
    }

    tx.commit().await?;

    Ok(Bill {
        id,
        entity_id,
        number,
        vendor_id: req.vendor_id,
        vendor_invoice_number: req.vendor_invoice_number,
        issue_date,
        due_date,
        currency,
        fx_rate: req.fx_rate.unwrap_or(Decimal::ONE),
        lines,
        tax_lines: Vec::new(),
        subtotal,
        tax_total,
        wht_amount,
        gross_total,
        amount_paid: Decimal::ZERO,
        balance_due: gross_total,
        status: BillStatus::Draft,
        journal_entry_id: None,
        approved_by: None,
        approved_at: None,
        notes: req.notes,
        created_at: Utc::now(),
    })
}

/// Approve a bill.
///
/// Validates that:
/// 1. The bill is in PendingApproval status
/// 2. The target fiscal period (for the bill's issue_date) is Open
///
/// If the period is SoftClosed or HardClosed, the approval is rejected with
/// an error identifying the closed period (Requirements 10.3, 10.5, 10.6).
pub async fn approve_bill(engine: &ErpEngine, entity_id: Uuid, req: ApproveBillRequest) -> ErpResult<()> {
    let bill = sqlx::query_as::<_, BillRow>("SELECT * FROM bills WHERE id = $1 AND entity_id = $2")
        .bind(req.bill_id)
        .bind(entity_id)
        .fetch_optional(engine.pool())
        .await?
        .ok_or_else(|| ErpError::NotFound {
            entity_type: "Bill".to_string(),
            id: req.bill_id,
        })?;

    // A bill is approved directly from draft (the UI offers Approve on drafts);
    // `pending_approval` is also accepted for flows that add a submit step.
    if bill.status != "draft" && bill.status != "pending_approval" {
        return Err(ErpError::ValidationFailed {
            message: format!("Bill {} cannot be approved (status: {})", bill.number, bill.status),
        });
    }

    // Validate target fiscal period is Open (Requirements 10.5, 10.6)
    let period = crate::services::periods::period_for_date(engine, entity_id, bill.issue_date).await?;
    let period_status = period.parsed_status();

    match period_status {
        PeriodStatus::SoftClosed | PeriodStatus::HardClosed => {
            return Err(ErpError::PeriodClosedDetailed {
                period_name: period.name.clone(),
                status: format!("{:?}", period_status),
                period_id: period.id,
            });
        }
        PeriodStatus::Open | PeriodStatus::Future => {
            // OK — posting is allowed
        }
    }

    sqlx::query(
        "UPDATE bills SET status = 'approved', approved_by = $1, approved_at = $2 WHERE id = $3",
    )
    .bind(req.approved_by)
    .bind(Utc::now())
    .bind(req.bill_id)
    .execute(engine.pool())
    .await?;

    Ok(())
}

/// Edit a **draft** bill — replaces its lines and recomputes totals.
///
/// Only permitted while the bill is a draft (not yet approved/posted to the ledger);
/// approved/posted bills are immutable.
pub async fn update_bill_draft(
    engine: &ErpEngine,
    entity_id: Uuid,
    bill_id: Uuid,
    req: CreateBillRequest,
) -> ErpResult<()> {
    let bill = sqlx::query_as::<_, BillRow>("SELECT * FROM bills WHERE id = $1 AND entity_id = $2")
        .bind(bill_id)
        .bind(entity_id)
        .fetch_optional(engine.pool())
        .await?
        .ok_or_else(|| ErpError::NotFound { entity_type: "Bill".to_string(), id: bill_id })?;

    if bill.status != "draft" {
        return Err(ErpError::ValidationFailed {
            message: format!(
                "Only draft bills can be edited; bill {} is '{}'. Void it instead.",
                bill.number, bill.status
            ),
        });
    }

    let today = Utc::now().date_naive();
    let vendor = sqlx::query_as::<_, crate::parties::VendorRow>(
        "SELECT * FROM vendors WHERE id = $1 AND entity_id = $2",
    )
    .bind(req.vendor_id)
    .bind(entity_id)
    .fetch_optional(engine.pool())
    .await?
    .ok_or_else(|| ErpError::NotFound { entity_type: "Vendor".to_string(), id: req.vendor_id })?;

    let currency = req.currency.clone().unwrap_or_else(|| vendor.currency.clone());
    let issue_date = req.issue_date.unwrap_or(today);
    let payment_terms: crate::types::PaymentTerms =
        serde_json::from_str(&format!("\"{}\"", vendor.payment_terms))
            .unwrap_or(crate::types::PaymentTerms::Net30);
    let due_date = req.due_date.unwrap_or_else(|| payment_terms.due_date(issue_date));

    // Ensure default posting groups exist + masters are assigned (idempotent),
    // so the matrix can drive purchase-account derivation below.
    let _ = crate::posting::groups::ensure_default_posting_groups(engine, entity_id).await;

    // Resolve lines
    let mut lines = Vec::new();
    for line_req in &req.lines {
        let mut line = crate::services::invoicing::resolve_bill_line(engine, entity_id, line_req, &vendor).await?;
        line.compute_totals();
        lines.push(line);
    }

    let subtotal: Decimal = lines.iter().map(|l| l.line_total).sum();
    let tax_total: Decimal = lines.iter().map(|l| l.vat_amount).sum();

    // Auto-compute WHT based on vendor category
    let wht_amount = if let Some(ref wht_cat_str) = vendor.wht_category {
        // Stored as JSON, e.g. "Rent" — parse directly (no extra quoting), then
        // read the rate from the wht_rates table (single source of truth).
        match serde_json::from_str::<crate::types::WhtCategory>(wht_cat_str) {
            Ok(cat) => {
                let rate = crate::services::wht::wht_rate_for(engine, &cat, vendor.resident).await;
                (subtotal * rate).round_dp(2)
            }
            Err(_) => Decimal::ZERO,
        }
    } else {
        Decimal::ZERO
    };

    let gross_total = subtotal + tax_total - wht_amount;

    let mut tx = engine.pool().begin().await?;

    sqlx::query(
        r#"UPDATE bills
           SET vendor_id = $1, vendor_invoice_number = $2, issue_date = $3, due_date = $4,
               currency = $5, fx_rate = $6, subtotal = $7, tax_total = $8, wht_amount = $9,
               gross_total = $10, balance_due = $10, notes = $11
           WHERE id = $12"#,
    )
    .bind(req.vendor_id)
    .bind(&req.vendor_invoice_number)
    .bind(issue_date)
    .bind(due_date)
    .bind(&currency)
    .bind(req.fx_rate.unwrap_or(Decimal::ONE))
    .bind(subtotal)
    .bind(tax_total)
    .bind(wht_amount)
    .bind(gross_total)
    .bind(&req.notes)
    .bind(bill_id)
    .execute(&mut *tx)
    .await?;

    // Replace bill lines
    sqlx::query("DELETE FROM bill_lines WHERE bill_id = $1")
        .bind(bill_id)
        .execute(&mut *tx)
        .await?;

    for line in &lines {
        sqlx::query(
            r#"INSERT INTO bill_lines
               (id, bill_id, product_id, description, quantity, unit_price, discount_percent, account_code, vat_treatment, line_total, vat_amount)
               VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)"#,
        )
        .bind(line.id)
        .bind(bill_id)
        .bind(line.product_id)
        .bind(&line.description)
        .bind(line.quantity)
        .bind(line.unit_price)
        .bind(line.discount_percent)
        .bind(&line.account_code)
        .bind(serde_json::to_string(&line.vat_treatment).unwrap_or_default())
        .bind(line.line_total)
        .bind(line.vat_amount)
        .execute(&mut *tx)
        .await?;
    }

    tx.commit().await?;
    Ok(())
}

/// Delete a **draft** bill and its line items. Only drafts can be deleted;
/// approved/posted bills must be voided so the ledger stays intact.
pub async fn delete_bill_draft(
    engine: &ErpEngine,
    entity_id: Uuid,
    bill_id: Uuid,
) -> ErpResult<()> {
    let row = sqlx::query_as::<_, (String, String)>(
        "SELECT number, status FROM bills WHERE id = $1 AND entity_id = $2",
    )
    .bind(bill_id)
    .bind(entity_id)
    .fetch_optional(engine.pool())
    .await?
    .ok_or_else(|| ErpError::NotFound { entity_type: "Bill".to_string(), id: bill_id })?;

    if row.1 != "draft" {
        return Err(ErpError::ValidationFailed {
            message: format!(
                "Only draft bills can be deleted; bill {} is '{}'. Void it instead.",
                row.0, row.1
            ),
        });
    }

    let mut tx = engine.pool().begin().await?;
    sqlx::query("DELETE FROM bill_lines WHERE bill_id = $1")
        .bind(bill_id)
        .execute(&mut *tx)
        .await?;
    sqlx::query("DELETE FROM bills WHERE id = $1 AND entity_id = $2")
        .bind(bill_id)
        .bind(entity_id)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    Ok(())
}

async fn generate_bill_number(engine: &ErpEngine, entity_id: Uuid) -> ErpResult<String> {
    let row = sqlx::query_scalar::<_, i64>(
        r#"UPDATE entity_settings 
           SET sequences = jsonb_set(sequences, '{bill_next}', to_jsonb((sequences->>'bill_next')::bigint + 1))
           WHERE entity_id = $1
           RETURNING (sequences->>'bill_next')::bigint - 1"#,
    )
    .bind(entity_id)
    .fetch_one(engine.pool())
    .await?;

    let cfg = engine.config_for(entity_id).await?;
    let prefix = &cfg.sequences.bill_prefix;
    let fiscal_year = Utc::now().format("%Y").to_string();
    Ok(format!("{}-{}-{:04}", prefix, fiscal_year, row))
}
