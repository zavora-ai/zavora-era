//! Supplier (AP) credit note service.
//!
//! A supplier credit note records a reduction the vendor has issued against a
//! prior bill (e.g. returned goods, a price adjustment, or an over-billing).
//! It is the AP mirror of the customer credit note and the KRA-correct way to
//! reduce a previously recorded purchase: rather than editing/voiding a bill
//! that may already be reflected in filed input VAT, a credit note reverses the
//! relevant amounts with a clear audit link to the original bill.
//!
//! Posting (reverse of bill posting):
//!   DR Accounts Payable            (gross_total — reduce what we owe)
//!     CR Expense (per line)        (line_total — reverse the cost)
//!     CR VAT Input (per line)      (vat_amount — reverse claimed input VAT)
//!
//! Header, line items, and the reversing journal entry all commit or roll back
//! together (Requirement 2.3 atomicity).

use chrono::Utc;
use rust_decimal::Decimal;
use uuid::Uuid;

use crate::ap::supplier_credit_note::*;
use crate::engine::ErpEngine;
use crate::error::{ErpError, ErpResult};
use crate::invoicing::line::InvoiceLine;
use crate::ledger::journal::{CreateJournalEntryRequest, CreateJournalLineRequest, JournalSource};
use crate::types::AgentOrUserId;

