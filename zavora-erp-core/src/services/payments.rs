use chrono::Utc;
use rust_decimal::Decimal;
use uuid::Uuid;

use crate::engine::ErpEngine;
use crate::error::{ErpError, ErpResult};
use crate::ledger::journal::{CreateJournalEntryRequest, CreateJournalLineRequest, JournalSource};
use crate::money::round_money;
use crate::payments::*;
use crate::types::AgentOrUserId;

/// GL account codes used for payment journal entries, resolved from the entity's
/// posting setup (`crate::posting::PostingSetup`) rather than hardcoded literals.
struct PaymentAccounts {
    ar: String,
    ap: String,
    unapplied_payments: String,
    wht_payable: String,
    realised_fx_gain: String,
    realised_fx_loss: String,
}

impl PaymentAccounts {
    async fn resolve(engine: &ErpEngine, entity_id: Uuid) -> ErpResult<Self> {
        let p = engine.posting_for(entity_id).await?;
        Ok(Self {
            ar: p.accounts_receivable.clone(),
            ap: p.accounts_payable.clone(),
            unapplied_payments: p.unapplied_payments.clone(),
            wht_payable: p.wht_payable.clone(),
            realised_fx_gain: p.realised_fx_gain.clone(),
            realised_fx_loss: p.realised_fx_loss.clone(),
        })
    }
}

/// Helper row for fetching bill balance + status for payment validation.
#[derive(sqlx::FromRow)]
struct BillBalanceRow {
    pub balance_due: Decimal,
    pub status: String,
}

