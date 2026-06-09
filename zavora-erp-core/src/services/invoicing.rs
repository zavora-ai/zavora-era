use chrono::Utc;
use rust_decimal::Decimal;
use uuid::Uuid;

use crate::engine::ErpEngine;
use crate::error::{ErpError, ErpResult};
use crate::invoicing::*;
use crate::ledger::journal::{CreateJournalEntryRequest, CreateJournalLineRequest, JournalSource};
use crate::types::AgentOrUserId;

/// Create a new invoice.
pub async fn create_invoice(
    engine: &ErpEngine,
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
    .bind(engine.entity_id())
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
        let mut line = resolve_invoice_line(engine, line_req).await?;
        line.compute_totals();
        lines.push(line);
    }

    // Calculate totals
    let subtotal: Decimal = lines.iter().map(|l| l.line_total).sum();
    let tax_total: Decimal = lines.iter().map(|l| l.vat_amount).sum();
    let discount_total: Decimal = lines.iter().map(|l| {
        let gross = l.quantity * l.unit_price;
        gross * l.discount_percent / Decimal::new(100, 0)
    }).sum();
    let gross_total = subtotal + tax_total;

    // Generate invoice number
    let number = generate_invoice_number(engine).await?;

    // Insert into database
    sqlx::query(
        r#"INSERT INTO invoices 
           (id, entity_id, number, invoice_type, customer_id, issue_date, due_date, currency, fx_rate,
            subtotal, discount_total, tax_total, gross_total, amount_paid, balance_due, status,
            source_estimate, template_id, notes, created_at)
           VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17, $18, $19, $20)"#,
    )
    .bind(id)
    .bind(engine.entity_id())
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
    .execute(engine.pool())
    .await?;

    // Insert invoice lines
    for line in &lines {
        sqlx::query(
            r#"INSERT INTO invoice_lines 
               (id, invoice_id, product_id, description, quantity, unit_price, discount_percent, account_code, vat_treatment, line_total, vat_amount)
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
        .execute(engine.pool())
        .await?;
    }

    Ok(Invoice {
        id,
        entity_id: engine.entity_id(),
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
    .bind(engine.entity_id())
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
            })
            .collect()
    } else {
        // Partial credit note — resolve specified lines
        let mut lines = Vec::new();
        for line_req in &req.lines {
            let mut line = resolve_invoice_line(engine, line_req).await?;
            line.compute_totals();
            lines.push(line);
        }
        lines
    };

    // Calculate totals
    let subtotal: Decimal = cn_lines.iter().map(|l| l.line_total).sum();
    let tax_total: Decimal = cn_lines.iter().map(|l| l.vat_amount).sum();
    let gross_total = subtotal + tax_total;

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
    let cn_number = generate_credit_note_number(engine).await?;
    let cn_id = Uuid::new_v4();

    // Insert credit note as invoice record with type 'credit_note'
    sqlx::query(
        r#"INSERT INTO invoices 
           (id, entity_id, number, invoice_type, customer_id, issue_date, due_date, currency, fx_rate,
            subtotal, discount_total, tax_total, gross_total, amount_paid, balance_due, status,
            credit_note_for, notes, created_at)
           VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17, $18, $19)"#,
    )
    .bind(cn_id)
    .bind(engine.entity_id())
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
    .execute(engine.pool())
    .await?;

    // Insert credit note lines
    for line in &cn_lines {
        sqlx::query(
            r#"INSERT INTO invoice_lines 
               (id, invoice_id, product_id, description, quantity, unit_price, discount_percent, account_code, vat_treatment, line_total, vat_amount)
               VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)"#,
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
        .execute(engine.pool())
        .await?;
    }

    // Create reversal GL entry: DR Revenue / DR VAT Output / CR AR
    let mut journal_lines = Vec::new();

    // CR Accounts Receivable (reduce AR)
    journal_lines.push(CreateJournalLineRequest {
        account_code: "1200".to_string(), // Trade Debtors
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
                account_code: "3100".to_string(), // VAT Output
                debit: Some(line.vat_amount),
                credit: None,
                currency: original.currency.clone(),
                fx_rate: Some(original.fx_rate),
                description: Some(format!("CN VAT reversal: {}", line.description)),
                dimensions: None,
            });
        }
    }

    // Post the journal entry
    let entry_req = CreateJournalEntryRequest {
        date: cn_date,
        source: JournalSource::CreditNote,
        reference: cn_number.clone(),
        description: format!("Credit note {} against invoice {}", cn_number, original.number),
        lines: journal_lines,
        post_immediately: true,
    };

    let period = crate::services::periods::period_for_date(engine, cn_date).await?;
    let entry = crate::services::journal::create_and_post(
        engine,
        entry_req,
        period.id,
        created_by.clone(),
    )
    .await?;

    // Update the journal_entry_id on credit note
    sqlx::query("UPDATE invoices SET journal_entry_id = $1 WHERE id = $2")
        .bind(entry.id)
        .bind(cn_id)
        .execute(engine.pool())
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
    .execute(engine.pool())
    .await?;

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
    .bind(engine.entity_id())
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
        let mut line = resolve_invoice_line(engine, line_req).await?;
        line.compute_totals();
        lines.push(line);
    }

    let subtotal: Decimal = lines.iter().map(|l| l.line_total).sum();
    let tax_total: Decimal = lines.iter().map(|l| l.vat_amount).sum();
    let gross_total = subtotal + tax_total;

    // Generate estimate number
    let number = generate_estimate_number(engine).await?;

    // Insert estimate
    sqlx::query(
        r#"INSERT INTO estimates 
           (id, entity_id, number, customer_id, issue_date, expiry_date, currency, fx_rate,
            subtotal, tax_total, gross_total, status, notes, template_id, created_at)
           VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15)"#,
    )
    .bind(id)
    .bind(engine.entity_id())
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
    .execute(engine.pool())
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
        .execute(engine.pool())
        .await?;
    }

    Ok(id)
}

