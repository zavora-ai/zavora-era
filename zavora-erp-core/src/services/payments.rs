use chrono::Utc;
use rust_decimal::Decimal;
use uuid::Uuid;

use crate::engine::ErpEngine;
use crate::error::{ErpError, ErpResult};
use crate::payments::*;
use crate::types::AgentOrUserId;

/// Record a payment (AR or AP).
pub async fn record_payment(
    engine: &ErpEngine,
    req: RecordPaymentRequest,
    recorded_by: &AgentOrUserId,
) -> ErpResult<Payment> {
    let id = Uuid::new_v4();
    let today = Utc::now().date_naive();
    let payment_date = req.payment_date.unwrap_or(today);
    let currency = req.currency.clone().unwrap_or_else(|| engine.config().base_currency.clone());

    // Validate applications don't exceed payment amount
    let total_applied: Decimal = req.applications.iter().map(|a| a.amount).sum();
    if total_applied > req.amount {
        return Err(ErpError::ValidationFailed {
            message: "Total applied amount exceeds payment amount".to_string(),
        });
    }

    let unapplied = req.amount - total_applied;

    // Generate payment number
    let number = generate_payment_number(engine).await?;

    // Build applications
    let applications: Vec<PaymentApplication> = req
        .applications
        .iter()
        .map(|a| PaymentApplication {
            document_id: a.document_id,
            document_type: match req.payment_type {
                PaymentType::CustomerPayment => PaymentDocType::Invoice,
                PaymentType::VendorPayment => PaymentDocType::Bill,
            },
            amount_applied: a.amount,
        })
        .collect();

    let reference = req.reference.unwrap_or_else(|| number.clone());

    // Insert payment
    sqlx::query(
        r#"INSERT INTO payments 
           (id, entity_id, number, payment_type, party_id, payment_date, amount, currency, fx_rate,
            method, reference, bank_account_id, applications, unapplied, status, created_at)
           VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16)"#,
    )
    .bind(id)
    .bind(engine.entity_id())
    .bind(&number)
    .bind(serde_json::to_string(&req.payment_type).unwrap_or_default())
    .bind(req.party_id)
    .bind(payment_date)
    .bind(req.amount)
    .bind(&currency)
    .bind(req.fx_rate.unwrap_or(Decimal::ONE))
    .bind(serde_json::to_value(&req.method).unwrap_or_default())
    .bind(&reference)
    .bind(req.bank_account_id)
    .bind(serde_json::to_value(&applications).unwrap_or_default())
    .bind(unapplied)
    .bind("completed")
    .bind(Utc::now())
    .execute(engine.pool())
    .await?;

    // Update invoice/bill balances for each application
    for app in &req.applications {
        match req.payment_type {
            PaymentType::CustomerPayment => {
                sqlx::query(
                    "UPDATE invoices SET amount_paid = amount_paid + $1, balance_due = balance_due - $1 WHERE id = $2",
                )
                .bind(app.amount)
                .bind(app.document_id)
                .execute(engine.pool())
                .await?;

                // Update status if fully paid
                sqlx::query(
                    "UPDATE invoices SET status = CASE WHEN balance_due <= 0 THEN 'paid' ELSE 'partially_paid' END, paid_at = CASE WHEN balance_due <= 0 THEN NOW() ELSE paid_at END WHERE id = $1",
                )
                .bind(app.document_id)
                .execute(engine.pool())
                .await?;
            }
            PaymentType::VendorPayment => {
                sqlx::query(
                    "UPDATE bills SET amount_paid = amount_paid + $1, balance_due = balance_due - $1 WHERE id = $2",
                )
                .bind(app.amount)
                .bind(app.document_id)
                .execute(engine.pool())
                .await?;

                sqlx::query(
                    "UPDATE bills SET status = CASE WHEN balance_due <= 0 THEN 'paid' ELSE 'partially_paid' END WHERE id = $1",
                )
                .bind(app.document_id)
                .execute(engine.pool())
                .await?;
            }
        }
    }

    Ok(Payment {
        id,
        entity_id: engine.entity_id(),
        number,
        payment_type: req.payment_type,
        party_id: req.party_id,
        payment_date,
        amount: req.amount,
        currency,
        fx_rate: req.fx_rate.unwrap_or(Decimal::ONE),
        method: req.method,
        reference,
        bank_account_id: req.bank_account_id,
        applications,
        unapplied,
        journal_entry_id: None,
        status: PaymentStatus::Completed,
        created_at: Utc::now(),
    })
}

/// Record an M-Pesa payment from Daraja callback.
pub async fn record_mpesa_payment(
    engine: &ErpEngine,
    invoice_id: Uuid,
    callback: MpesaCallback,
) -> ErpResult<Payment> {
    if !callback.is_success() {
        return Err(ErpError::PaymentError {
            message: format!("M-Pesa transaction failed: {}", callback.result_desc),
        });
    }

    let amount = callback.amount.ok_or_else(|| ErpError::PaymentError {
        message: "M-Pesa callback missing amount".to_string(),
    })?;

    let receipt = callback.mpesa_receipt_number.clone().unwrap_or_default();
    let phone = callback.phone_number.clone().unwrap_or_default();

    // Look up invoice to get customer
    let invoice = sqlx::query_as::<_, crate::invoicing::InvoiceRow>(
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

    // Check for overpayment
    if amount > invoice.balance_due {
        return Err(ErpError::Overpayment {
            invoice_id,
            balance: invoice.balance_due,
            amount,
        });
    }

    let req = RecordPaymentRequest {
        payment_type: PaymentType::CustomerPayment,
        party_id: invoice.customer_id,
        payment_date: None,
        amount,
        currency: Some(invoice.currency),
        fx_rate: Some(invoice.fx_rate),
        method: PaymentMethod::Mpesa {
            transaction_id: receipt,
            phone,
        },
        reference: Some(callback.mpesa_receipt_number.unwrap_or_default()),
        bank_account_id: None,
        applications: vec![PaymentApplicationRequest {
            document_id: invoice_id,
            amount,
        }],
    };

    let actor = AgentOrUserId::Agent("mpesa-webhook".to_string());
    record_payment(engine, req, &actor).await
}

async fn generate_payment_number(engine: &ErpEngine) -> ErpResult<String> {
    let row = sqlx::query_scalar::<_, i64>(
        r#"UPDATE entity_settings 
           SET sequences = jsonb_set(sequences, '{payment_next}', to_jsonb((sequences->>'payment_next')::bigint + 1))
           WHERE entity_id = $1
           RETURNING (sequences->>'payment_next')::bigint - 1"#,
    )
    .bind(engine.entity_id())
    .fetch_one(engine.pool())
    .await?;

    let prefix = &engine.config().sequences.payment_prefix;
    let fiscal_year = Utc::now().format("%Y").to_string();
    Ok(format!("{}-{}-{:04}", prefix, fiscal_year, row))
}