/// Record a payment (AR or AP).
///
/// Overpayment handling (Requirements 3.4, 3.5, 3.6, 24.1):
/// - When a payment application amount exceeds the document's balance_due,
///   only balance_due is applied; the remainder becomes unapplied credit.
/// - When a payment has no applications at all, the full amount is held as
///   an Unapplied_Payment on the customer/vendor account.
/// - Journal entry is split accordingly:
///   DR Bank / CR AR (applied portion) + CR Unapplied Payments (excess portion)
pub async fn record_payment(
    engine: &ErpEngine,
    entity_id: Uuid,
    req: RecordPaymentRequest,
    recorded_by: &AgentOrUserId,
) -> ErpResult<Payment> {
    let id = Uuid::new_v4();
    let today = Utc::now().date_naive();
    let payment_date = req.payment_date.unwrap_or(today);
    let currency = match req.currency.clone() {
        Some(c) => c,
        None => engine.config_for(entity_id).await?.base_currency.clone(),
    };
    let fx_rate = req.fx_rate.unwrap_or(Decimal::ONE);

    // Validate applications don't exceed payment amount
    let total_requested: Decimal = req.applications.iter().map(|a| a.amount).sum();
    if total_requested > req.amount {
        return Err(ErpError::ValidationFailed {
            message: "Total applied amount exceeds payment amount".to_string(),
        });
    }

    // Generate payment number
    let number = generate_payment_number(engine, entity_id).await?;

    // --- Overpayment handling ---
    // For each application, cap the applied amount to the document's balance_due.
    // Any excess per-document is accumulated as unapplied credit.
    let mut applications: Vec<PaymentApplication> = Vec::new();
    let mut total_actually_applied = Decimal::ZERO;

    for app_req in &req.applications {
        let doc_balance = fetch_document_balance(engine, entity_id, app_req.document_id, &req.payment_type).await?;

        // Apply only up to balance_due; remainder becomes unapplied
        let effective_apply = app_req.amount.min(doc_balance);

        applications.push(PaymentApplication {
            document_id: app_req.document_id,
            document_type: match req.payment_type {
                PaymentType::CustomerPayment => PaymentDocType::Invoice,
                PaymentType::VendorPayment => PaymentDocType::Bill,
            },
            amount_applied: effective_apply,
        });

        total_actually_applied += effective_apply;
    }

    // Total unapplied = payment amount minus what was actually applied to documents
    let unapplied = req.amount - total_actually_applied;

    let reference = req.reference.unwrap_or_else(|| number.clone());

    // Determine the bank account code for the JE
    let bank_account_code = resolve_bank_account_code(engine, entity_id, req.bank_account_id).await?;

    // Everything that touches the ledger (payment record, document balances, and
    // the journal entry) commits or rolls back together (Requirement 2.1).
    let mut tx = engine.pool().begin().await?;

    // Insert payment record
    sqlx::query(
        r#"INSERT INTO payments 
           (id, entity_id, number, payment_type, party_id, payment_date, amount, currency, fx_rate,
            method, reference, bank_account_id, applications, unapplied, status, created_at)
           VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16)"#,
    )
    .bind(id)
    .bind(entity_id)
    .bind(&number)
    .bind(serde_json::to_string(&req.payment_type).unwrap_or_default())
    .bind(req.party_id)
    .bind(payment_date)
    .bind(req.amount)
    .bind(&currency)
    .bind(fx_rate)
    .bind(serde_json::to_value(&req.method).unwrap_or_default())
    .bind(&reference)
    .bind(req.bank_account_id)
    .bind(serde_json::to_value(&applications).unwrap_or_default())
    .bind(unapplied)
    .bind("completed")
    .bind(Utc::now())
    .execute(&mut *tx)
    .await?;

    // Update invoice/bill balances for each application (using effective amounts)
    for app in &applications {
        if app.amount_applied == Decimal::ZERO {
            continue;
        }
        match req.payment_type {
            PaymentType::CustomerPayment => {
                sqlx::query(
                    "UPDATE invoices SET amount_paid = amount_paid + $1, balance_due = balance_due - $1 WHERE id = $2",
                )
                .bind(app.amount_applied)
                .bind(app.document_id)
                .execute(&mut *tx)
                .await?;

                // Update status if fully paid
                sqlx::query(
                    "UPDATE invoices SET status = CASE WHEN balance_due <= 0 THEN 'paid' ELSE 'partially_paid' END, paid_at = CASE WHEN balance_due <= 0 THEN NOW() ELSE paid_at END WHERE id = $1",
                )
                .bind(app.document_id)
                .execute(&mut *tx)
                .await?;
            }
            PaymentType::VendorPayment => {
                sqlx::query(
                    "UPDATE bills SET amount_paid = amount_paid + $1, balance_due = balance_due - $1 WHERE id = $2",
                )
                .bind(app.amount_applied)
                .bind(app.document_id)
                .execute(&mut *tx)
                .await?;

                sqlx::query(
                    "UPDATE bills SET status = CASE WHEN balance_due <= 0 THEN 'paid' ELSE 'partially_paid' END WHERE id = $1",
                )
                .bind(app.document_id)
                .execute(&mut *tx)
                .await?;
            }
        }
    }

    // --- WHT handling for vendor payments (Requirements 11.4, 11.5) ---
    // When paying a bill with WHT, we need to also clear the WHT Payable liability
    // that was created when the bill was posted to GL.
    // JE structure: DR AP (applied) / CR Bank (applied) + DR WHT Payable / CR Bank (wht)
    let wht_total = if req.payment_type == PaymentType::VendorPayment {
        let mut wht_sum = Decimal::ZERO;
        for app in &applications {
            if app.amount_applied == Decimal::ZERO {
                continue;
            }
            let bill_wht = fetch_bill_wht_amount(engine, entity_id, app.document_id).await.unwrap_or(Decimal::ZERO);
            wht_sum += bill_wht;
        }
        wht_sum
    } else {
        Decimal::ZERO
    };

    // --- Post the payment journal entry ---
    // For customer payments: DR Bank / CR AR (applied) / CR Unapplied Payments (excess)
    // For vendor payments: DR AP (applied) / CR Bank (applied) + DR WHT Payable / CR Bank (wht)
    let journal_entry_id = post_payment_journal_entry(
        &mut tx,
        engine,
        entity_id,
        &number,
        payment_date,
        &currency,
        fx_rate,
        req.amount,
        total_actually_applied,
        unapplied,
        &bank_account_code,
        &req.payment_type,
        wht_total,
        recorded_by,
    )
    .await?;

    // Link journal entry to payment
    sqlx::query("UPDATE payments SET journal_entry_id = $1 WHERE id = $2")
        .bind(journal_entry_id)
        .bind(id)
        .execute(&mut *tx)
        .await?;

    // Commit the atomic ledger writes. Side-effects below (reminder cancellation,
    // FX gain/loss) run post-commit and are best-effort (Requirement 2.1 allows
    // these to run after the core transaction).
    tx.commit().await?;

    // Cancel pending reminders for customer invoices that were paid down.
    if req.payment_type == PaymentType::CustomerPayment {
        for app in &applications {
            if app.amount_applied == Decimal::ZERO {
                continue;
            }
            let new_balance = fetch_document_balance(engine, entity_id, app.document_id, &req.payment_type)
                .await
                .unwrap_or(Decimal::ZERO);
            let _ = crate::services::scheduler::cancel_reminders_on_payment(
                engine,
                entity_id,
                app.document_id,
                new_balance,
            )
            .await;
        }
    }

    // --- FX Gain/Loss handling (Requirements 22.2, 22.3, 22.5) ---
    // When payment currency differs from invoice/bill currency (cross-currency),
    // or even same currency but at a different FX rate, compute realised FX gain/loss.
    for app in &applications {
        if app.amount_applied == Decimal::ZERO {
            continue;
        }
        let doc_fx_rate = fetch_document_fx_rate(engine, entity_id, app.document_id, &req.payment_type).await?;

        // Only post FX gain/loss if the rates differ
        if doc_fx_rate != fx_rate {
            post_fx_gain_loss_entry(
                engine,
                entity_id,
                &number,
                payment_date,
                &currency,
                fx_rate,
                doc_fx_rate,
                app.amount_applied,
                &req.payment_type,
                recorded_by,
            )
            .await?;

            // Record the exchange rate used in audit trail
            let audit_event = serde_json::json!({
                "event_type": "fx_gain_loss",
                "object_type": "payment",
                "object_id": id.to_string(),
                "document_id": app.document_id.to_string(),
                "payment_fx_rate": fx_rate.to_string(),
                "invoice_fx_rate": doc_fx_rate.to_string(),
                "applied_amount": app.amount_applied.to_string(),
                "currency": currency,
                "timestamp": Utc::now().to_rfc3339(),
            });
            let stream_key = format!("erp:audit:{}", entity_id);
            let mut redis_conn = engine.redis_conn().await;
            let _: Result<(), _> = redis::cmd("XADD")
                .arg(&stream_key)
                .arg("*")
                .arg("data")
                .arg(audit_event.to_string())
                .query_async(&mut redis_conn)
                .await;
        }
    }

    Ok(Payment {
        id,
        entity_id,
        number,
        payment_type: req.payment_type,
        party_id: req.party_id,
        payment_date,
        amount: req.amount,
        currency,
        fx_rate,
        method: req.method,
        reference,
        bank_account_id: req.bank_account_id,
        applications,
        unapplied,
        journal_entry_id: Some(journal_entry_id),
        status: PaymentStatus::Completed,
        created_at: Utc::now(),
    })
}

