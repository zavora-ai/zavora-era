use chrono::{NaiveDate, Utc};
use rust_decimal::Decimal;
use uuid::Uuid;

use crate::engine::ErpEngine;
use crate::error::{ErpError, ErpResult};
use crate::invoicing::*;
use crate::types::AgentOrUserId;

/// Create a new invoice.
pub async fn create_invoice(
    engine: &ErpEngine,
    req: CreateInvoiceRequest,
    created_by: &AgentOrUserId,
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
    journal_lines.push(crate::ledger::journal::CreateJournalLineRequest {
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
        journal_lines.push(crate::ledger::journal::CreateJournalLineRequest {
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
            journal_lines.push(crate::ledger::journal::CreateJournalLineRequest {
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
    let entry_req = crate::ledger::journal::CreateJournalEntryRequest {
        date: invoice.issue_date,
        source: crate::ledger::journal::JournalSource::Invoice,
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