/// Convert an accepted estimate into an invoice.
///
/// Copies all lines from the estimate, creates a new invoice, and marks
/// the estimate as converted with a link to the new invoice.
pub async fn convert_estimate_to_invoice(
    engine: &ErpEngine,
    estimate_id: Uuid,
    created_by: &AgentOrUserId,
) -> ErpResult<Uuid> {
    // Fetch the estimate
    let estimate = sqlx::query_as::<_, EstimateRow>(
        "SELECT * FROM estimates WHERE id = $1 AND entity_id = $2",
    )
    .bind(estimate_id)
    .bind(engine.entity_id())
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

    // Fetch estimate lines
    let est_lines = sqlx::query_as::<_, InvoiceLineRow>(
        "SELECT * FROM estimate_lines WHERE estimate_id = $1",
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
    let invoice = create_invoice(engine, inv_req, created_by).await?;

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
async fn resolve_invoice_line(
    engine: &ErpEngine,
    req: &CreateInvoiceLineRequest,
) -> ErpResult<InvoiceLine> {
    let id = Uuid::new_v4();

    if let Some(product_id) = req.product_id {
        // Look up product for defaults
        let product = sqlx::query_as::<_, crate::catalog::ProductRow>(
            "SELECT * FROM products WHERE id = $1 AND entity_id = $2",
        )
        .bind(product_id)
        .bind(engine.entity_id())
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
            account_code: req.account_code.clone().unwrap_or_else(|| "5000".to_string()),
            vat_treatment: req.vat_treatment.clone().unwrap_or(crate::types::VatTreatment::Standard16),
            line_total: Decimal::ZERO,
            vat_amount: Decimal::ZERO,
        })
    }
}

/// Generate the next invoice number atomically.
async fn generate_invoice_number(engine: &ErpEngine) -> ErpResult<String> {
    let row = sqlx::query_scalar::<_, i64>(
        r#"UPDATE entity_settings 
           SET sequences = jsonb_set(sequences, '{invoice_next}', to_jsonb((sequences->>'invoice_next')::bigint + 1))
           WHERE entity_id = $1
           RETURNING (sequences->>'invoice_next')::bigint - 1"#,
    )
    .bind(engine.entity_id())
    .fetch_one(engine.pool())
    .await?;

    let prefix = &engine.config().sequences.invoice_prefix;
    let fiscal_year = Utc::now().format("%Y").to_string();

    Ok(format!("{}-{}-{:04}", prefix, fiscal_year, row))
}

/// Generate the next credit note number atomically.
async fn generate_credit_note_number(engine: &ErpEngine) -> ErpResult<String> {
    let row = sqlx::query_scalar::<_, i64>(
        r#"UPDATE entity_settings 
           SET sequences = jsonb_set(sequences, '{credit_note_next}', to_jsonb((sequences->>'credit_note_next')::bigint + 1))
           WHERE entity_id = $1
           RETURNING (sequences->>'credit_note_next')::bigint - 1"#,
    )
    .bind(engine.entity_id())
    .fetch_one(engine.pool())
    .await?;

    let prefix = &engine.config().sequences.credit_note_prefix;
    let fiscal_year = Utc::now().format("%Y").to_string();

    Ok(format!("{}-{}-{:04}", prefix, fiscal_year, row))
}