/// Fetch the current balance_due for a document (invoice or bill).
///
/// For vendor payments (bills): also validates that the bill is in an appropriate
/// status for payment (must be Approved, Posted, or PartiallyPaid). Payments on
/// bills in Draft or PendingApproval status are rejected (Requirement 11.6).
async fn fetch_document_balance(
    engine: &ErpEngine,
    entity_id: Uuid,
    document_id: Uuid,
    payment_type: &PaymentType,
) -> ErpResult<Decimal> {
    match payment_type {
        PaymentType::CustomerPayment => {
            sqlx::query_scalar::<_, Decimal>(
                "SELECT balance_due FROM invoices WHERE id = $1 AND entity_id = $2",
            )
            .bind(document_id)
            .bind(entity_id)
            .fetch_optional(engine.pool())
            .await?
            .ok_or_else(|| ErpError::NotFound {
                entity_type: "Invoice".to_string(),
                id: document_id,
            })
        }
        PaymentType::VendorPayment => {
            // Fetch both balance_due and status to validate bill is payable
            let row = sqlx::query_as::<_, BillBalanceRow>(
                "SELECT balance_due, status FROM bills WHERE id = $1 AND entity_id = $2",
            )
            .bind(document_id)
            .bind(entity_id)
            .fetch_optional(engine.pool())
            .await?
            .ok_or_else(|| ErpError::NotFound {
                entity_type: "Bill".to_string(),
                id: document_id,
            })?;

            // Reject payment on bills in Draft or PendingApproval status (Requirement 11.6)
            match row.status.as_str() {
                "draft" | "pending_approval" => {
                    return Err(ErpError::ValidationFailed {
                        message: format!(
                            "Cannot process payment for bill in '{}' status; bill must be approved/posted first",
                            row.status
                        ),
                    });
                }
                _ => {}
            }

            Ok(row.balance_due)
        }
    }
}

/// Fetch the WHT (Withholding Tax) amount from a bill.
/// Returns Decimal::ZERO if the bill has no WHT or if the bill is not found.
/// Used during vendor payment to determine how much WHT liability to clear.
async fn fetch_bill_wht_amount(
    engine: &ErpEngine,
    entity_id: Uuid,
    bill_id: Uuid,
) -> ErpResult<Decimal> {
    let wht = sqlx::query_scalar::<_, Decimal>(
        "SELECT COALESCE(wht_amount, 0) FROM bills WHERE id = $1 AND entity_id = $2",
    )
    .bind(bill_id)
    .bind(entity_id)
    .fetch_optional(engine.pool())
    .await?
    .unwrap_or(Decimal::ZERO);

    Ok(wht)
}

/// Resolve the GL account code for a bank account.
/// Falls back to "1020" (default bank/cash account) if no bank_account_id is provided
/// or if the bank account has no linked GL account.
async fn resolve_bank_account_code(
    engine: &ErpEngine,
    entity_id: Uuid,
    bank_account_id: Option<Uuid>,
) -> ErpResult<String> {
    let default_bank = engine.posting_for(entity_id).await?.default_bank.clone();

    let Some(ba_id) = bank_account_id else {
        return Ok(default_bank);
    };

    let code = sqlx::query_scalar::<_, String>(
        "SELECT COALESCE(gl_account, $3) FROM bank_accounts WHERE id = $1 AND entity_id = $2",
    )
    .bind(ba_id)
    .bind(entity_id)
    .bind(&default_bank)
    .fetch_optional(engine.pool())
    .await?
    .unwrap_or(default_bank);

    Ok(code)
}

