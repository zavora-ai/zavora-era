use chrono::Utc;
use rust_decimal::Decimal;
use uuid::Uuid;

use crate::ap::*;
use crate::engine::ErpEngine;
use crate::error::{ErpError, ErpResult};
use crate::types::AgentOrUserId;

/// Create a new bill (AP document).
pub async fn create_bill(
    engine: &ErpEngine,
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
    .bind(engine.entity_id())
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

    // Resolve lines
    let mut lines = Vec::new();
    for line_req in &req.lines {
        let mut line = crate::services::invoicing::resolve_bill_line(engine, line_req, &vendor).await?;
        line.compute_totals();
        lines.push(line);
    }

    let subtotal: Decimal = lines.iter().map(|l| l.line_total).sum();
    let tax_total: Decimal = lines.iter().map(|l| l.vat_amount).sum();

    // Auto-compute WHT based on vendor category
    let wht_amount = if let Some(ref wht_cat_str) = vendor.wht_category {
        let wht_category: Option<crate::types::WhtCategory> =
            serde_json::from_str(&format!("\"{}\"", wht_cat_str)).ok();
        if let Some(cat) = wht_category {
            let rate = cat.rate_for(vendor.resident);
            (subtotal * rate).round_dp(2)
        } else {
            Decimal::ZERO
        }
    } else {
        Decimal::ZERO
    };

    let gross_total = subtotal + tax_total - wht_amount;
    let number = generate_bill_number(engine).await?;

    sqlx::query(
        r#"INSERT INTO bills 
           (id, entity_id, number, vendor_id, vendor_invoice_number, issue_date, due_date, currency, fx_rate,
            subtotal, tax_total, wht_amount, gross_total, amount_paid, balance_due, status, notes, created_at)
           VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17, $18)"#,
    )
    .bind(id)
    .bind(engine.entity_id())
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
    .execute(engine.pool())
    .await?;

    Ok(Bill {
        id,
        entity_id: engine.entity_id(),
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
pub async fn approve_bill(engine: &ErpEngine, req: ApproveBillRequest) -> ErpResult<()> {
    let bill = sqlx::query_as::<_, BillRow>("SELECT * FROM bills WHERE id = $1 AND entity_id = $2")
        .bind(req.bill_id)
        .bind(engine.entity_id())
        .fetch_optional(engine.pool())
        .await?
        .ok_or_else(|| ErpError::NotFound {
            entity_type: "Bill".to_string(),
            id: req.bill_id,
        })?;

    if bill.status != "pending_approval" {
        return Err(ErpError::ValidationFailed {
            message: format!("Bill {} is not pending approval (status: {})", bill.number, bill.status),
        });
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

async fn generate_bill_number(engine: &ErpEngine) -> ErpResult<String> {
    let row = sqlx::query_scalar::<_, i64>(
        r#"UPDATE entity_settings 
           SET sequences = jsonb_set(sequences, '{bill_next}', to_jsonb((sequences->>'bill_next')::bigint + 1))
           WHERE entity_id = $1
           RETURNING (sequences->>'bill_next')::bigint - 1"#,
    )
    .bind(engine.entity_id())
    .fetch_one(engine.pool())
    .await?;

    let prefix = &engine.config().sequences.bill_prefix;
    let fiscal_year = Utc::now().format("%Y").to_string();
    Ok(format!("{}-{}-{:04}", prefix, fiscal_year, row))
}