/// Generate the next estimate number atomically.
async fn generate_estimate_number(engine: &ErpEngine) -> ErpResult<String> {
    let row = sqlx::query_scalar::<_, i64>(
        r#"UPDATE entity_settings 
           SET sequences = jsonb_set(sequences, '{estimate_next}', to_jsonb((sequences->>'estimate_next')::bigint + 1))
           WHERE entity_id = $1
           RETURNING (sequences->>'estimate_next')::bigint - 1"#,
    )
    .bind(engine.entity_id())
    .fetch_one(engine.pool())
    .await?;

    let prefix = &engine.config().sequences.estimate_prefix;
    let fiscal_year = Utc::now().format("%Y").to_string();

    Ok(format!("{}-{}-{:04}", prefix, fiscal_year, row))
}

/// Post an invoice — creates the GL journal entry (DR AR / CR Revenue / CR VAT Output).
pub async fn post_invoice(
    engine: &ErpEngine,
    invoice_id: Uuid,
    posted_by: &AgentOrUserId,
) -> ErpResult<Uuid> {
    let invoice = sqlx::query_as::<_, InvoiceRow>(
        "SELECT * FROM invoices WHERE id = $1 AND entity_id = $2",
    )
    .bind(invoice_id)
    .bind(engine.entity_id())
    .fetch_optional(engine.pool())
    .await?
    .ok_or_else(|| ErpError::NotFound {
        entity_type: "Invoice".to_string(),
        id: invoice_id,
    })?;

    if invoice.status != "draft" && invoice.status != "sent" {
        return Err(ErpError::ValidationFailed {
            message: format!("Invoice {} is already posted (status: {})", invoice.number, invoice.status),
        });
    }

    // Build journal entry lines
    let lines = sqlx::query_as::<_, InvoiceLineRow>(
        "SELECT * FROM invoice_lines WHERE invoice_id = $1",
    )
    .bind(invoice_id)
    .fetch_all(engine.pool())
    .await?;

    let mut journal_lines = Vec::new();

    // DR Accounts Receivable (total including tax)
    journal_lines.push(CreateJournalLineRequest {
        account_code: "1200".to_string(), // Trade Debtors
        debit: Some(invoice.gross_total),
        credit: None,
        currency: invoice.currency.clone(),
        fx_rate: Some(invoice.fx_rate),
        description: Some(format!("Invoice {} - {}", invoice.number, "AR")),
        dimensions: None,
    });

    // CR Revenue (per line)
    for line in &lines {
        journal_lines.push(CreateJournalLineRequest {
            account_code: line.account_code.clone(),
            debit: None,
            credit: Some(line.line_total),
            currency: invoice.currency.clone(),
            fx_rate: Some(invoice.fx_rate),
            description: Some(line.description.clone()),
            dimensions: None,
        });

        // CR VAT Output (if applicable)
        if line.vat_amount > Decimal::ZERO {
            journal_lines.push(CreateJournalLineRequest {
                account_code: "3100".to_string(), // VAT Output
                debit: None,
                credit: Some(line.vat_amount),
                currency: invoice.currency.clone(),
                fx_rate: Some(invoice.fx_rate),
                description: Some(format!("VAT on {}", line.description)),
                dimensions: None,
            });
        }
    }

    // Create and post journal entry
    let entry_req = CreateJournalEntryRequest {
        date: invoice.issue_date,
        source: JournalSource::Invoice,
        reference: invoice.number.clone(),
        description: format!("Invoice {} posted", invoice.number),
        lines: journal_lines,
        post_immediately: true,
    };

    let period = crate::services::periods::period_for_date(engine, invoice.issue_date).await?;
    let entry = crate::services::journal::create_and_post(engine, entry_req, period.id, posted_by.clone()).await?;

    // Update invoice status and link journal entry
    sqlx::query(
        "UPDATE invoices SET status = 'sent', journal_entry_id = $1 WHERE id = $2",
    )
    .bind(entry.id)
    .bind(invoice_id)
    .execute(engine.pool())
    .await?;

    Ok(entry.id)
}


/// Resolve a bill line from a request — auto-fills from product catalog (purchase side).
pub async fn resolve_bill_line(
    engine: &ErpEngine,
    req: &CreateInvoiceLineRequest,
    vendor: &crate::parties::VendorRow,
) -> ErpResult<InvoiceLine> {
    let id = Uuid::new_v4();

    if let Some(product_id) = req.product_id {
        let product = sqlx::query_as::<_, crate::catalog::ProductRow>(
            "SELECT * FROM products WHERE id = $1 AND entity_id = $2",
        )
        .bind(product_id)
        .bind(engine.entity_id())
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
        })
    } else {
        let default_account = vendor
            .default_expense_account
            .clone()
            .unwrap_or_else(|| "7900".to_string());

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
        })
    }
}