/// Post the journal entry for a payment, handling the split between applied and unapplied portions.
///
/// Journal structure for customer payments:
/// - DR Bank account (full payment amount)
/// - CR AR (applied portion — the amount that cleared document balances)
/// - CR Unapplied Payments (excess portion — held as credit on the party's account)
///
/// Journal structure for vendor payments (Requirements 11.4, 11.5):
/// - DR AP (applied portion — the amount clearing the vendor's balance)
/// - CR Bank (payment amount — what actually leaves the bank)
/// - DR WHT Payable (WHT amount — clearing the WHT liability set up at bill posting)
/// - CR Bank (WHT amount — if WHT is being remitted to KRA with this payment)
/// - CR Unapplied Payments (excess portion, if any)
///
/// When payment has no applications (unapplied == full amount):
/// - DR Bank / CR Unapplied Payments (entire amount) for customer payments
/// - DR Unapplied Payments / CR Bank (entire amount) for vendor payments
#[allow(clippy::too_many_arguments)]
async fn post_payment_journal_entry(
    tx: &mut crate::services::journal::PgTx<'_>,
    engine: &ErpEngine,
    entity_id: Uuid,
    payment_number: &str,
    payment_date: chrono::NaiveDate,
    currency: &str,
    fx_rate: Decimal,
    total_amount: Decimal,
    applied_amount: Decimal,
    unapplied_amount: Decimal,
    bank_account_code: &str,
    payment_type: &PaymentType,
    wht_amount: Decimal,
    posted_by: &AgentOrUserId,
) -> ErpResult<Uuid> {
    let acct = PaymentAccounts::resolve(engine, entity_id).await?;
    let mut lines: Vec<CreateJournalLineRequest> = Vec::new();

    match payment_type {
        PaymentType::CustomerPayment => {
            // DR Bank (full amount received)
            lines.push(CreateJournalLineRequest {
                account_code: bank_account_code.to_string(),
                debit: Some(total_amount),
                credit: None,
                currency: currency.to_string(),
                fx_rate: Some(fx_rate),
                description: Some(format!("Payment received: {}", payment_number)),
                dimensions: None,
            });

            // CR AR (applied portion)
            if applied_amount > Decimal::ZERO {
                lines.push(CreateJournalLineRequest {
                    account_code: acct.ar.clone(),
                    debit: None,
                    credit: Some(applied_amount),
                    currency: currency.to_string(),
                    fx_rate: Some(fx_rate),
                    description: Some(format!("Applied to documents: {}", payment_number)),
                    dimensions: None,
                });
            }

            // CR Unapplied Payments (excess portion)
            if unapplied_amount > Decimal::ZERO {
                lines.push(CreateJournalLineRequest {
                    account_code: acct.unapplied_payments.clone(),
                    debit: None,
                    credit: Some(unapplied_amount),
                    currency: currency.to_string(),
                    fx_rate: Some(fx_rate),
                    description: Some(format!("Unapplied payment credit: {}", payment_number)),
                    dimensions: None,
                });
            }
        }
        PaymentType::VendorPayment => {
            // DR AP (applied portion — the full balance being cleared, which is net of WHT)
            if applied_amount > Decimal::ZERO {
                lines.push(CreateJournalLineRequest {
                    account_code: acct.ap.clone(),
                    debit: Some(applied_amount),
                    credit: None,
                    currency: currency.to_string(),
                    fx_rate: Some(fx_rate),
                    description: Some(format!("Bill payment - AP cleared: {}", payment_number)),
                    dimensions: None,
                });
            }

            // CR Bank (net amount actually paid to vendor)
            lines.push(CreateJournalLineRequest {
                account_code: bank_account_code.to_string(),
                debit: None,
                credit: Some(total_amount),
                currency: currency.to_string(),
                fx_rate: Some(fx_rate),
                description: Some(format!("Payment made: {}", payment_number)),
                dimensions: None,
            });

            // WHT handling (Requirements 11.4, 11.5):
            // DR WHT Payable (clearing liability) / CR Bank (WHT remitted to KRA)
            if wht_amount > Decimal::ZERO {
                lines.push(CreateJournalLineRequest {
                    account_code: acct.wht_payable.clone(),
                    debit: Some(wht_amount),
                    credit: None,
                    currency: currency.to_string(),
                    fx_rate: Some(fx_rate),
                    description: Some(format!("WHT remitted to KRA: {}", payment_number)),
                    dimensions: None,
                });
                lines.push(CreateJournalLineRequest {
                    account_code: bank_account_code.to_string(),
                    debit: None,
                    credit: Some(wht_amount),
                    currency: currency.to_string(),
                    fx_rate: Some(fx_rate),
                    description: Some(format!("WHT payment to KRA: {}", payment_number)),
                    dimensions: None,
                });
            }

            // CR Unapplied Payments (excess portion)
            if unapplied_amount > Decimal::ZERO {
                lines.push(CreateJournalLineRequest {
                    account_code: acct.unapplied_payments.clone(),
                    debit: None,
                    credit: Some(unapplied_amount),
                    currency: currency.to_string(),
                    fx_rate: Some(fx_rate),
                    description: Some(format!("Unapplied payment credit: {}", payment_number)),
                    dimensions: None,
                });
            }
        }
    }

    let description = match payment_type {
        PaymentType::CustomerPayment => {
            if unapplied_amount > Decimal::ZERO && applied_amount > Decimal::ZERO {
                format!(
                    "Payment {} — applied {} / unapplied {}",
                    payment_number, applied_amount, unapplied_amount
                )
            } else if unapplied_amount > Decimal::ZERO {
                format!("Payment {} — full amount held as unapplied", payment_number)
            } else {
                format!("Payment {} — fully applied", payment_number)
            }
        }
        PaymentType::VendorPayment => {
            if wht_amount > Decimal::ZERO {
                format!(
                    "Payment {} — vendor payment {} + WHT {} remitted",
                    payment_number, total_amount, wht_amount
                )
            } else if unapplied_amount > Decimal::ZERO {
                format!(
                    "Payment {} — applied {} / unapplied {}",
                    payment_number, applied_amount, unapplied_amount
                )
            } else {
                format!("Payment {} — fully applied to vendor bill", payment_number)
            }
        }
    };

    let je_req = CreateJournalEntryRequest {
        date: payment_date,
        source: JournalSource::Payment,
        source_id: None,
        reference: payment_number.to_string(),
        description,
        lines,
        post_immediately: true,
    };

    // Resolve the fiscal period for the payment date
    let period = crate::services::periods::period_for_date(engine, entity_id, payment_date).await?;

    let entry = crate::services::journal::create_and_post_in_tx(
        tx,
        engine,
        entity_id,
        je_req,
        period.id,
        posted_by.clone(),
    )
    .await?;

    Ok(entry.id)
}