/// Create and post a supplier credit note with line items.
pub async fn create_supplier_credit_note(
    engine: &ErpEngine,
    entity_id: Uuid,
    req: CreateSupplierCreditNoteRequest,
    created_by: &AgentOrUserId,
) -> ErpResult<SupplierCreditNote> {
    let today = Utc::now().date_naive();
    let cn_date = req.credit_note_date.unwrap_or(today);

    // Validate the vendor belongs to this tenant.
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

    // If it applies to a specific bill, validate that bill is this tenant's.
    if let Some(bill_id) = req.applies_to_bill {
        let exists = sqlx::query_scalar::<_, bool>(
            "SELECT EXISTS(SELECT 1 FROM bills WHERE id = $1 AND entity_id = $2)",
        )
        .bind(bill_id)
        .bind(entity_id)
        .fetch_one(engine.pool())
        .await?;
        if !exists {
            return Err(ErpError::NotFound { entity_type: "Bill".to_string(), id: bill_id });
        }
    }

    let mut lines: Vec<InvoiceLine> = Vec::new();

    if req.lines.is_empty() {
        // Full reversal — copy all lines from the original bill (mirrors the
        // customer credit note pattern in create_credit_note()).
        if let Some(bill_id) = req.applies_to_bill {
            let bill_lines = sqlx::query_as::<_, (Option<Uuid>, String, Decimal, Decimal, Decimal, String, String, Decimal, Decimal)>(
                r#"SELECT product_id, description, quantity, unit_price, discount_percent,
                          account_code, vat_treatment, line_total, vat_amount
                   FROM bill_lines WHERE bill_id = $1"#,
            )
            .bind(bill_id)
            .fetch_all(engine.pool())
            .await?;

            if bill_lines.is_empty() {
                return Err(ErpError::ValidationFailed {
                    message: "Bill has no lines to reverse".to_string(),
                });
            }

            for (product_id, description, quantity, unit_price, discount_percent, account_code, vat_treatment, line_total, vat_amount) in &bill_lines {
                lines.push(InvoiceLine {
                    id: Uuid::new_v4(),
                    product_id: *product_id,
                    description: description.clone(),
                    quantity: *quantity,
                    unit_price: *unit_price,
                    discount_percent: *discount_percent,
                    account_code: account_code.clone(),
                    vat_treatment: serde_json::from_str(vat_treatment)
                        .unwrap_or(crate::types::VatTreatment::Standard16),
                    line_total: *line_total,
                    vat_amount: *vat_amount,
                    dimensions: Default::default(),
                });
            }
        } else {
            return Err(ErpError::ValidationFailed {
                message: "A supplier credit note must have at least one line, or specify applies_to_bill for a full reversal".to_string(),
            });
        }
    } else {
        // Explicit lines provided — resolve and compute totals.
        for line_req in &req.lines {
            let mut line = crate::services::invoicing::resolve_invoice_line(engine, entity_id, line_req, None).await?;
            line.compute_totals();
            lines.push(line);
        }
    }
    let subtotal: Decimal = lines.iter().map(|l| l.line_total).sum();
    let tax_total: Decimal = lines.iter().map(|l| l.vat_amount).sum();
    let gross_total = crate::money::round_money(subtotal + tax_total);

    let currency = req.currency.clone().unwrap_or_else(|| vendor.currency.clone());
    let fx_rate = req.fx_rate.unwrap_or(Decimal::ONE);
    let posting = engine.posting_for(entity_id).await?;

    let number = match req.credit_note_number {
        Some(ref n) if !n.trim().is_empty() => n.trim().to_string(),
        _ => generate_supplier_cn_number(engine, entity_id).await?,
    };
    let cn_id = Uuid::new_v4();

    let mut tx = engine.pool().begin().await?;

    // Header
    sqlx::query(
        r#"INSERT INTO supplier_credit_notes
           (id, entity_id, vendor_id, credit_note_number, credit_note_date, applies_to_bill,
            subtotal, tax_total, gross_total, currency, fx_rate, reason, status, etims_status, created_at)
           VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15)"#,
    )
    .bind(cn_id)
    .bind(entity_id)
    .bind(req.vendor_id)
    .bind(&number)
    .bind(cn_date)
    .bind(req.applies_to_bill)
    .bind(subtotal)
    .bind(tax_total)
    .bind(gross_total)
    .bind(&currency)
    .bind(fx_rate)
    .bind(&req.reason)
    .bind("posted")
    .bind("not_transmitted")
    .bind(Utc::now())
    .execute(&mut *tx)
    .await?;

    // Lines
    for line in &lines {
        sqlx::query(
            r#"INSERT INTO supplier_credit_note_lines
               (id, credit_note_id, product_id, description, quantity, unit_price, vat_treatment, vat_amount, gl_account_code, line_total)
               VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)"#,
        )
        .bind(line.id)
        .bind(cn_id)
        .bind(line.product_id)
        .bind(&line.description)
        .bind(line.quantity)
        .bind(line.unit_price)
        .bind(serde_json::to_string(&line.vat_treatment).unwrap_or_default())
        .bind(line.vat_amount)
        .bind(&line.account_code)
        .bind(line.line_total)
        .execute(&mut *tx)
        .await?;
    }

    // Reversing journal entry: DR AP / CR expense (per line) / CR VAT input.
    let mut je_lines: Vec<CreateJournalLineRequest> = Vec::new();
    let ap_account = crate::posting::groups::resolve_payables(engine, entity_id, req.vendor_id)
        .await
        .unwrap_or_else(|| posting.accounts_payable.clone());
    je_lines.push(CreateJournalLineRequest {
        account_code: ap_account,
        debit: Some(gross_total),
        credit: None,
        currency: currency.clone(),
        fx_rate: Some(fx_rate),
        description: Some(format!("Supplier credit note {number} - AP reduction")),
        dimensions: None,
    });
    for line in &lines {
        je_lines.push(CreateJournalLineRequest {
            account_code: line.account_code.clone(),
            debit: None,
            credit: Some(line.line_total),
            currency: currency.clone(),
            fx_rate: Some(fx_rate),
            description: Some(format!("SCN reversal: {}", line.description)),
            dimensions: None,
        });
        if line.vat_amount > Decimal::ZERO {
            let vat_account = crate::posting::groups::resolve_vat_input(engine, entity_id, req.vendor_id, line.product_id)
                .await
                .unwrap_or_else(|| posting.vat_input.clone());
            je_lines.push(CreateJournalLineRequest {
                account_code: vat_account,
                debit: None,
                credit: Some(line.vat_amount),
                currency: currency.clone(),
                fx_rate: Some(fx_rate),
                description: Some(format!("SCN VAT input reversal: {}", line.description)),
                dimensions: None,
            });
        }
    }

    let entry_req = CreateJournalEntryRequest {
        date: cn_date,
        source: JournalSource::SupplierCreditNote,
        source_id: Some(cn_id),
        reference: number.clone(),
        description: format!("Supplier credit note {number}"),
        lines: je_lines,
        post_immediately: true,
    };

    let period = crate::services::periods::period_for_date(engine, entity_id, cn_date).await?;
    let entry = crate::services::journal::create_and_post_in_tx(
        &mut tx,
        engine,
        entity_id,
        entry_req,
        period.id,
        created_by.clone(),
    )
    .await?;

    sqlx::query("UPDATE supplier_credit_notes SET journal_entry_id = $1 WHERE id = $2")
        .bind(entry.id)
        .bind(cn_id)
        .execute(&mut *tx)
        .await?;

    tx.commit().await?;

    Ok(SupplierCreditNote {
        id: cn_id,
        entity_id,
        vendor_id: req.vendor_id,
        credit_note_number: number,
        credit_note_date: cn_date,
        applies_to_bill: req.applies_to_bill,
        lines,
        tax_lines: Vec::new(),
        gross_total,
        status: ApDocStatus::Posted,
        journal_entry_id: Some(entry.id),
        created_at: Utc::now(),
    })
}

