use chrono::Utc;
use rust_decimal::Decimal;
use uuid::Uuid;

use crate::engine::ErpEngine;
use crate::error::{ErpError, ErpResult};
use crate::invoicing::*;
use crate::ledger::journal::{CreateJournalEntryRequest, CreateJournalLineRequest, JournalSource};
use crate::types::AgentOrUserId;

/// Lightweight row for querying stock movement cost data during credit note processing.
#[derive(Debug, Clone, sqlx::FromRow)]
struct StockMovementRow {
    #[allow(dead_code)]
    pub id: Uuid,
    #[allow(dead_code)]
    pub item_id: Uuid,
    pub unit_cost: Decimal,
    #[allow(dead_code)]
    pub quantity: Decimal,
}

/// Create a new invoice.
pub async fn create_invoice(
    engine: &ErpEngine,
    entity_id: Uuid,
    req: CreateInvoiceRequest,
    _created_by: &AgentOrUserId,
) -> ErpResult<Invoice> {
    let today = Utc::now().date_naive();
    let id = Uuid::new_v4();

    // Look up customer for defaults
    let customer = sqlx::query_as::<_, crate::parties::CustomerRow>(
        "SELECT * FROM customers WHERE id = $1 AND entity_id = $2",
    )
    .bind(req.customer_id)
    .bind(entity_id)
    .fetch_optional(engine.pool())
    .await?
    .ok_or_else(|| ErpError::NotFound {
        entity_type: "Customer".to_string(),
        id: req.customer_id,
    })?;

    let currency = req.currency.unwrap_or(customer.currency.clone());
    let issue_date = req.issue_date.unwrap_or(today);

    // Determine due date from customer payment terms
    let payment_terms: crate::types::PaymentTerms =
        serde_json::from_str(&format!("\"{}\"", customer.payment_terms))
            .unwrap_or(crate::types::PaymentTerms::Net30);
    let due_date = req.due_date.unwrap_or_else(|| payment_terms.due_date(issue_date));

    // Resolve invoice lines (auto-fill from products if product_id specified)
    let mut lines = Vec::new();
    for line_req in &req.lines {
        let mut line = resolve_invoice_line(engine, entity_id, line_req).await?;
        line.compute_totals();
        lines.push(line);
    }

    // Calculate totals
    // Each line total/VAT is already rounded to 2dp in `compute_totals`, so the
    // sums are exact at 2dp. The per-line discount is a fresh multiplication, so
    // round each line's discount before summing (Req 5.1, 5.2).
    let subtotal: Decimal = lines.iter().map(|l| l.line_total).sum();
    let tax_total: Decimal = lines.iter().map(|l| l.vat_amount).sum();
    let discount_total: Decimal = lines.iter().map(|l| {
        let gross = l.quantity * l.unit_price;
        crate::money::round_money(gross * l.discount_percent / Decimal::new(100, 0))
    }).sum();
    let gross_total = crate::money::round_money(subtotal + tax_total);

    // Generate invoice number
    let number = generate_invoice_number(engine, entity_id).await?;

    // Insert header + lines atomically so a failure cannot leave an invoice
    // header without its line items.
    let mut tx = engine.pool().begin().await?;

    sqlx::query(
        r#"INSERT INTO invoices 
           (id, entity_id, number, invoice_type, customer_id, issue_date, due_date, currency, fx_rate,
            subtotal, discount_total, tax_total, gross_total, amount_paid, balance_due, status,
            source_estimate, template_id, notes, created_at)
           VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17, $18, $19, $20)"#,
    )
    .bind(id)
    .bind(entity_id)
    .bind(&number)
    .bind("invoice")
    .bind(req.customer_id)
    .bind(issue_date)
    .bind(due_date)
    .bind(&currency)
    .bind(req.fx_rate.unwrap_or(Decimal::ONE))
    .bind(subtotal)
    .bind(discount_total)
    .bind(tax_total)
    .bind(gross_total)
    .bind(Decimal::ZERO)
    .bind(gross_total)
    .bind("draft")
    .bind::<Option<Uuid>>(None)
    .bind(req.template_id)
    .bind(&req.notes)
    .bind(Utc::now())
    .execute(&mut *tx)
    .await?;

    // Insert invoice lines
    for line in &lines {
        sqlx::query(
            r#"INSERT INTO invoice_lines
               (id, invoice_id, product_id, description, quantity, unit_price, discount_percent, account_code, vat_treatment, line_total, vat_amount, dimensions)
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

    Ok(Invoice {
        id,
        entity_id,
        number,
        invoice_type: InvoiceType::Invoice,
        customer_id: req.customer_id,
        issue_date,
        due_date,
        currency,
        fx_rate: req.fx_rate.unwrap_or(Decimal::ONE),
        lines,
        tax_lines: Vec::new(),
        subtotal,
        discount_total,
        tax_total,
        gross_total,
        amount_paid: Decimal::ZERO,
        balance_due: gross_total,
        status: InvoiceStatus::Draft,
        source_estimate: None,
        credit_note_for: None,
        journal_entry_id: None,
        sent_at: None,
        viewed_at: None,
        paid_at: None,
        template_id: req.template_id,
        notes: req.notes,
        attachments: Vec::new(),
    })
}

/// Create a credit note against an existing invoice.
///
/// This function:
/// 1. Looks up the original invoice
/// 2. Creates a new invoice record with type CreditNote, linked to original
/// 3. Creates a reversal GL entry: DR Revenue / DR VAT Output / CR AR
/// 4. Reduces balance_due on original invoice
pub async fn create_credit_note(
    engine: &ErpEngine,
    entity_id: Uuid,
    req: CreateCreditNoteRequest,
    created_by: &AgentOrUserId,
) -> ErpResult<CreditNoteResult> {
    let today = Utc::now().date_naive();
    let cn_date = req.date.unwrap_or(today);

    // Look up the original invoice
    let original = sqlx::query_as::<_, InvoiceRow>(
        "SELECT * FROM invoices WHERE id = $1 AND entity_id = $2",
    )
    .bind(req.invoice_id)
    .bind(entity_id)
    .fetch_optional(engine.pool())
    .await?
    .ok_or_else(|| ErpError::NotFound {
        entity_type: "Invoice".to_string(),
        id: req.invoice_id,
    })?;

    if original.invoice_type != "invoice" {
        return Err(ErpError::ValidationFailed {
            message: "Cannot create credit note against a non-invoice document".to_string(),
        });
    }

    // Principle: a credit note is the ONLY way to cancel/reduce a *posted*
    // invoice (transmitted to eTIMS or not). A draft has not entered the ledger,
    // so it is edited or deleted instead — never credit-noted.
    if original.status == "draft" {
        return Err(ErpError::ValidationFailed {
            message: "Cannot credit-note a draft invoice; edit or delete the draft instead".to_string(),
        });
    }
    if original.status == "voided" {
        return Err(ErpError::ValidationFailed {
            message: "Invoice is already cancelled".to_string(),
        });
    }

    // Determine credit note lines — if empty, full reversal of original
    let original_lines = sqlx::query_as::<_, InvoiceLineRow>(
        "SELECT * FROM invoice_lines WHERE invoice_id = $1",
    )
    .bind(req.invoice_id)
    .fetch_all(engine.pool())
    .await?;

    let cn_lines: Vec<InvoiceLine> = if req.lines.is_empty() {
        // Full reversal — copy all original lines
        original_lines
            .iter()
            .map(|l| InvoiceLine {
                id: Uuid::new_v4(),
                product_id: l.product_id,
                description: l.description.clone(),
                quantity: l.quantity,
                unit_price: l.unit_price,
                discount_percent: l.discount_percent,
                account_code: l.account_code.clone(),
                vat_treatment: serde_json::from_str(&l.vat_treatment)
                    .unwrap_or(crate::types::VatTreatment::Standard16),
                line_total: l.line_total,
                vat_amount: l.vat_amount,
                dimensions: serde_json::from_value(l.dimensions.clone()).unwrap_or_default(),
            })
            .collect()
    } else {
        // Partial credit note — resolve specified lines
        let mut lines = Vec::new();
        for line_req in &req.lines {
            let mut line = resolve_invoice_line(engine, entity_id, line_req).await?;
            line.compute_totals();
            lines.push(line);
        }
        lines
    };

    // Calculate totals
    let subtotal: Decimal = cn_lines.iter().map(|l| l.line_total).sum();
    let tax_total: Decimal = cn_lines.iter().map(|l| l.vat_amount).sum();
    let gross_total = crate::money::round_money(subtotal + tax_total);

    // Validate credit note doesn't exceed remaining balance
    if gross_total > original.balance_due {
        return Err(ErpError::ValidationFailed {
            message: format!(
                "Credit note amount ({}) exceeds remaining balance on invoice ({})",
                gross_total, original.balance_due
            ),
        });
    }

    // Generate credit note number
    let cn_number = generate_credit_note_number(engine, entity_id).await?;
    let cn_id = Uuid::new_v4();

    // The credit note record, its lines, the reversing journal entry, and the
    // original invoice's balance adjustment all commit or roll back together
    // (Requirement 2.3).
    let mut tx = engine.pool().begin().await?;

    // Insert credit note as invoice record with type 'credit_note'
    sqlx::query(
        r#"INSERT INTO invoices
           (id, entity_id, number, invoice_type, customer_id, issue_date, due_date, currency, fx_rate,
            subtotal, discount_total, tax_total, gross_total, amount_paid, balance_due, status,
            credit_note_for, notes, created_at)
           VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17, $18, $19)"#,
    )
    .bind(cn_id)
    .bind(entity_id)
    .bind(&cn_number)
    .bind("credit_note")
    .bind(original.customer_id)
    .bind(cn_date)
    .bind(cn_date) // credit notes are due immediately
    .bind(&original.currency)
    .bind(original.fx_rate)
    .bind(subtotal)
    .bind(Decimal::ZERO) // no discount on credit note
    .bind(tax_total)
    .bind(gross_total)
    .bind(gross_total) // fully "paid" (applied)
    .bind(Decimal::ZERO)
    .bind("paid") // credit notes are immediately applied
    .bind(req.invoice_id)
    .bind(&req.reason)
    .bind(Utc::now())
    .execute(&mut *tx)
    .await?;

    // Insert credit note lines
    for line in &cn_lines {
        sqlx::query(
            r#"INSERT INTO invoice_lines
               (id, invoice_id, product_id, description, quantity, unit_price, discount_percent, account_code, vat_treatment, line_total, vat_amount, dimensions)
               VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12)"#,
        )
        .bind(line.id)
        .bind(cn_id)
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

    // Create reversal GL entry: DR Revenue / DR VAT Output / CR AR
    let mut journal_lines = Vec::new();

    // CR Accounts Receivable (reduce AR)
    journal_lines.push(CreateJournalLineRequest {
        account_code: engine.posting_for(entity_id).await?.accounts_receivable.clone(),
        debit: None,
        credit: Some(gross_total),
        currency: original.currency.clone(),
        fx_rate: Some(original.fx_rate),
        description: Some(format!("Credit note {} against {}", cn_number, original.number)),
        dimensions: None,
    });

    // DR Revenue (per line) and DR VAT Output
    for line in &cn_lines {
        journal_lines.push(CreateJournalLineRequest {
            account_code: line.account_code.clone(),
            debit: Some(line.line_total),
            credit: None,
            currency: original.currency.clone(),
            fx_rate: Some(original.fx_rate),
            description: Some(format!("CN reversal: {}", line.description)),
            dimensions: None,
        });

        if line.vat_amount > Decimal::ZERO {
            journal_lines.push(CreateJournalLineRequest {
                account_code: engine.posting_for(entity_id).await?.vat_output.clone(),
                debit: Some(line.vat_amount),
                credit: None,
                currency: original.currency.clone(),
                fx_rate: Some(original.fx_rate),
                description: Some(format!("CN VAT reversal: {}", line.description)),
                dimensions: None,
            });
        }
    }

    // === Inventory return logic (Requirements 23.4, 23.5) ===
    // For each credit note line item where the original invoice issued inventory,
    // receive stock back and reverse COGS journal lines (DR Inventory / CR COGS).
    for line in &cn_lines {
        if let Some(product_id) = line.product_id {
            let product = sqlx::query_as::<_, crate::catalog::ProductRow>(
                "SELECT * FROM products WHERE id = $1 AND entity_id = $2",
            )
            .bind(product_id)
            .bind(entity_id)
            .fetch_optional(engine.pool())
            .await?;

            if let Some(product) = product {
                if product.track_inventory {
                    // Resolve the inventory_item_id linked to this product
                    let inventory_item_id = match product.inventory_item_id {
                        Some(id) => id,
                        None => continue, // skip if no inventory link (defensive)
                    };

                    // Look up the original stock movement from the invoice to find
                    // the unit cost at which goods were issued
                    let original_movement = sqlx::query_as::<_, StockMovementRow>(
                        r#"SELECT id, item_id, unit_cost, quantity 
                           FROM stock_movements 
                           WHERE reference_id = $1 
                             AND item_id = $2 
                             AND entity_id = $3
                             AND movement_type = 'issue'
                           ORDER BY created_at DESC
                           LIMIT 1"#,
                    )
                    .bind(req.invoice_id)
                    .bind(inventory_item_id)
                    .bind(entity_id)
                    .fetch_optional(engine.pool())
                    .await?;

                    let original_cost = match original_movement {
                        Some(ref mov) => mov.unit_cost,
                        None => continue, // no stock was issued for this item; skip
                    };

                    // Receive inventory back at original cost
                    let receive_req = crate::inventory::ReceiveInventoryRequest {
                        item_id: inventory_item_id,
                        quantity: line.quantity,
                        unit_cost: original_cost,
                        date: Some(cn_date),
                        reference_id: Some(cn_id),
                        warehouse_id: None,
                    };

                    crate::services::inventory::receive_inventory_in_tx(
                        &mut tx,
                        entity_id,
                        receive_req,
                        created_by,
                    )
                    .await?;

                    // Reverse COGS: DR Inventory / CR COGS
                    let return_cost = line.quantity * original_cost;

                    // Look up GL accounts from inventory item
                    let inv_item = sqlx::query_as::<_, crate::inventory::InventoryItemRow>(
                        "SELECT * FROM inventory_items WHERE id = $1 AND entity_id = $2",
                    )
                    .bind(inventory_item_id)
                    .bind(entity_id)
                    .fetch_one(engine.pool())
                    .await?;

                    // DR Inventory (return stock value)
                    journal_lines.push(CreateJournalLineRequest {
                        account_code: inv_item.gl_inventory.clone(),
                        debit: Some(return_cost),
                        credit: None,
                        currency: original.currency.clone(),
                        fx_rate: Some(original.fx_rate),
                        description: Some(format!(
                            "Inventory return: {} × {} @ {}",
                            line.description, line.quantity, original_cost
                        )),
                        dimensions: None,
                    });

                    // CR COGS (reverse cost of goods sold)
                    journal_lines.push(CreateJournalLineRequest {
                        account_code: inv_item.gl_cogs.clone(),
                        debit: None,
                        credit: Some(return_cost),
                        currency: original.currency.clone(),
                        fx_rate: Some(original.fx_rate),
                        description: Some(format!(
                            "COGS reversal: {} × {} @ {}",
                            line.description, line.quantity, original_cost
                        )),
                        dimensions: None,
                    });
                }
            }
        }
    }

    // Post the journal entry
    let entry_req = CreateJournalEntryRequest {
        date: cn_date,
        source: JournalSource::CreditNote,
        source_id: Some(cn_id),
        reference: cn_number.clone(),
        description: format!("Credit note {} against invoice {}", cn_number, original.number),
        lines: journal_lines,
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

    // Update the journal_entry_id on credit note
    sqlx::query("UPDATE invoices SET journal_entry_id = $1 WHERE id = $2")
        .bind(entry.id)
        .bind(cn_id)
        .execute(&mut *tx)
        .await?;

    // Reduce balance_due on original invoice
    let new_balance = original.balance_due - gross_total;
    let new_amount_paid = original.amount_paid + gross_total;
    let new_status = if new_balance <= Decimal::ZERO {
        "paid"
    } else if new_amount_paid > Decimal::ZERO {
        "partially_paid"
    } else {
        &original.status
    };

    sqlx::query(
        "UPDATE invoices SET balance_due = $1, amount_paid = $2, status = $3 WHERE id = $4",
    )
    .bind(new_balance)
    .bind(new_amount_paid)
    .bind(new_status)
    .bind(req.invoice_id)
    .execute(&mut *tx)
    .await?;

    tx.commit().await?;

    Ok(CreditNoteResult {
        credit_note_id: cn_id,
        credit_note_number: cn_number,
        amount: gross_total,
        journal_entry_id: entry.id,
        original_new_balance: new_balance,
    })
}

/// Create an estimate (quote).
pub async fn create_estimate(
    engine: &ErpEngine,
    entity_id: Uuid,
    req: CreateEstimateRequest,
    _created_by: &AgentOrUserId,
) -> ErpResult<Uuid> {
    let today = Utc::now().date_naive();
    let id = Uuid::new_v4();

    // Look up customer
    let customer = sqlx::query_as::<_, crate::parties::CustomerRow>(
        "SELECT * FROM customers WHERE id = $1 AND entity_id = $2",
    )
    .bind(req.customer_id)
    .bind(entity_id)
    .fetch_optional(engine.pool())
    .await?
    .ok_or_else(|| ErpError::NotFound {
        entity_type: "Customer".to_string(),
        id: req.customer_id,
    })?;

    let currency = req.currency.unwrap_or(customer.currency.clone());
    let issue_date = req.issue_date.unwrap_or(today);
    let expiry_date = req.expiry_date.unwrap_or(issue_date + chrono::Duration::days(30));

    // Resolve lines
    let mut lines = Vec::new();
    for line_req in &req.lines {
        let mut line = resolve_invoice_line(engine, entity_id, line_req).await?;
        line.compute_totals();
        lines.push(line);
    }

    let subtotal: Decimal = lines.iter().map(|l| l.line_total).sum();
    let tax_total: Decimal = lines.iter().map(|l| l.vat_amount).sum();
    let gross_total = crate::money::round_money(subtotal + tax_total);

    // Generate estimate number
    let number = generate_estimate_number(engine, entity_id).await?;

    // Insert estimate header + lines atomically.
    let mut tx = engine.pool().begin().await?;

    sqlx::query(
        r#"INSERT INTO estimates 
           (id, entity_id, number, customer_id, issue_date, expiry_date, currency, fx_rate,
            subtotal, tax_total, gross_total, status, notes, template_id, created_at)
           VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15)"#,
    )
    .bind(id)
    .bind(entity_id)
    .bind(&number)
    .bind(req.customer_id)
    .bind(issue_date)
    .bind(expiry_date)
    .bind(&currency)
    .bind(Decimal::ONE)
    .bind(subtotal)
    .bind(tax_total)
    .bind(gross_total)
    .bind("draft")
    .bind(&req.notes)
    .bind(req.template_id)
    .bind(Utc::now())
    .execute(&mut *tx)
    .await?;

    // Insert estimate lines (reusing invoice_lines table pattern)
    for line in &lines {
        sqlx::query(
            r#"INSERT INTO estimate_lines 
               (id, estimate_id, product_id, description, quantity, unit_price, discount_percent, account_code, vat_treatment, line_total, vat_amount)
               VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)"#,
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

    Ok(id)
}

/// Update a draft estimate's header and lines in place.
///
/// The caller is responsible for enforcing that the estimate is in `draft`
/// status (so the right HTTP status can be returned); this re-validates that
/// guard defensively and replaces the line set atomically.
pub async fn update_estimate_draft(
    engine: &ErpEngine,
    entity_id: Uuid,
    id: Uuid,
    req: CreateEstimateRequest,
) -> ErpResult<()> {
    let today = Utc::now().date_naive();

    let status: Option<String> = sqlx::query_scalar(
        "SELECT status FROM estimates WHERE id = $1 AND entity_id = $2",
    )
    .bind(id)
    .bind(entity_id)
    .fetch_optional(engine.pool())
    .await?;
    let status = status.ok_or_else(|| ErpError::NotFound { entity_type: "Estimate".to_string(), id })?;
    if status != "draft" {
        return Err(ErpError::ValidationFailed {
            message: format!("Only draft estimates can be edited (current status: {status})"),
        });
    }

    let customer = sqlx::query_as::<_, crate::parties::CustomerRow>(
        "SELECT * FROM customers WHERE id = $1 AND entity_id = $2",
    )
    .bind(req.customer_id)
    .bind(entity_id)
    .fetch_optional(engine.pool())
    .await?
    .ok_or_else(|| ErpError::NotFound { entity_type: "Customer".to_string(), id: req.customer_id })?;

    let currency = req.currency.unwrap_or(customer.currency.clone());
    let issue_date = req.issue_date.unwrap_or(today);
    let expiry_date = req.expiry_date.unwrap_or(issue_date + chrono::Duration::days(30));

    let mut lines = Vec::new();
    for line_req in &req.lines {
        let mut line = resolve_invoice_line(engine, entity_id, line_req).await?;
        line.compute_totals();
        lines.push(line);
    }
    let subtotal: Decimal = lines.iter().map(|l| l.line_total).sum();
    let tax_total: Decimal = lines.iter().map(|l| l.vat_amount).sum();
    let gross_total = crate::money::round_money(subtotal + tax_total);

    let mut tx = engine.pool().begin().await?;

    sqlx::query(
        r#"UPDATE estimates SET
              customer_id = $1, issue_date = $2, expiry_date = $3, currency = $4,
              subtotal = $5, tax_total = $6, gross_total = $7, notes = $8
           WHERE id = $9 AND entity_id = $10"#,
    )
    .bind(req.customer_id)
    .bind(issue_date)
    .bind(expiry_date)
    .bind(&currency)
    .bind(subtotal)
    .bind(tax_total)
    .bind(gross_total)
    .bind(&req.notes)
    .bind(id)
    .bind(entity_id)
    .execute(&mut *tx)
    .await?;

    sqlx::query("DELETE FROM estimate_lines WHERE estimate_id = $1")
        .bind(id)
        .execute(&mut *tx)
        .await?;

    for line in &lines {
        sqlx::query(
            r#"INSERT INTO estimate_lines
               (id, estimate_id, product_id, description, quantity, unit_price, discount_percent, account_code, vat_treatment, line_total, vat_amount)
               VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)"#,
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
        .execute(&mut *tx)
        .await?;
    }

    tx.commit().await?;
    Ok(())
}

/// Delete a draft estimate and its lines.
pub async fn delete_estimate_draft(engine: &ErpEngine, entity_id: Uuid, id: Uuid) -> ErpResult<()> {
    let status: Option<String> = sqlx::query_scalar(
        "SELECT status FROM estimates WHERE id = $1 AND entity_id = $2",
    )
    .bind(id)
    .bind(entity_id)
    .fetch_optional(engine.pool())
    .await?;
    let status = status.ok_or_else(|| ErpError::NotFound { entity_type: "Estimate".to_string(), id })?;
    if status != "draft" {
        return Err(ErpError::ValidationFailed {
            message: format!("Only draft estimates can be deleted (current status: {status})"),
        });
    }

    let mut tx = engine.pool().begin().await?;
    sqlx::query("DELETE FROM estimate_lines WHERE estimate_id = $1")
        .bind(id)
        .execute(&mut *tx)
        .await?;
    sqlx::query("DELETE FROM estimates WHERE id = $1 AND entity_id = $2")
        .bind(id)
        .bind(entity_id)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    Ok(())
}

/// Convert an accepted estimate into an invoice.
///
/// Copies all lines from the estimate, creates a new invoice, and marks
/// the estimate as converted with a link to the new invoice.
pub async fn convert_estimate_to_invoice(
    engine: &ErpEngine,
    entity_id: Uuid,
    estimate_id: Uuid,
    created_by: &AgentOrUserId,
) -> ErpResult<Uuid> {
    // Fetch the estimate
    let estimate = sqlx::query_as::<_, EstimateRow>(
        "SELECT * FROM estimates WHERE id = $1 AND entity_id = $2",
    )
    .bind(estimate_id)
    .bind(entity_id)
    .fetch_optional(engine.pool())
    .await?
    .ok_or_else(|| ErpError::NotFound {
        entity_type: "Estimate".to_string(),
        id: estimate_id,
    })?;

    // Validate status — must be draft, sent, or accepted (not converted/declined/expired)
    match estimate.status.as_str() {
        "converted" => {
            return Err(ErpError::ValidationFailed {
                message: "Estimate has already been converted to an invoice".to_string(),
            });
        }
        "declined" => {
            return Err(ErpError::ValidationFailed {
                message: "Cannot convert a declined estimate".to_string(),
            });
        }
        "expired" => {
            return Err(ErpError::ValidationFailed {
                message: "Cannot convert an expired estimate".to_string(),
            });
        }
        _ => {} // draft, sent, accepted — all OK
    }

    // Fetch estimate lines. `estimate_lines.estimate_id` is aliased to `invoice_id`
    // so the row maps onto the shared `InvoiceLineRow` struct.
    let est_lines = sqlx::query_as::<_, InvoiceLineRow>(
        r#"SELECT id, estimate_id AS invoice_id, product_id, description, quantity,
                  unit_price, discount_percent, account_code, vat_treatment, line_total, vat_amount
           FROM estimate_lines WHERE estimate_id = $1"#,
    )
    .bind(estimate_id)
    .fetch_all(engine.pool())
    .await?;

    // Build CreateInvoiceRequest from estimate data
    let invoice_lines: Vec<CreateInvoiceLineRequest> = est_lines
        .iter()
        .map(|l| CreateInvoiceLineRequest {
            product_id: l.product_id,
            description: Some(l.description.clone()),
            quantity: l.quantity,
            unit_price: Some(l.unit_price),
            discount_percent: Some(l.discount_percent),
            account_code: Some(l.account_code.clone()),
            vat_treatment: serde_json::from_str(&l.vat_treatment).ok(),
            dimensions: serde_json::from_value(l.dimensions.clone()).ok(),
        })
        .collect();

    let inv_req = CreateInvoiceRequest {
        customer_id: estimate.customer_id,
        issue_date: None, // use today
        due_date: None,   // use customer payment terms
        currency: Some(estimate.currency),
        fx_rate: Some(estimate.fx_rate),
        lines: invoice_lines,
        template_id: estimate.template_id,
        notes: estimate.notes,
        send_immediately: None,
    };

    // Create the invoice
    let invoice = create_invoice(engine, entity_id, inv_req, created_by).await?;

    // Link invoice back to estimate via source_estimate
    sqlx::query("UPDATE invoices SET source_estimate = $1 WHERE id = $2")
        .bind(estimate_id)
        .bind(invoice.id)
        .execute(engine.pool())
        .await?;

    // Mark estimate as converted
    sqlx::query("UPDATE estimates SET status = 'converted', converted_to = $1 WHERE id = $2")
        .bind(invoice.id)
        .bind(estimate_id)
        .execute(engine.pool())
        .await?;

    Ok(invoice.id)
}

/// Resolve an invoice line from a request — auto-fills from product catalog.
pub(crate) async fn resolve_invoice_line(
    engine: &ErpEngine,
    entity_id: Uuid,
    req: &CreateInvoiceLineRequest,
) -> ErpResult<InvoiceLine> {
    let id = Uuid::new_v4();

    if let Some(product_id) = req.product_id {
        // Look up product for defaults
        let product = sqlx::query_as::<_, crate::catalog::ProductRow>(
            "SELECT * FROM products WHERE id = $1 AND entity_id = $2",
        )
        .bind(product_id)
        .bind(entity_id)
        .fetch_optional(engine.pool())
        .await?
        .ok_or_else(|| ErpError::NotFound {
            entity_type: "Product".to_string(),
            id: product_id,
        })?;

        let vat_treatment: crate::types::VatTreatment = req.vat_treatment.clone().unwrap_or_else(|| {
            serde_json::from_str(&format!("\"{}\"", product.vat_treatment))
                .unwrap_or(crate::types::VatTreatment::Standard16)
        });

        Ok(InvoiceLine {
            id,
            product_id: Some(product_id),
            description: req.description.clone().unwrap_or(product.name),
            quantity: req.quantity,
            unit_price: req.unit_price.unwrap_or(product.unit_price.unwrap_or(Decimal::ZERO)),
            discount_percent: req.discount_percent.unwrap_or(Decimal::ZERO),
            account_code: req.account_code.clone().unwrap_or(product.sales_account),
            vat_treatment,
            line_total: Decimal::ZERO,
            vat_amount: Decimal::ZERO,
            dimensions: req.dimensions.clone().unwrap_or_default(),
        })
    } else {
        // Manual line — all fields required
        Ok(InvoiceLine {
            id,
            product_id: None,
            description: req.description.clone().unwrap_or_default(),
            quantity: req.quantity,
            unit_price: req.unit_price.unwrap_or(Decimal::ZERO),
            discount_percent: req.discount_percent.unwrap_or(Decimal::ZERO),
            account_code: match req.account_code.clone() {
                Some(c) => c,
                None => engine.posting_for(entity_id).await?.default_sales.clone(),
            },
            vat_treatment: req.vat_treatment.clone().unwrap_or(crate::types::VatTreatment::Standard16),
            line_total: Decimal::ZERO,
            vat_amount: Decimal::ZERO,
            dimensions: req.dimensions.clone().unwrap_or_default(),
        })
    }
}

/// Generate the next invoice number atomically.
async fn generate_invoice_number(engine: &ErpEngine, entity_id: Uuid) -> ErpResult<String> {
    let row = sqlx::query_scalar::<_, i64>(
        r#"UPDATE entity_settings 
           SET sequences = jsonb_set(sequences, '{invoice_next}', to_jsonb((sequences->>'invoice_next')::bigint + 1))
           WHERE entity_id = $1
           RETURNING (sequences->>'invoice_next')::bigint - 1"#,
    )
    .bind(entity_id)
    .fetch_one(engine.pool())
    .await?;

    let cfg = engine.config_for(entity_id).await?;
    let prefix = &cfg.sequences.invoice_prefix;
    let fiscal_year = Utc::now().format("%Y").to_string();

    Ok(format!("{}-{}-{:04}", prefix, fiscal_year, row))
}

/// Generate the next credit note number atomically.
async fn generate_credit_note_number(engine: &ErpEngine, entity_id: Uuid) -> ErpResult<String> {
    let row = sqlx::query_scalar::<_, i64>(
        r#"UPDATE entity_settings 
           SET sequences = jsonb_set(sequences, '{credit_note_next}', to_jsonb((sequences->>'credit_note_next')::bigint + 1))
           WHERE entity_id = $1
           RETURNING (sequences->>'credit_note_next')::bigint - 1"#,
    )
    .bind(entity_id)
    .fetch_one(engine.pool())
    .await?;

    let cfg = engine.config_for(entity_id).await?;
    let prefix = &cfg.sequences.credit_note_prefix;
    let fiscal_year = Utc::now().format("%Y").to_string();

    Ok(format!("{}-{}-{:04}", prefix, fiscal_year, row))
}

/// Generate the next estimate number atomically.
async fn generate_estimate_number(engine: &ErpEngine, entity_id: Uuid) -> ErpResult<String> {
    let row = sqlx::query_scalar::<_, i64>(
        r#"UPDATE entity_settings 
           SET sequences = jsonb_set(sequences, '{estimate_next}', to_jsonb((sequences->>'estimate_next')::bigint + 1))
           WHERE entity_id = $1
           RETURNING (sequences->>'estimate_next')::bigint - 1"#,
    )
    .bind(entity_id)
    .fetch_one(engine.pool())
    .await?;

    let cfg = engine.config_for(entity_id).await?;
    let prefix = &cfg.sequences.estimate_prefix;
    let fiscal_year = Utc::now().format("%Y").to_string();

    Ok(format!("{}-{}-{:04}", prefix, fiscal_year, row))
}

/// Edit a **draft** invoice — replaces its lines and recomputes totals.
///
/// Only permitted while the invoice is a draft (not yet posted to the ledger);
/// posted invoices are immutable and must be corrected with a void/credit note.
pub async fn update_invoice_draft(
    engine: &ErpEngine,
    entity_id: Uuid,
    invoice_id: Uuid,
    req: CreateInvoiceRequest,
) -> ErpResult<()> {
    let invoice = sqlx::query_as::<_, InvoiceRow>(
        "SELECT * FROM invoices WHERE id = $1 AND entity_id = $2",
    )
    .bind(invoice_id)
    .bind(entity_id)
    .fetch_optional(engine.pool())
    .await?
    .ok_or_else(|| ErpError::NotFound { entity_type: "Invoice".to_string(), id: invoice_id })?;

    if invoice.status != "draft" {
        return Err(ErpError::ValidationFailed {
            message: format!(
                "Only draft invoices can be edited; invoice {} is '{}'. Post then void/credit to correct it.",
                invoice.number, invoice.status
            ),
        });
    }

    let today = Utc::now().date_naive();
    let customer = sqlx::query_as::<_, crate::parties::CustomerRow>(
        "SELECT * FROM customers WHERE id = $1 AND entity_id = $2",
    )
    .bind(req.customer_id)
    .bind(entity_id)
    .fetch_optional(engine.pool())
    .await?
    .ok_or_else(|| ErpError::NotFound { entity_type: "Customer".to_string(), id: req.customer_id })?;

    let currency = req.currency.clone().unwrap_or_else(|| customer.currency.clone());
    let issue_date = req.issue_date.unwrap_or(today);
    let payment_terms: crate::types::PaymentTerms =
        serde_json::from_str(&format!("\"{}\"", customer.payment_terms))
            .unwrap_or(crate::types::PaymentTerms::Net30);
    let due_date = req.due_date.unwrap_or_else(|| payment_terms.due_date(issue_date));

    let mut lines = Vec::new();
    for line_req in &req.lines {
        let mut line = resolve_invoice_line(engine, entity_id, line_req).await?;
        line.compute_totals();
        lines.push(line);
    }
    let subtotal: Decimal = lines.iter().map(|l| l.line_total).sum();
    let tax_total: Decimal = lines.iter().map(|l| l.vat_amount).sum();
    let discount_total: Decimal = lines
        .iter()
        .map(|l| (l.quantity * l.unit_price) * l.discount_percent / Decimal::new(100, 0))
        .sum();
    let gross_total = subtotal + tax_total;

    let mut tx = engine.pool().begin().await?;
    sqlx::query(
        r#"UPDATE invoices
           SET customer_id = $1, issue_date = $2, due_date = $3, currency = $4, fx_rate = $5,
               subtotal = $6, discount_total = $7, tax_total = $8, gross_total = $9,
               balance_due = $9, notes = $10
           WHERE id = $11"#,
    )
    .bind(req.customer_id)
    .bind(issue_date)
    .bind(due_date)
    .bind(&currency)
    .bind(req.fx_rate.unwrap_or(Decimal::ONE))
    .bind(subtotal)
    .bind(discount_total)
    .bind(tax_total)
    .bind(gross_total)
    .bind(&req.notes)
    .bind(invoice_id)
    .execute(&mut *tx)
    .await?;

    sqlx::query("DELETE FROM invoice_lines WHERE invoice_id = $1")
        .bind(invoice_id)
        .execute(&mut *tx)
        .await?;
    for line in &lines {
        sqlx::query(
            r#"INSERT INTO invoice_lines
               (id, invoice_id, product_id, description, quantity, unit_price, discount_percent, account_code, vat_treatment, line_total, vat_amount, dimensions)
               VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12)"#,
        )
        .bind(line.id)
        .bind(invoice_id)
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
    Ok(())
}

/// Delete a **draft** invoice and its line items. Only drafts can be deleted;
/// posted invoices must be voided/credited so the ledger stays intact.
pub async fn delete_invoice_draft(
    engine: &ErpEngine,
    entity_id: Uuid,
    invoice_id: Uuid,
) -> ErpResult<()> {
    let row = sqlx::query_as::<_, (String, String)>(
        "SELECT number, status FROM invoices WHERE id = $1 AND entity_id = $2",
    )
    .bind(invoice_id)
    .bind(entity_id)
    .fetch_optional(engine.pool())
    .await?
    .ok_or_else(|| ErpError::NotFound { entity_type: "Invoice".to_string(), id: invoice_id })?;

    if row.1 != "draft" {
        return Err(ErpError::ValidationFailed {
            message: format!(
                "Only draft invoices can be deleted; invoice {} is '{}'. Void it instead.",
                row.0, row.1
            ),
        });
    }

    let mut tx = engine.pool().begin().await?;
    sqlx::query("DELETE FROM invoice_lines WHERE invoice_id = $1")
        .bind(invoice_id)
        .execute(&mut *tx)
        .await?;
    sqlx::query("DELETE FROM invoices WHERE id = $1 AND entity_id = $2")
        .bind(invoice_id)
        .bind(entity_id)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    Ok(())
}

/// Mark a posted invoice as sent (records `sent_at`). Delivery is decoupled from
/// posting, so this just stamps when the invoice was sent — including off-system
/// (printed, emailed manually, etc.). Drafts must be posted first.
pub async fn mark_invoice_sent(
    engine: &ErpEngine,
    entity_id: Uuid,
    invoice_id: Uuid,
) -> ErpResult<()> {
    let status = sqlx::query_scalar::<_, String>(
        "SELECT status FROM invoices WHERE id = $1 AND entity_id = $2",
    )
    .bind(invoice_id)
    .bind(entity_id)
    .fetch_optional(engine.pool())
    .await?
    .ok_or_else(|| ErpError::NotFound { entity_type: "Invoice".to_string(), id: invoice_id })?;

    if status == "draft" {
        return Err(ErpError::ValidationFailed {
            message: "Post the invoice before marking it as sent".to_string(),
        });
    }

    sqlx::query("UPDATE invoices SET sent_at = NOW() WHERE id = $1 AND entity_id = $2")
        .bind(invoice_id)
        .bind(entity_id)
        .execute(engine.pool())
        .await?;
    Ok(())
}

/// Send an invoice to its customer by email — a formatted HTML message with the
/// invoice PDF attached — and stamp `sent_at`. The PDF is rendered with the
/// chosen template (request `template_id` → invoice's template → entity default
/// → built-in). If `mark_sent_only` is set, or no recipient email is available,
/// it falls back to just stamping `sent_at` (off-system send). Email delivery is
/// queued via the notification system; if SMTP is unconfigured the worker
/// no-ops, so this never blocks marking the invoice sent.
///
/// Returns the recipient the email was queued to, if any.
pub async fn send_invoice(
    engine: &ErpEngine,
    entity_id: Uuid,
    req: crate::invoicing::SendInvoiceRequest,
) -> ErpResult<Option<String>> {
    let invoice_id = req.invoice_id;
    let invoice = sqlx::query_as::<_, InvoiceRow>(
        "SELECT * FROM invoices WHERE id = $1 AND entity_id = $2",
    )
    .bind(invoice_id)
    .bind(entity_id)
    .fetch_optional(engine.pool())
    .await?
    .ok_or_else(|| ErpError::NotFound { entity_type: "Invoice".to_string(), id: invoice_id })?;

    if invoice.status == "draft" {
        return Err(ErpError::ValidationFailed {
            message: "Post the invoice before sending it".to_string(),
        });
    }

    // Mark-sent-only short circuit (off-system delivery).
    if req.mark_sent_only {
        mark_invoice_sent(engine, entity_id, invoice_id).await?;
        return Ok(None);
    }

    // Resolve the customer + recipient email.
    let customer = sqlx::query_as::<_, crate::parties::CustomerRow>(
        "SELECT * FROM customers WHERE id = $1 AND entity_id = $2",
    )
    .bind(invoice.customer_id)
    .bind(entity_id)
    .fetch_optional(engine.pool())
    .await?
    .ok_or_else(|| ErpError::NotFound { entity_type: "Customer".to_string(), id: invoice.customer_id })?;

    let recipient = req.recipient_email.clone().or_else(|| {
        serde_json::from_value::<Vec<crate::types::ContactEmail>>(customer.email.clone())
            .ok()
            .and_then(|emails| emails.into_iter().map(|e| e.email).find(|e| !e.is_empty()))
    });

    // No email anywhere → mark sent only (still records the action).
    let Some(recipient) = recipient else {
        mark_invoice_sent(engine, entity_id, invoice_id).await?;
        return Ok(None);
    };

    // Resolve the template: request → invoice → entity default → None (built-in).
    let template_id = req.template_id.or(invoice.template_id);
    let template = load_template(engine, entity_id, template_id).await?;

    // Org name + branding from settings (best-effort).
    let org_name = sqlx::query_scalar::<_, Option<String>>(
        "SELECT organization_name FROM entity_settings WHERE entity_id = $1",
    )
    .bind(entity_id)
    .fetch_optional(engine.pool())
    .await?
    .flatten()
    .unwrap_or_else(|| "Your Company".to_string());

    // Build the PDF.
    let lines = sqlx::query_as::<_, InvoiceLineRow>(
        "SELECT * FROM invoice_lines WHERE invoice_id = $1",
    )
    .bind(invoice_id)
    .fetch_all(engine.pool())
    .await?;

    let (accent_hex, footer_text) = match &template {
        Some(t) => (t.primary_color.clone(), t.footer_text.clone()),
        None => ("#1a56db".to_string(), None),
    };

    let pdf_data = crate::invoicing::pdf::InvoicePdfData {
        org_name: org_name.clone(),
        invoice_number: invoice.number.clone(),
        invoice_type_label: if invoice.invoice_type == "credit_note" { "Credit Note".to_string() } else { "Tax Invoice".to_string() },
        issue_date: invoice.issue_date.to_string(),
        due_date: invoice.due_date.to_string(),
        currency: invoice.currency.clone(),
        customer_name: customer.name.clone(),
        customer_email: Some(recipient.clone()),
        lines: lines.iter().map(|l| crate::invoicing::pdf::InvoicePdfLine {
            description: l.description.clone(),
            quantity: l.quantity,
            unit_price: l.unit_price,
            line_total: l.line_total,
        }).collect(),
        subtotal: invoice.subtotal,
        discount_total: invoice.discount_total,
        tax_total: invoice.tax_total,
        gross_total: invoice.gross_total,
        amount_paid: invoice.amount_paid,
        balance_due: invoice.balance_due,
        notes: invoice.notes.clone(),
        footer_text,
        accent_rgb: crate::invoicing::pdf::parse_hex_color(&accent_hex),
    };
    let pdf_bytes = crate::invoicing::pdf::render_invoice_pdf(&pdf_data);
    let pdf_b64 = {
        use base64::{engine::general_purpose::STANDARD as B64, Engine as _};
        B64.encode(&pdf_bytes)
    };

    // Formatted HTML email body.
    let body = render_invoice_email_html(&org_name, &invoice, &customer.name, req.message.as_deref(), &accent_hex);
    let subject = format!("Invoice {} from {}", invoice.number, org_name);

    let notif = crate::notifications::SendNotificationRequest {
        event_type: crate::notifications::NotificationEventType::InvoiceSent,
        channels: vec![crate::types::Channel::Email],
        recipients: vec![recipient.clone()],
        subject: Some(subject),
        body,
        related_type: Some("Invoice".to_string()),
        related_id: Some(invoice_id),
        schedule_at: None,
        attachments: vec![crate::notifications::NotificationAttachment {
            filename: format!("{}.pdf", invoice.number),
            mime_type: "application/pdf".to_string(),
            content_base64: pdf_b64,
        }],
    };
    crate::services::notifications::send_notification(engine, entity_id, notif).await?;

    // Stamp sent.
    mark_invoice_sent(engine, entity_id, invoice_id).await?;

    Ok(Some(recipient))
}

/// Load an invoice template by id, or the entity's default, or None.
async fn load_template(
    engine: &ErpEngine,
    entity_id: Uuid,
    template_id: Option<Uuid>,
) -> ErpResult<Option<crate::invoicing::InvoiceTemplateRow>> {
    if let Some(tid) = template_id {
        let row = sqlx::query_as::<_, crate::invoicing::InvoiceTemplateRow>(
            "SELECT * FROM invoice_templates WHERE id = $1 AND entity_id = $2",
        )
        .bind(tid)
        .bind(entity_id)
        .fetch_optional(engine.pool())
        .await?;
        if row.is_some() {
            return Ok(row);
        }
    }
    // Fall back to the entity default template.
    let row = sqlx::query_as::<_, crate::invoicing::InvoiceTemplateRow>(
        "SELECT * FROM invoice_templates WHERE entity_id = $1 AND is_default = true LIMIT 1",
    )
    .bind(entity_id)
    .fetch_optional(engine.pool())
    .await?;
    Ok(row)
}

/// Render a clean, branded HTML email body for an invoice.
fn render_invoice_email_html(
    org_name: &str,
    invoice: &InvoiceRow,
    customer_name: &str,
    message: Option<&str>,
    accent_hex: &str,
) -> String {
    let intro = message
        .filter(|m| !m.trim().is_empty())
        .map(|m| format!("<p style=\"margin:0 0 16px;color:#374151\">{}</p>", html_escape(m)))
        .unwrap_or_default();
    format!(
        r#"<!DOCTYPE html><html><body style="margin:0;background:#f3f4f6;font-family:Arial,Helvetica,sans-serif">
  <div style="max-width:600px;margin:0 auto;padding:24px">
    <div style="background:#fff;border-radius:12px;overflow:hidden;border:1px solid #e5e7eb">
      <div style="background:{accent};padding:20px 24px">
        <h1 style="margin:0;color:#fff;font-size:18px">{org}</h1>
      </div>
      <div style="padding:24px">
        <p style="margin:0 0 16px;color:#111827">Hi {customer},</p>
        {intro}
        <p style="margin:0 0 16px;color:#374151">
          Please find attached invoice <strong>{number}</strong>.
        </p>
        <table style="width:100%;border-collapse:collapse;margin:8px 0 20px">
          <tr><td style="padding:6px 0;color:#6b7280">Invoice</td><td style="padding:6px 0;text-align:right;color:#111827">{number}</td></tr>
          <tr><td style="padding:6px 0;color:#6b7280">Issue date</td><td style="padding:6px 0;text-align:right;color:#111827">{issue}</td></tr>
          <tr><td style="padding:6px 0;color:#6b7280">Due date</td><td style="padding:6px 0;text-align:right;color:#111827">{due}</td></tr>
          <tr><td style="padding:10px 0;border-top:1px solid #e5e7eb;color:#111827;font-weight:bold">Amount due</td>
              <td style="padding:10px 0;border-top:1px solid #e5e7eb;text-align:right;color:{accent};font-weight:bold;font-size:18px">{currency} {balance}</td></tr>
        </table>
        <p style="margin:0;color:#9ca3af;font-size:12px">Thank you for your business.</p>
      </div>
    </div>
    <p style="text-align:center;color:#9ca3af;font-size:11px;margin:16px 0 0">Sent by {org} via Zavora ERP</p>
  </div>
</body></html>"#,
        accent = html_escape(accent_hex),
        org = html_escape(org_name),
        customer = html_escape(customer_name),
        intro = intro,
        number = html_escape(&invoice.number),
        issue = invoice.issue_date,
        due = invoice.due_date,
        currency = html_escape(&invoice.currency),
        balance = invoice.balance_due,
    )
}

fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

/// Post an invoice — creates the GL journal entry (DR AR / CR Revenue / CR VAT Output).
/// For line items with tracked inventory products, also issues stock and posts COGS
/// (DR COGS / CR Inventory at weighted average cost). Rejects with InsufficientStock
/// if any tracked item lacks adequate available quantity.
pub async fn post_invoice(
    engine: &ErpEngine,
    entity_id: Uuid,
    invoice_id: Uuid,
    posted_by: &AgentOrUserId,
) -> ErpResult<Uuid> {
    let invoice = sqlx::query_as::<_, InvoiceRow>(
        "SELECT * FROM invoices WHERE id = $1 AND entity_id = $2",
    )
    .bind(invoice_id)
    .bind(entity_id)
    .fetch_optional(engine.pool())
    .await?
    .ok_or_else(|| ErpError::NotFound {
        entity_type: "Invoice".to_string(),
        id: invoice_id,
    })?;

    if invoice.status != "draft" {
        return Err(ErpError::ValidationFailed {
            message: format!(
                "Only draft invoices can be posted; invoice {} is '{}'",
                invoice.number, invoice.status
            ),
        });
    }

    // === Credit limit check (Requirements 20.4, 20.5) ===
    // Look up the customer and check if they have a credit limit set
    let customer = sqlx::query_as::<_, crate::parties::CustomerRow>(
        "SELECT * FROM customers WHERE id = $1 AND entity_id = $2",
    )
    .bind(invoice.customer_id)
    .bind(entity_id)
    .fetch_optional(engine.pool())
    .await?
    .ok_or_else(|| ErpError::NotFound {
        entity_type: "Customer".to_string(),
        id: invoice.customer_id,
    })?;

    if let Some(credit_limit) = customer.credit_limit {
        // Query total outstanding AR balance (sum of balance_due where status NOT IN (paid, voided))
        let outstanding: Decimal = sqlx::query_scalar(
            r#"SELECT COALESCE(SUM(balance_due), 0) 
               FROM invoices 
               WHERE customer_id = $1 
                 AND entity_id = $2 
                 AND invoice_type = 'invoice'
                 AND status NOT IN ('paid', 'voided')
                 AND id != $3"#,
        )
        .bind(invoice.customer_id)
        .bind(entity_id)
        .bind(invoice_id)
        .fetch_one(engine.pool())
        .await?;

        if outstanding + invoice.gross_total > credit_limit {
            // Send notification to Admin users via In-App and Email channels
            let notification_req = crate::notifications::SendNotificationRequest {
                event_type: crate::notifications::NotificationEventType::CreditLimitExceeded,
                channels: vec![
                    crate::types::Channel::InApp,
                    crate::types::Channel::Email,
                ],
                recipients: vec!["role:Admin".to_string()],
                subject: Some(format!(
                    "Credit limit exceeded for customer '{}'",
                    customer.name
                )),
                body: format!(
                    "Invoice {} (amount {}) would cause customer '{}' to exceed their credit limit of {}. \
                     Current outstanding: {}. Total if posted: {}.",
                    invoice.number,
                    invoice.gross_total,
                    customer.name,
                    credit_limit,
                    outstanding,
                    outstanding + invoice.gross_total,
                ),
                related_type: Some("Invoice".to_string()),
                related_id: Some(invoice_id),
                schedule_at: None,
                attachments: Vec::new(),
            };

            // Best-effort notification — don't fail the entire operation if notification fails
            let _ = crate::services::notifications::send_notification(engine, entity_id, notification_req).await;

            return Err(ErpError::CreditLimitExceeded {
                customer_name: customer.name,
                customer_id: invoice.customer_id,
                outstanding,
                invoice_total: invoice.gross_total,
                credit_limit,
            });
        }
    }

    // Build journal entry lines
    let lines = sqlx::query_as::<_, InvoiceLineRow>(
        "SELECT * FROM invoice_lines WHERE invoice_id = $1",
    )
    .bind(invoice_id)
    .fetch_all(engine.pool())
    .await?;

    // Stock issue, the journal entry, and the invoice status update all commit
    // or roll back together (Requirement 2.2).
    let mut tx = engine.pool().begin().await?;

    let mut journal_lines = Vec::new();

    // DR Accounts Receivable (total including tax)
    journal_lines.push(CreateJournalLineRequest {
        account_code: engine.posting_for(entity_id).await?.accounts_receivable.clone(),
        debit: Some(invoice.gross_total),
        credit: None,
        currency: invoice.currency.clone(),
        fx_rate: Some(invoice.fx_rate),
        description: Some(format!("Invoice {} - {}", invoice.number, "AR")),
        dimensions: None,
    });

    // CR Revenue (per line). A negative line total represents a discount or
    // contra-revenue line; book it as a positive DEBIT to the revenue account
    // rather than a negative credit, so no journal line ever carries a negative
    // amount (which would violate journal validation) while the entry still
    // balances against AR.
    for line in &lines {
        let (debit, credit) = if line.line_total < Decimal::ZERO {
            (Some(-line.line_total), None)
        } else {
            (None, Some(line.line_total))
        };
        journal_lines.push(CreateJournalLineRequest {
            account_code: line.account_code.clone(),
            debit,
            credit,
            currency: invoice.currency.clone(),
            fx_rate: Some(invoice.fx_rate),
            description: Some(line.description.clone()),
            dimensions: serde_json::from_value(line.dimensions.clone()).ok(),
        });

        // CR VAT Output (if applicable)
        if line.vat_amount > Decimal::ZERO {
            journal_lines.push(CreateJournalLineRequest {
                account_code: engine.posting_for(entity_id).await?.vat_output.clone(),
                debit: None,
                credit: Some(line.vat_amount),
                currency: invoice.currency.clone(),
                fx_rate: Some(invoice.fx_rate),
                description: Some(format!("VAT on {}", line.description)),
                dimensions: None,
            });
        }
    }

    // === Inventory issue logic (Requirements 23.1, 23.2, 23.3) ===
    // For each line item linked to a product with track_inventory = true,
    // issue stock and create COGS journal lines (DR COGS / CR Inventory).
    for line in &lines {
        if let Some(product_id) = line.product_id {
            let product = sqlx::query_as::<_, crate::catalog::ProductRow>(
                "SELECT * FROM products WHERE id = $1 AND entity_id = $2",
            )
            .bind(product_id)
            .bind(entity_id)
            .fetch_optional(engine.pool())
            .await?;

            if let Some(product) = product {
                if product.track_inventory {
                    // Resolve the inventory_item_id linked to this product
                    let inventory_item_id = product.inventory_item_id.ok_or_else(|| {
                        ErpError::ValidationFailed {
                            message: format!(
                                "Product '{}' has track_inventory=true but no linked inventory_item_id",
                                product.name
                            ),
                        }
                    })?;

                    // Issue inventory — returns InsufficientStock error if not enough stock
                    let issue_req = crate::inventory::IssueInventoryRequest {
                        item_id: inventory_item_id,
                        quantity: line.quantity,
                        date: Some(invoice.issue_date),
                        reference_id: Some(invoice_id),
                        warehouse_id: None,
                    };

                    let issue_result = crate::services::inventory::issue_inventory_in_tx(
                        &mut tx, entity_id, issue_req, posted_by,
                    )
                    .await?;

                    // DR COGS at computed cost (WAC)
                    journal_lines.push(CreateJournalLineRequest {
                        account_code: issue_result.gl_cogs.clone(),
                        debit: Some(issue_result.total_cost),
                        credit: None,
                        currency: invoice.currency.clone(),
                        fx_rate: Some(invoice.fx_rate),
                        description: Some(format!(
                            "COGS: {} × {} @ {}",
                            line.description, line.quantity, issue_result.unit_cost
                        )),
                        dimensions: None,
                    });

                    // CR Inventory at computed cost (WAC)
                    journal_lines.push(CreateJournalLineRequest {
                        account_code: issue_result.gl_inventory.clone(),
                        debit: None,
                        credit: Some(issue_result.total_cost),
                        currency: invoice.currency.clone(),
                        fx_rate: Some(invoice.fx_rate),
                        description: Some(format!(
                            "Inventory issued: {} × {}",
                            line.description, line.quantity
                        )),
                        dimensions: None,
                    });
                }
            }
        }
    }

    // Create and post journal entry
    let entry_req = CreateJournalEntryRequest {
        date: invoice.issue_date,
        source: JournalSource::Invoice,
        source_id: Some(invoice.id),
        reference: invoice.number.clone(),
        description: format!("Invoice {} posted", invoice.number),
        lines: journal_lines,
        post_immediately: true,
    };

    let period = crate::services::periods::period_for_date(engine, entity_id, invoice.issue_date).await?;
    let entry = crate::services::journal::create_and_post_in_tx(
        &mut tx, engine, entity_id, entry_req, period.id, posted_by.clone(),
    )
    .await?;

    // Update invoice status and link journal entry. "Posted" reflects the
    // accounting state only — delivery ("sent") is tracked separately via
    // sent_at so an invoice can be posted and sent independently (incl. off-system).
    sqlx::query(
        "UPDATE invoices SET status = 'posted', journal_entry_id = $1 WHERE id = $2",
    )
    .bind(entry.id)
    .bind(invoice_id)
    .execute(&mut *tx)
    .await?;

    tx.commit().await?;

    Ok(entry.id)
}


/// Write off an uncollectable invoice (or part of it) to a bad-debt expense
/// account: DR <expense account> / CR Accounts Receivable for the written-off
/// amount, reducing the invoice's outstanding balance. Marks the invoice
/// `written_off` once nothing remains. The expense account is caller-supplied
/// (no hardcoded bad-debt account); AR comes from the posting config.
///
/// VAT bad-debt relief is not applied here (the full outstanding gross is
/// expensed); reclaiming output VAT is a separate, conditional step.
pub async fn write_off_invoice(
    engine: &ErpEngine,
    entity_id: Uuid,
    invoice_id: Uuid,
    expense_account: String,
    amount: Option<Decimal>,
    reason: Option<String>,
    actor: AgentOrUserId,
) -> ErpResult<Uuid> {
    let mut tx = engine.pool().begin().await?;

    let invoice = sqlx::query_as::<_, InvoiceRow>("SELECT * FROM invoices WHERE id = $1 AND entity_id = $2")
        .bind(invoice_id)
        .bind(entity_id)
        .fetch_optional(&mut *tx)
        .await?
        .ok_or_else(|| ErpError::NotFound { entity_type: "Invoice".to_string(), id: invoice_id })?;

    if invoice.status == "draft" || invoice.status == "voided" || invoice.status == "written_off" {
        return Err(ErpError::ValidationFailed {
            message: format!("Invoice {} cannot be written off (status: {})", invoice.number, invoice.status),
        });
    }
    if invoice.balance_due <= Decimal::ZERO {
        return Err(ErpError::ValidationFailed {
            message: format!("Invoice {} has nothing outstanding to write off", invoice.number),
        });
    }
    let amount = amount.unwrap_or(invoice.balance_due);
    if amount <= Decimal::ZERO || amount > invoice.balance_due {
        return Err(ErpError::ValidationFailed {
            message: format!("Write-off amount must be between 0 and the outstanding {}", invoice.balance_due),
        });
    }

    let ar_account = engine.posting_for(entity_id).await?.accounts_receivable.clone();
    let today = Utc::now().date_naive();
    let lines = vec![
        CreateJournalLineRequest {
            account_code: expense_account,
            debit: Some(amount),
            credit: None,
            currency: invoice.currency.clone(),
            fx_rate: Some(invoice.fx_rate),
            description: Some(format!("Bad debt write-off {}", invoice.number)),
            dimensions: None,
        },
        CreateJournalLineRequest {
            account_code: ar_account,
            debit: None,
            credit: Some(amount),
            currency: invoice.currency.clone(),
            fx_rate: Some(invoice.fx_rate),
            description: Some(format!("Write-off {} - AR", invoice.number)),
            dimensions: None,
        },
    ];

    let entry_req = CreateJournalEntryRequest {
        date: today,
        source: JournalSource::Manual,
        source_id: Some(invoice.id),
        reference: format!("WRITEOFF-{}", invoice.number),
        description: reason
            .filter(|r| !r.trim().is_empty())
            .map(|r| format!("Bad debt write-off {}: {}", invoice.number, r))
            .unwrap_or_else(|| format!("Bad debt write-off {}", invoice.number)),
        lines,
        post_immediately: true,
    };

    let period = crate::services::periods::period_for_date(engine, entity_id, today).await?;
    let entry = crate::services::journal::create_and_post_in_tx(&mut tx, engine, entity_id, entry_req, period.id, actor).await?;

    let new_balance = invoice.balance_due - amount;
    let new_status = if new_balance <= Decimal::ZERO { "written_off" } else { &invoice.status };
    sqlx::query("UPDATE invoices SET amount_paid = amount_paid + $1, balance_due = balance_due - $1, status = $2 WHERE id = $3")
        .bind(amount)
        .bind(new_status)
        .bind(invoice_id)
        .execute(&mut *tx)
        .await?;

    tx.commit().await?;
    Ok(entry.id)
}

/// Resolve a bill line from a request — auto-fills from product catalog (purchase side).
pub async fn resolve_bill_line(
    engine: &ErpEngine,
    entity_id: Uuid,
    req: &CreateInvoiceLineRequest,
    vendor: &crate::parties::VendorRow,
) -> ErpResult<InvoiceLine> {
    let id = Uuid::new_v4();

    if let Some(product_id) = req.product_id {
        let product = sqlx::query_as::<_, crate::catalog::ProductRow>(
            "SELECT * FROM products WHERE id = $1 AND entity_id = $2",
        )
        .bind(product_id)
        .bind(entity_id)
        .fetch_optional(engine.pool())
        .await?
        .ok_or_else(|| ErpError::NotFound {
            entity_type: "Product".to_string(),
            id: product_id,
        })?;

        let vat_treatment: crate::types::VatTreatment = req.vat_treatment.clone().unwrap_or_else(|| {
            serde_json::from_str(&format!("\"{}\"", product.vat_treatment))
                .unwrap_or(crate::types::VatTreatment::Standard16)
        });

        Ok(InvoiceLine {
            id,
            product_id: Some(product_id),
            description: req.description.clone().unwrap_or(product.name),
            quantity: req.quantity,
            unit_price: req.unit_price.unwrap_or(product.unit_price.unwrap_or(Decimal::ZERO)),
            discount_percent: req.discount_percent.unwrap_or(Decimal::ZERO),
            account_code: req.account_code.clone().unwrap_or(product.purchase_account),
            vat_treatment,
            line_total: Decimal::ZERO,
            vat_amount: Decimal::ZERO,
            dimensions: req.dimensions.clone().unwrap_or_default(),
        })
    } else {
        let default_account = match vendor.default_expense_account.clone() {
            Some(a) => a,
            None => engine.posting_for(entity_id).await?.default_expense.clone(),
        };

        Ok(InvoiceLine {
            id,
            product_id: None,
            description: req.description.clone().unwrap_or_default(),
            quantity: req.quantity,
            unit_price: req.unit_price.unwrap_or(Decimal::ZERO),
            discount_percent: req.discount_percent.unwrap_or(Decimal::ZERO),
            account_code: req.account_code.clone().unwrap_or(default_account),
            vat_treatment: req.vat_treatment.clone().unwrap_or(crate::types::VatTreatment::Standard16),
            line_total: Decimal::ZERO,
            vat_amount: Decimal::ZERO,
            dimensions: req.dimensions.clone().unwrap_or_default(),
        })
    }
}

/// Mark an issued invoice as transmitted to KRA eTIMS.
///
/// In Kenya (2026) a tax invoice must be transmitted to KRA. Once transmitted it
/// becomes immutable — it can no longer be voided/deleted, only corrected with a
/// credit note. This records that transmission (the actual Daraja/eTIMS API call
/// is a separate integration; this is the state transition + reference capture).
pub async fn mark_invoice_etims_transmitted(
    engine: &ErpEngine,
    entity_id: Uuid,
    invoice_id: Uuid,
    etims_invoice_number: Option<String>,
) -> ErpResult<()> {
    let inv = sqlx::query_as::<_, InvoiceRow>(
        "SELECT * FROM invoices WHERE id = $1 AND entity_id = $2",
    )
    .bind(invoice_id)
    .bind(entity_id)
    .fetch_optional(engine.pool())
    .await?
    .ok_or_else(|| ErpError::NotFound { entity_type: "Invoice".to_string(), id: invoice_id })?;

    if inv.status == "draft" {
        return Err(ErpError::ValidationFailed {
            message: "Cannot transmit a draft invoice to eTIMS; post it first".to_string(),
        });
    }
    if inv.status == "voided" {
        return Err(ErpError::ValidationFailed {
            message: "Cannot transmit a voided invoice to eTIMS".to_string(),
        });
    }
    if crate::etims::EtimsStatus::from_db(&inv.etims_status).is_transmitted() {
        return Err(ErpError::ValidationFailed {
            message: "Invoice has already been transmitted to eTIMS".to_string(),
        });
    }

    // A transmitted invoice must carry the KRA-issued control number returned by
    // the OSCU/VSCU — recording "transmitted" without it is not a compliant state.
    let etims_invoice_number = match etims_invoice_number {
        Some(n) if !n.trim().is_empty() => n.trim().to_string(),
        _ => {
            return Err(ErpError::ValidationFailed {
                message: "A KRA eTIMS invoice/control number is required to mark an invoice transmitted".to_string(),
            });
        }
    };

    sqlx::query(
        "UPDATE invoices SET etims_status = 'transmitted', etims_invoice_number = $1, \
         etims_transmitted_at = NOW() WHERE id = $2 AND entity_id = $3",
    )
    .bind(etims_invoice_number)
    .bind(invoice_id)
    .bind(entity_id)
    .execute(engine.pool())
    .await?;
    Ok(())
}

// Note: by accounting principle there is no `void_invoice`. A posted invoice
// (transmitted to KRA eTIMS or not) can only be cancelled or reduced via a
// credit note that references it (see `create_credit_note`). Drafts are
// removed with `delete_invoice_draft`.