/// Record an M-Pesa payment from Daraja callback.
///
/// Overpayment handling: If the M-Pesa amount exceeds the invoice balance_due,
/// the payment is still accepted — the excess is held as unapplied credit rather
/// than rejected. This allows the `record_payment` function's overpayment logic
/// to handle the split automatically.
pub async fn record_mpesa_payment(
    engine: &ErpEngine,
    entity_id: Uuid,
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
    .bind(entity_id)
    .fetch_optional(engine.pool())
    .await?
    .ok_or_else(|| ErpError::NotFound {
        entity_type: "Invoice".to_string(),
        id: invoice_id,
    })?;

    // Idempotency: claim this M-Pesa receipt before recording a payment so that
    // duplicate Daraja callbacks (delivered at-least-once) cannot create duplicate
    // payments. The unique index on (entity_id, receipt_number) enforces the claim.
    if !receipt.is_empty() {
        let claim = sqlx::query(
            r#"INSERT INTO mpesa_transactions
               (entity_id, receipt_number, transaction_type, amount, phone_number, timestamp, invoice_id, reconciled)
               VALUES ($1, $2, 'c2b', $3, $4, $5, $6, false)"#,
        )
        .bind(entity_id)
        .bind(&receipt)
        .bind(amount)
        .bind(&phone)
        .bind(callback.transaction_date.unwrap_or_else(Utc::now))
        .bind(invoice_id)
        .execute(engine.pool())
        .await;
        if let Err(e) = claim {
            let is_dup = matches!(&e, sqlx::Error::Database(db) if db.is_unique_violation());
            if !is_dup {
                return Err(ErpError::Database(e));
            }
            // Duplicate callback. Two cases:
            //  (a) a payment was already recorded for this receipt -> idempotent
            //      success, return the existing payment.
            //  (b) the claim row exists but has no payment_id -> a previous attempt
            //      crashed between claiming the receipt and recording the payment
            //      (an orphaned claim). Recover by continuing to record the payment
            //      now, rather than rejecting forever and losing the money.
            let existing = sqlx::query_as::<_, PaymentRow>(
                r#"SELECT p.* FROM payments p
                   JOIN mpesa_transactions m ON m.payment_id = p.id
                   WHERE m.entity_id = $1 AND m.receipt_number = $2"#,
            )
            .bind(entity_id)
            .bind(&receipt)
            .fetch_optional(engine.pool())
            .await?;

            if let Some(row) = existing {
                return Ok(payment_from_row(row));
            }
            // Orphaned claim recovery: fall through to record the payment. The
            // back-link UPDATE below will attach payment_id to the existing claim
            // row. (If two callbacks race here, the payments unique number + the
            // single claim row keep this safe; at worst one retry is needed.)
            tracing::warn!(
                "Recovering orphaned M-Pesa claim for receipt {} (no payment linked yet)",
                receipt
            );
        }
    }

    // No longer reject overpayments — the record_payment function handles the split.
    // The application amount is the full M-Pesa amount; record_payment will cap it
    // at balance_due and create unapplied credit for the excess.

    let req = RecordPaymentRequest {
        payment_type: PaymentType::CustomerPayment,
        party_id: invoice.customer_id,
        payment_date: None,
        amount,
        currency: Some(invoice.currency),
        fx_rate: Some(invoice.fx_rate),
        method: PaymentMethod::Mpesa {
            transaction_id: receipt.clone(),
            phone: phone.clone(),
        },
        reference: Some(callback.mpesa_receipt_number.unwrap_or_default()),
        bank_account_id: None,
        applications: vec![PaymentApplicationRequest {
            document_id: invoice_id,
            amount,
        }],
    };

    let actor = AgentOrUserId::Agent("mpesa-webhook".to_string());
    let payment = record_payment(engine, entity_id, req, &actor).await?;

    // Link the recorded payment back to the claimed M-Pesa transaction.
    if !receipt.is_empty() {
        let _ = sqlx::query(
            "UPDATE mpesa_transactions SET payment_id = $1, reconciled = true WHERE entity_id = $2 AND receipt_number = $3",
        )
        .bind(payment.id)
        .bind(entity_id)
        .bind(&receipt)
        .execute(engine.pool())
        .await;
    }

    Ok(payment)
}

/// Reconstruct a `Payment` domain object from its database row.
fn payment_from_row(row: PaymentRow) -> Payment {
    let payment_type: PaymentType =
        serde_json::from_str(&format!("\"{}\"", row.payment_type))
            .unwrap_or(PaymentType::CustomerPayment);
    let applications: Vec<PaymentApplication> =
        serde_json::from_value(row.applications.clone()).unwrap_or_default();

    Payment {
        id: row.id,
        entity_id: row.entity_id,
        number: row.number,
        payment_type,
        party_id: row.party_id,
        payment_date: row.payment_date,
        amount: row.amount,
        currency: row.currency,
        fx_rate: row.fx_rate,
        method: serde_json::from_value(row.method).unwrap_or(PaymentMethod::Cash),
        reference: row.reference,
        bank_account_id: row.bank_account_id,
        applications,
        unapplied: row.unapplied,
        journal_entry_id: row.journal_entry_id,
        status: PaymentStatus::Completed,
        created_at: row.created_at,
    }
}