/// List supplier credit notes for a tenant.
pub async fn list_supplier_credit_notes(
    engine: &ErpEngine,
    entity_id: Uuid,
) -> ErpResult<Vec<SupplierCreditNoteRow>> {
    let rows = sqlx::query_as::<_, SupplierCreditNoteRow>(
        "SELECT id, entity_id, vendor_id, credit_note_number, credit_note_date, applies_to_bill, \
                gross_total, status, journal_entry_id, created_at \
         FROM supplier_credit_notes WHERE entity_id = $1 ORDER BY created_at DESC",
    )
    .bind(entity_id)
    .fetch_all(engine.pool())
    .await?;
    Ok(rows)
}

/// Fetch a single supplier credit note header (tenant-scoped).
pub async fn get_supplier_credit_note(
    engine: &ErpEngine,
    entity_id: Uuid,
    id: Uuid,
) -> ErpResult<Option<SupplierCreditNoteRow>> {
    let row = sqlx::query_as::<_, SupplierCreditNoteRow>(
        "SELECT id, entity_id, vendor_id, credit_note_number, credit_note_date, applies_to_bill, \
                gross_total, status, journal_entry_id, created_at \
         FROM supplier_credit_notes WHERE id = $1 AND entity_id = $2",
    )
    .bind(id)
    .bind(entity_id)
    .fetch_optional(engine.pool())
    .await?;
    Ok(row)
}

/// Allocate the next supplier-credit-note number from its own `SCN` sequence,
/// independent of the customer credit-note counter. `COALESCE(..., 1)` seeds the
/// counter at 1 for tenants whose settings predate this key.
async fn generate_supplier_cn_number(engine: &ErpEngine, entity_id: Uuid) -> ErpResult<String> {
    let row = sqlx::query_scalar::<_, i64>(
        r#"UPDATE entity_settings
           SET sequences = jsonb_set(
               sequences,
               '{supplier_credit_note_next}',
               to_jsonb(COALESCE((sequences->>'supplier_credit_note_next')::bigint, 1) + 1)
           )
           WHERE entity_id = $1
           RETURNING COALESCE((sequences->>'supplier_credit_note_next')::bigint, 1) - 1"#,
    )
    .bind(entity_id)
    .fetch_one(engine.pool())
    .await?;
    let fiscal_year = Utc::now().format("%Y").to_string();
    Ok(format!("SCN-{fiscal_year}-{row:04}"))
}