/// Apply unapplied funds from an existing payment to a target document (invoice or bill).
///
/// Requirements 24.2, 24.3, 24.4, 24.5:
/// - Reduces the payment's unapplied balance and the target document's balance_due.
/// - Creates a JE: DR Unapplied Payments (3050) / CR AR (1200) or AP (3010).
/// - Rejects if the apply amount exceeds the payment's unapplied balance.
/// - Records an audit event with before/after amounts.
pub async fn apply_unapplied_payment(
    engine: &ErpEngine,
    entity_id: Uuid,
    req: ApplyPaymentRequest,
    actor: &AgentOrUserId,
) -> ErpResult<Payment> {
    // 1. Fetch the payment record
    let row = sqlx::query_as::<_, PaymentRow>(
        "SELECT * FROM payments WHERE id = $1 AND entity_id = $2",
    )
    .bind(req.payment_id)
    .bind(entity_id)
    .fetch_optional(engine.pool())
    .await?
    .ok_or_else(|| ErpError::NotFound {
        entity_type: "Payment".to_string(),
        id: req.payment_id,
    })?;

    let current_unapplied = row.unapplied;
    let payment_type: PaymentType = serde_json::from_str(&format!("\"{}\"", row.payment_type))
        .unwrap_or(PaymentType::CustomerPayment);

    // 2. Validate: reject if apply amount exceeds unapplied balance (Requirement 24.4)
    if req.amount > current_unapplied {
        return Err(ErpError::ValidationFailed {
            message: format!(
                "Apply amount {} exceeds unapplied balance {}",
                req.amount, current_unapplied
            ),
        });
    }

    if req.amount <= Decimal::ZERO {
        return Err(ErpError::ValidationFailed {
            message: "Apply amount must be positive".to_string(),
        });
    }

    // 3. Fetch target document's current balance_due
    let doc_balance = fetch_document_balance(engine, entity_id, req.document_id, &payment_type).await?;

    // Cap application at document's balance_due
    let effective_apply = req.amount.min(doc_balance);

    if effective_apply <= Decimal::ZERO {
        return Err(ErpError::ValidationFailed {
            message: "Target document has no outstanding balance".to_string(),
        });
    }

    // 4. Reduce payment's unapplied balance (Requirement 24.2)
    let new_unapplied = current_unapplied - effective_apply;

    // Add new application to the payment's applications array
    let new_application = PaymentApplication {
        document_id: req.document_id,
        document_type: match payment_type {
            PaymentType::CustomerPayment => PaymentDocType::Invoice,
            PaymentType::VendorPayment => PaymentDocType::Bill,
        },
        amount_applied: effective_apply,
    };

    // Parse existing applications and append
    let mut applications: Vec<PaymentApplication> =
        serde_json::from_value(row.applications.clone()).unwrap_or_default();
    applications.push(new_application);

    // Allocation record, balance transfer, and journal entry commit together
    // or roll back together (Requirement 2.4).
    let mut tx = engine.pool().begin().await?;

    sqlx::query(
        "UPDATE payments SET unapplied = $1, applications = $2 WHERE id = $3",
    )
    .bind(new_unapplied)
    .bind(serde_json::to_value(&applications).unwrap_or_default())
    .bind(req.payment_id)
    .execute(&mut *tx)
    .await?;

    // 5. Reduce target document's balance_due (Requirement 24.2)
    match payment_type {
        PaymentType::CustomerPayment => {
            sqlx::query(
                "UPDATE invoices SET amount_paid = amount_paid + $1, balance_due = balance_due - $1 WHERE id = $2",
            )
            .bind(effective_apply)
            .bind(req.document_id)
            .execute(&mut *tx)
            .await?;

            // Update invoice status
            sqlx::query(
                "UPDATE invoices SET status = CASE WHEN balance_due <= 0 THEN 'paid' ELSE 'partially_paid' END, paid_at = CASE WHEN balance_due <= 0 THEN NOW() ELSE paid_at END WHERE id = $1",
            )
            .bind(req.document_id)
            .execute(&mut *tx)
            .await?;
        }
        PaymentType::VendorPayment => {
            sqlx::query(
                "UPDATE bills SET amount_paid = amount_paid + $1, balance_due = balance_due - $1 WHERE id = $2",
            )
            .bind(effective_apply)
            .bind(req.document_id)
            .execute(&mut *tx)
            .await?;

            sqlx::query(
                "UPDATE bills SET status = CASE WHEN balance_due <= 0 THEN 'paid' ELSE 'partially_paid' END WHERE id = $1",
            )
            .bind(req.document_id)
            .execute(&mut *tx)
            .await?;
        }
    }

    // 6. Create JE: DR Unapplied Payments / CR AR or AP (Requirement 24.3)
    let acct = PaymentAccounts::resolve(engine, entity_id).await?;
    let receivable_payable_code = match payment_type {
        PaymentType::CustomerPayment => acct.ar.clone(),
        PaymentType::VendorPayment => acct.ap.clone(),
    };

    let currency = row.currency.clone();
    let fx_rate = row.fx_rate;
    let payment_date = row.payment_date;

    let je_lines = vec![
        CreateJournalLineRequest {
            account_code: acct.unapplied_payments.clone(),
            debit: Some(effective_apply),
            credit: None,
            currency: currency.clone(),
            fx_rate: Some(fx_rate),
            description: Some(format!(
                "Apply unapplied funds from payment {} to document",
                row.number
            )),
            dimensions: None,
        },
        CreateJournalLineRequest {
            account_code: receivable_payable_code.to_string(),
            debit: None,
            credit: Some(effective_apply),
            currency: currency.clone(),
            fx_rate: Some(fx_rate),
            description: Some(format!(
                "Unapplied payment allocation: {}",
                row.number
            )),
            dimensions: None,
        },
    ];

    let je_req = CreateJournalEntryRequest {
        date: payment_date,
        source: JournalSource::Payment,
        source_id: None,
        reference: format!("{}-APPLY", row.number),
        description: format!(
            "Apply unapplied funds ({}) from payment {} to document",
            effective_apply, row.number
        ),
        lines: je_lines,
        post_immediately: true,
    };

    let period = crate::services::periods::period_for_date(engine, entity_id, payment_date).await?;
    let _entry = crate::services::journal::create_and_post_in_tx(
        &mut tx,
        engine,
        entity_id,
        je_req,
        period.id,
        actor.clone(),
    )
    .await?;

    tx.commit().await?;

    // Cancel pending reminders for a paid-down customer invoice (post-commit).
    if payment_type == PaymentType::CustomerPayment {
        let updated_balance = fetch_document_balance(engine, entity_id, req.document_id, &payment_type)
            .await
            .unwrap_or(Decimal::ZERO);
        let _ = crate::services::scheduler::cancel_reminders_on_payment(
            engine,
            entity_id,
            req.document_id,
            updated_balance,
        )
        .await;
    }

    // 7. Record audit event with before/after amounts (Requirement 24.5)
    let audit_event = serde_json::json!({
        "event_type": "Updated",
        "object_type": "payment",
        "object_id": req.payment_id,
        "actor": actor,
        "action": "apply_unapplied",
        "document_id": req.document_id,
        "amount_applied": effective_apply,
        "before": {
            "unapplied_balance": current_unapplied,
            "document_balance_due": doc_balance,
        },
        "after": {
            "unapplied_balance": new_unapplied,
            "document_balance_due": doc_balance - effective_apply,
        },
        "timestamp": Utc::now(),
    });
    let stream_key = format!("erp:audit:{}", entity_id);
    let mut redis_conn = engine.redis_conn().await;
    let _: Result<(), _> = redis::cmd("XADD")
        .arg(&stream_key)
        .arg("*")
        .arg("data")
        .arg(audit_event.to_string())
        .query_async(&mut redis_conn)
        .await;

    // Return updated payment
    Ok(Payment {
        id: row.id,
        entity_id: row.entity_id,
        number: row.number,
        payment_type,
        party_id: row.party_id,
        payment_date: row.payment_date,
        amount: row.amount,
        currency: row.currency,
        fx_rate: row.fx_rate,
        method: serde_json::from_value(row.method).unwrap_or(PaymentMethod::Cash),
        reference: row.reference,
        bank_account_id: row.bank_account_id,
        applications,
        unapplied: new_unapplied,
        journal_entry_id: row.journal_entry_id,
        status: PaymentStatus::Completed,
        created_at: row.created_at,
    })
}

async fn generate_payment_number(engine: &ErpEngine, entity_id: Uuid) -> ErpResult<String> {
    let row = sqlx::query_scalar::<_, i64>(
        r#"UPDATE entity_settings 
           SET sequences = jsonb_set(sequences, '{payment_next}', to_jsonb((sequences->>'payment_next')::bigint + 1))
           WHERE entity_id = $1
           RETURNING (sequences->>'payment_next')::bigint - 1"#,
    )
    .bind(entity_id)
    .fetch_one(engine.pool())
    .await?;

    let cfg = engine.config_for(entity_id).await?;
    let prefix = &cfg.sequences.payment_prefix;
    let fiscal_year = Utc::now().format("%Y").to_string();
    Ok(format!("{}-{}-{:04}", prefix, fiscal_year, row))
}


/// Fetch the FX rate at which a document (invoice or bill) was originally recorded.
/// This is needed to compute realised FX gain/loss when the payment rate differs.
async fn fetch_document_fx_rate(
    engine: &ErpEngine,
    entity_id: Uuid,
    document_id: Uuid,
    payment_type: &PaymentType,
) -> ErpResult<Decimal> {
    match payment_type {
        PaymentType::CustomerPayment => {
            sqlx::query_scalar::<_, Decimal>(
                "SELECT fx_rate FROM invoices WHERE id = $1 AND entity_id = $2",
            )
            .bind(document_id)
            .bind(entity_id)
            .fetch_optional(engine.pool())
            .await?
            .ok_or_else(|| ErpError::NotFound {
                entity_type: "Invoice".to_string(),
                id: document_id,
            })
        }
        PaymentType::VendorPayment => {
            sqlx::query_scalar::<_, Decimal>(
                "SELECT fx_rate FROM bills WHERE id = $1 AND entity_id = $2",
            )
            .bind(document_id)
            .bind(entity_id)
            .fetch_optional(engine.pool())
            .await?
            .ok_or_else(|| ErpError::NotFound {
                entity_type: "Bill".to_string(),
                id: document_id,
            })
        }
    }
}

/// Post a realised FX gain/loss journal entry for a cross-currency payment application.
///
/// Calculation (Requirements 22.2, 22.3, 22.5):
/// - applied_functional_at_invoice_rate = applied_amount × invoice_fx_rate
/// - applied_functional_at_payment_rate = applied_amount × payment_fx_rate
/// - fx_difference = applied_functional_at_payment_rate - applied_functional_at_invoice_rate
///
/// If fx_difference > 0 → Realised FX Gain:
///   DR AR/AP (difference) / CR 8120 Realised FX Gain
///
/// If fx_difference < 0 → Realised FX Loss:
///   DR 8130 Realised FX Loss / CR AR/AP (abs difference)
async fn post_fx_gain_loss_entry(
    engine: &ErpEngine,
    entity_id: Uuid,
    payment_number: &str,
    payment_date: chrono::NaiveDate,
    _currency: &str,
    payment_fx_rate: Decimal,
    invoice_fx_rate: Decimal,
    applied_amount: Decimal,
    payment_type: &PaymentType,
    posted_by: &AgentOrUserId,
) -> ErpResult<Uuid> {
    // Round each functional-currency conversion to 2dp independently before
    // differencing, mirroring the journal posting policy (Req 5.1). This keeps
    // the realised FX gain/loss a true 2-decimal monetary value at source rather
    // than relying solely on the ledger layer to round it.
    let functional_at_invoice_rate = round_money(applied_amount * invoice_fx_rate);
    let functional_at_payment_rate = round_money(applied_amount * payment_fx_rate);
    let fx_difference = functional_at_payment_rate - functional_at_invoice_rate;

    // Determine the AR/AP account for the offsetting entry
    let acct = PaymentAccounts::resolve(engine, entity_id).await?;
    let ar_ap_code = match payment_type {
        PaymentType::CustomerPayment => acct.ar.clone(),
        PaymentType::VendorPayment => acct.ap.clone(),
    };

    let abs_difference = fx_difference.abs();
    let base_currency = engine.config_for(entity_id).await?.base_currency.clone();

    let mut lines: Vec<CreateJournalLineRequest> = Vec::new();

    if fx_difference > Decimal::ZERO {
        // FX Gain: DR AR/AP, CR 8120 Realised FX Gain
        lines.push(CreateJournalLineRequest {
            account_code: ar_ap_code.to_string(),
            debit: Some(abs_difference),
            credit: None,
            currency: base_currency.clone(),
            fx_rate: Some(Decimal::ONE),
            description: Some(format!(
                "FX gain adjustment on {}: {} @ {} vs {}",
                payment_number, applied_amount, payment_fx_rate, invoice_fx_rate
            )),
            dimensions: None,
        });
        lines.push(CreateJournalLineRequest {
            account_code: acct.realised_fx_gain.clone(),
            debit: None,
            credit: Some(abs_difference),
            currency: base_currency.clone(),
            fx_rate: Some(Decimal::ONE),
            description: Some(format!(
                "Realised FX gain on payment {}",
                payment_number
            )),
            dimensions: None,
        });
    } else {
        // FX Loss: DR 8130 Realised FX Loss, CR AR/AP
        lines.push(CreateJournalLineRequest {
            account_code: acct.realised_fx_loss.clone(),
            debit: Some(abs_difference),
            credit: None,
            currency: base_currency.clone(),
            fx_rate: Some(Decimal::ONE),
            description: Some(format!(
                "Realised FX loss on payment {}",
                payment_number
            )),
            dimensions: None,
        });
        lines.push(CreateJournalLineRequest {
            account_code: ar_ap_code.to_string(),
            debit: None,
            credit: Some(abs_difference),
            currency: base_currency.clone(),
            fx_rate: Some(Decimal::ONE),
            description: Some(format!(
                "FX loss adjustment on {}: {} @ {} vs {}",
                payment_number, applied_amount, payment_fx_rate, invoice_fx_rate
            )),
            dimensions: None,
        });
    }

    let description = if fx_difference > Decimal::ZERO {
        format!(
            "Realised FX gain {} on payment {} (rate {} vs invoice rate {})",
            abs_difference, payment_number, payment_fx_rate, invoice_fx_rate
        )
    } else {
        format!(
            "Realised FX loss {} on payment {} (rate {} vs invoice rate {})",
            abs_difference, payment_number, payment_fx_rate, invoice_fx_rate
        )
    };

    let je_req = CreateJournalEntryRequest {
        date: payment_date,
        source: JournalSource::Payment,
        source_id: None,
        reference: format!("{}-FX", payment_number),
        description,
        lines,
        post_immediately: true,
    };

    let period = crate::services::periods::period_for_date(engine, entity_id, payment_date).await?;

    let entry = crate::services::journal::create_and_post(
        engine,
        entity_id,
        je_req,
        period.id,
        posted_by.clone(),
    )
    .await?;

    Ok(entry.id)
}
