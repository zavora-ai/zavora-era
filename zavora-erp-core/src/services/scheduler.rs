use chrono::Utc;
use rust_decimal::Decimal;
use uuid::Uuid;

use crate::engine::ErpEngine;
use crate::error::{ErpError, ErpResult};
use crate::invoicing::RecurringInvoiceRow;
use crate::types::Channel;

#[derive(Debug, sqlx::FromRow)]
struct ReportScheduleRow {
    id: Uuid,
    name: String,
    report_type: String,
    cadence: String,
    recipients: String,
}

/// Run any report schedules that are due for this entity: generate the report,
/// queue it (as CSV) to each recipient via the notification outbox, and advance
/// next_run_at by the cadence. Returns the number of schedules run.
pub async fn process_report_schedules(engine: &ErpEngine) -> ErpResult<u32> {
    use crate::reporting::{ReportParameters, ReportRequest, ReportType};

    let now = Utc::now();
    let due = sqlx::query_as::<_, ReportScheduleRow>(
        "SELECT id, name, report_type, cadence, recipients FROM report_schedules
         WHERE entity_id = $1 AND is_active = true AND (next_run_at IS NULL OR next_run_at <= $2)",
    )
    .bind(engine.entity_id())
    .bind(now)
    .fetch_all(engine.pool())
    .await?;

    let mut count = 0u32;
    for s in due {
        // Stored report_type is the enum variant name, e.g. "TrialBalance".
        let Ok(report_type) = serde_json::from_str::<ReportType>(&format!("\"{}\"", s.report_type)) else {
            tracing::warn!("Report schedule {} has unknown report_type {}", s.id, s.report_type);
            continue;
        };
        let req = ReportRequest {
            entity_id: engine.entity_id(),
            report_type,
            parameters: ReportParameters {
                as_at: None, period_from: None, period_to: None, compare_to: None,
                comparative: None, customer_id: None, vendor_id: None, account_code: None,
                bank_account_id: None, statement_id: None, period_id: None, dimension_type: None,
            },
        };

        let report = match crate::services::reporting::generate_report(engine, req).await {
            Ok(r) => r,
            Err(e) => { tracing::error!("Scheduled report {} failed: {}", s.id, e); continue; }
        };
        let csv = crate::services::reporting::export_to_csv(&report).unwrap_or_default();

        let recipients: Vec<String> = s.recipients.split(',').map(|r| r.trim().to_string()).filter(|r| !r.is_empty()).collect();
        if !recipients.is_empty() {
            let req = crate::notifications::SendNotificationRequest {
                event_type: crate::notifications::NotificationEventType::ScheduledReport,
                channels: vec![crate::types::Channel::Email],
                recipients,
                subject: Some(format!("{} — {}", s.name, now.date_naive())),
                body: String::from_utf8_lossy(&csv).to_string(),
                related_type: Some("report_schedule".to_string()),
                related_id: Some(s.id),
                schedule_at: None,
            };
            let _ = crate::services::notifications::send_notification(engine, engine.entity_id(), req).await;
        }

        let next = match s.cadence.as_str() {
            "daily" => now + chrono::Duration::days(1),
            "weekly" => now + chrono::Duration::days(7),
            _ => now.checked_add_months(chrono::Months::new(1)).unwrap_or(now + chrono::Duration::days(30)),
        };
        sqlx::query("UPDATE report_schedules SET last_run_at = $1, next_run_at = $2 WHERE id = $3")
            .bind(now).bind(next).bind(s.id)
            .execute(engine.pool())
            .await?;
        count += 1;
    }

    Ok(count)
}

/// Process all recurring invoices that are due today or earlier.
/// Creates invoices for each due recurring template.
pub async fn process_recurring_invoices(engine: &ErpEngine) -> ErpResult<Vec<Uuid>> {
    let today = Utc::now().date_naive();

    // Fetch all active recurring invoices where next_run <= today
    let due = sqlx::query_as::<_, RecurringInvoiceRow>(
        "SELECT * FROM recurring_invoices WHERE entity_id = $1 AND is_active = true AND next_run <= $2",
    )
    .bind(engine.entity_id())
    .bind(today)
    .fetch_all(engine.pool())
    .await?;

    let mut created_ids = Vec::new();
    let actor = crate::types::AgentOrUserId::Agent("recurring-scheduler".to_string());

    for rec in &due {
        // Deserialize the template into CreateInvoiceRequest
        let template: crate::invoicing::CreateInvoiceRequest =
            serde_json::from_value(rec.template.clone()).unwrap_or_else(|_| {
                crate::invoicing::CreateInvoiceRequest {
                    customer_id: rec.customer_id,
                    issue_date: Some(today),
                    due_date: None,
                    currency: None,
                    fx_rate: None,
                    lines: Vec::new(),
                    template_id: None,
                    notes: None,
                    send_immediately: None,
                }
            });

        // Create invoice from template
        match crate::services::invoicing::create_invoice(engine, rec.entity_id, template, &actor).await {
            Ok(invoice) => {
                created_ids.push(invoice.id);

                // If auto_send, post it
                if rec.auto_send {
                    let _ =
                        crate::services::invoicing::post_invoice(engine, rec.entity_id, invoice.id, &actor).await;
                }

                // Advance the next_run date
                let frequency: crate::invoicing::RecurrenceFreq =
                    serde_json::from_str(&format!("\"{}\"", rec.frequency))
                        .unwrap_or(crate::invoicing::RecurrenceFreq::Monthly);
                let next_run = frequency.next_date(today);

                sqlx::query(
                    "UPDATE recurring_invoices SET next_run = $1, last_run = $2, run_count = run_count + 1 WHERE id = $3",
                )
                .bind(next_run)
                .bind(today)
                .bind(rec.id)
                .execute(engine.pool())
                .await?;

                // Deactivate if past end_date
                if let Some(end) = rec.end_date {
                    if next_run > end {
                        sqlx::query(
                            "UPDATE recurring_invoices SET is_active = false WHERE id = $1",
                        )
                        .bind(rec.id)
                        .execute(engine.pool())
                        .await?;
                    }
                }
            }
            Err(e) => {
                tracing::error!("Failed to create recurring invoice {}: {}", rec.id, e);
            }
        }
    }

    Ok(created_ids)
}

/// Process invoice payment reminders.
/// Checks all unpaid invoices and sends reminders based on customer reminder policies.
pub async fn process_invoice_reminders(engine: &ErpEngine) -> ErpResult<u32> {
    let today = Utc::now().date_naive();
    let mut sent_count = 0u32;

    // Find invoices that are unpaid and have reminder policies
    let unpaid = sqlx::query_as::<_, UnpaidInvoiceRow>(
        r#"SELECT i.id, i.number, i.customer_id, i.due_date, i.balance_due, i.status,
               c.name as customer_name, c.reminder_policy
           FROM invoices i
           JOIN customers c ON c.id = i.customer_id
           WHERE i.entity_id = $1
             AND i.status IN ('posted', 'sent', 'viewed', 'overdue', 'partially_paid')
             AND i.balance_due > 0"#,
    )
    .bind(engine.entity_id())
    .fetch_all(engine.pool())
    .await?;

    for inv in &unpaid {
        // Parse reminder policy
        let policy: crate::parties::ReminderPolicy =
            serde_json::from_value(inv.reminder_policy.clone()).unwrap_or_default();

        for rule in &policy.reminders {
            // Check if today matches this rule's offset from due_date
            let reminder_date = inv.due_date + chrono::Duration::days(rule.offset_days as i64);
            if reminder_date == today {
                // Queue notification
                let req = crate::notifications::SendNotificationRequest {
                    event_type: crate::notifications::NotificationEventType::InvoiceReminder,
                    channels: rule.channels.clone(),
                    recipients: vec![inv.customer_name.clone()],
                    subject: Some(format!("Payment Reminder: Invoice {}", inv.number)),
                    body: format!(
                        "This is a reminder that invoice {} for KES {} is {}.",
                        inv.number,
                        inv.balance_due,
                        if today > inv.due_date {
                            "overdue"
                        } else {
                            "due soon"
                        }
                    ),
                    related_type: Some("Invoice".to_string()),
                    related_id: Some(inv.id),
                    schedule_at: None,
                };

                let _ = crate::services::notifications::send_notification(engine, engine.entity_id(), req).await;
                sent_count += 1;
            }
        }
    }

    // Mark overdue invoices
    sqlx::query(
        "UPDATE invoices SET status = 'overdue' WHERE entity_id = $1 AND status IN ('posted', 'sent', 'viewed') AND due_date < $2 AND balance_due > 0",
    )
    .bind(engine.entity_id())
    .bind(today)
    .execute(engine.pool())
    .await?;

    Ok(sent_count)
}

#[derive(Debug, sqlx::FromRow)]
#[allow(dead_code)]
struct UnpaidInvoiceRow {
    id: uuid::Uuid,
    number: String,
    customer_id: uuid::Uuid,
    due_date: chrono::NaiveDate,
    balance_due: rust_decimal::Decimal,
    status: String,
    customer_name: String,
    reminder_policy: serde_json::Value,
}

/// Row representing an overdue-eligible invoice with customer contact details.
#[derive(Debug, sqlx::FromRow)]
#[allow(dead_code)]
struct OverdueInvoiceRow {
    id: Uuid,
    number: String,
    customer_id: Uuid,
    due_date: chrono::NaiveDate,
    balance_due: Decimal,
    status: String,
    customer_name: String,
    customer_email: serde_json::Value,
    customer_phone: serde_json::Value,
    reminder_policy: serde_json::Value,
}

/// Summary of an overdue check run.
#[derive(Debug, Clone)]
pub struct OverdueCheckResult {
    /// Number of invoices transitioned to Overdue status.
    pub transitioned_count: u32,
    /// Number of reminder notifications queued.
    pub reminders_sent: u32,
    /// Number of channels skipped due to missing delivery address.
    pub channels_skipped: u32,
}

/// Run the overdue detection and reminder delivery job.
///
/// This function:
/// 1. Queries invoices past due with outstanding balance and status in (Sent, Viewed, PartiallyPaid)
/// 2. Transitions matching invoices to Overdue status
/// 3. For each overdue invoice, evaluates the customer's Reminder_Policy
/// 4. When a reminder rule's trigger date (due_date + offset_days) matches today, delivers via specified channels
/// 5. Skips channels where the customer has no valid delivery address, logging a warning
/// 6. Records an audit event for each reminder delivery attempt
pub async fn run_overdue_check(engine: &ErpEngine) -> ErpResult<OverdueCheckResult> {
    let today = Utc::now().date_naive();
    let mut result = OverdueCheckResult {
        transitioned_count: 0,
        reminders_sent: 0,
        channels_skipped: 0,
    };

    // Step 1: Query invoices that are past due with outstanding balance
    // Status must be sent, viewed, or partially_paid (not already overdue or paid)
    let overdue_invoices = sqlx::query_as::<_, OverdueInvoiceRow>(
        r#"SELECT i.id, i.number, i.customer_id, i.due_date, i.balance_due, i.status,
               c.name as customer_name, c.email as customer_email,
               c.phone as customer_phone, c.reminder_policy
           FROM invoices i
           JOIN customers c ON c.id = i.customer_id
           WHERE i.entity_id = $1
             AND i.due_date < $2
             AND i.balance_due > 0
             AND i.status IN ('posted', 'sent', 'viewed', 'partially_paid')"#,
    )
    .bind(engine.entity_id())
    .bind(today)
    .fetch_all(engine.pool())
    .await?;

    // Step 2: Transition matching invoices to Overdue status
    for inv in &overdue_invoices {
        sqlx::query("UPDATE invoices SET status = 'overdue' WHERE id = $1")
            .bind(inv.id)
            .execute(engine.pool())
            .await?;
        result.transitioned_count += 1;

        tracing::info!(
            invoice_id = %inv.id,
            invoice_number = %inv.number,
            customer = %inv.customer_name,
            "Invoice transitioned to Overdue status"
        );
    }

    // Step 3 & 4: Evaluate reminder policies and deliver reminders
    // Query ALL overdue invoices (including those already in overdue status) for reminder evaluation
    let all_overdue = sqlx::query_as::<_, OverdueInvoiceRow>(
        r#"SELECT i.id, i.number, i.customer_id, i.due_date, i.balance_due, i.status,
               c.name as customer_name, c.email as customer_email,
               c.phone as customer_phone, c.reminder_policy
           FROM invoices i
           JOIN customers c ON c.id = i.customer_id
           WHERE i.entity_id = $1
             AND i.status = 'overdue'
             AND i.balance_due > 0"#,
    )
    .bind(engine.entity_id())
    .fetch_all(engine.pool())
    .await?;

    let actor = crate::types::AgentOrUserId::Agent("overdue-scheduler".to_string());

    for inv in &all_overdue {
        // Parse the customer's reminder policy
        let policy: crate::parties::ReminderPolicy =
            serde_json::from_value(inv.reminder_policy.clone()).unwrap_or_default();

        // Parse customer contact details for channel validation
        let emails: Vec<crate::types::ContactEmail> =
            serde_json::from_value(inv.customer_email.clone()).unwrap_or_default();
        let phones: Vec<crate::types::ContactPhone> =
            serde_json::from_value(inv.customer_phone.clone()).unwrap_or_default();

        for rule in &policy.reminders {
            // Check if today matches this rule's offset from due_date
            let reminder_date = inv.due_date + chrono::Duration::days(rule.offset_days as i64);
            if reminder_date != today {
                continue;
            }

            // Filter channels based on valid delivery addresses
            let mut valid_channels: Vec<Channel> = Vec::new();
            for channel in &rule.channels {
                match channel {
                    Channel::Email => {
                        if emails.iter().any(|e| !e.email.is_empty()) {
                            valid_channels.push(Channel::Email);
                        } else {
                            tracing::warn!(
                                invoice_id = %inv.id,
                                customer = %inv.customer_name,
                                channel = "Email",
                                "Skipping channel: no valid email address for customer"
                            );
                            result.channels_skipped += 1;
                        }
                    }
                    Channel::WhatsApp => {
                        if phones.iter().any(|p| p.whatsapp_enabled && !p.number.is_empty()) {
                            valid_channels.push(Channel::WhatsApp);
                        } else {
                            tracing::warn!(
                                invoice_id = %inv.id,
                                customer = %inv.customer_name,
                                channel = "WhatsApp",
                                "Skipping channel: no WhatsApp-enabled phone number for customer"
                            );
                            result.channels_skipped += 1;
                        }
                    }
                    Channel::Sms => {
                        if phones.iter().any(|p| !p.number.is_empty()) {
                            valid_channels.push(Channel::Sms);
                        } else {
                            tracing::warn!(
                                invoice_id = %inv.id,
                                customer = %inv.customer_name,
                                channel = "SMS",
                                "Skipping channel: no valid phone number for customer"
                            );
                            result.channels_skipped += 1;
                        }
                    }
                    Channel::InApp => {
                        // InApp is always deliverable
                        valid_channels.push(Channel::InApp);
                    }
                }
            }

            // Skip if no valid channels remain
            if valid_channels.is_empty() {
                tracing::warn!(
                    invoice_id = %inv.id,
                    customer = %inv.customer_name,
                    "No valid delivery channels available for reminder; skipping"
                );
                continue;
            }

            // Deliver reminder via valid channels
            let notification_req = crate::notifications::SendNotificationRequest {
                event_type: crate::notifications::NotificationEventType::InvoiceReminder,
                channels: valid_channels.clone(),
                recipients: vec![inv.customer_name.clone()],
                subject: Some(format!("Payment Reminder: Invoice {}", inv.number)),
                body: format!(
                    "This is a reminder that invoice {} for {} is overdue. Amount due: {}. Due date was {}.",
                    inv.number,
                    inv.customer_name,
                    inv.balance_due,
                    inv.due_date,
                ),
                related_type: Some("Invoice".to_string()),
                related_id: Some(inv.id),
                schedule_at: None,
            };

            let delivery_outcome =
                crate::services::notifications::send_notification(engine, engine.entity_id(), notification_req).await;

            // Record audit event for the reminder delivery attempt
            let outcome_str = match &delivery_outcome {
                Ok(()) => "queued",
                Err(e) => {
                    tracing::error!(
                        invoice_id = %inv.id,
                        error = %e,
                        "Failed to queue reminder notification"
                    );
                    "failed"
                }
            };

            let audit_event = serde_json::json!({
                "event_type": "reminder_sent",
                "object_type": "invoice",
                "object_id": inv.id,
                "actor": actor,
                "invoice_number": inv.number,
                "customer_id": inv.customer_id,
                "customer_name": inv.customer_name,
                "channels": valid_channels,
                "offset_days": rule.offset_days,
                "outcome": outcome_str,
                "timestamp": Utc::now(),
            });
            let stream_key = format!("erp:audit:{}", engine.entity_id());
            let mut redis_conn = engine.redis_conn().await;
            let _: Result<(), _> = redis::cmd("XADD")
                .arg(&stream_key)
                .arg("*")
                .arg("data")
                .arg(audit_event.to_string())
                .query_async(&mut redis_conn)
                .await;

            if delivery_outcome.is_ok() {
                result.reminders_sent += 1;
            }
        }
    }

    tracing::info!(
        transitioned = result.transitioned_count,
        reminders = result.reminders_sent,
        skipped_channels = result.channels_skipped,
        "Overdue check completed"
    );

    Ok(result)
}


/// Result of the cancel-reminders-on-payment operation.
#[derive(Debug, Clone)]
pub struct CancelRemindersResult {
    /// Whether the invoice was in Overdue status before the payment.
    pub was_overdue: bool,
    /// Number of pending reminders cancelled (notifications + Redis stream entries).
    pub reminders_cancelled: u32,
    /// The new invoice status after the payment (paid or partially_paid).
    pub new_status: String,
}

/// Cancel pending reminders when a payment is received on an overdue invoice.
///
/// This function implements Requirement 5.4:
/// - When a payment clears (or partially clears) an overdue invoice's balance:
///   1. Check if the invoice was in Overdue status
///   2. Cancel all pending/queued reminders for that invoice (in notifications table)
///   3. Update invoice status based on new balance (paid if 0, partially_paid otherwise)
///   4. Record an audit event for the cancellation
///
/// This should be called from the payments service after a payment application
/// reduces an overdue invoice's balance.
pub async fn cancel_reminders_on_payment(
    engine: &ErpEngine,
    entity_id: Uuid,
    invoice_id: Uuid,
    new_balance: Decimal,
) -> ErpResult<CancelRemindersResult> {
    // Step 1: Check if the invoice was in Overdue status
    let invoice_status = sqlx::query_scalar::<_, String>(
        "SELECT status FROM invoices WHERE id = $1 AND entity_id = $2",
    )
    .bind(invoice_id)
    .bind(entity_id)
    .fetch_optional(engine.pool())
    .await?
    .ok_or_else(|| ErpError::NotFound {
        entity_type: "Invoice".to_string(),
        id: invoice_id,
    })?;

    if invoice_status != "overdue" {
        return Ok(CancelRemindersResult {
            was_overdue: false,
            reminders_cancelled: 0,
            new_status: invoice_status,
        });
    }

    // Step 2: Cancel all pending/queued notification reminders for this invoice
    let cancelled = sqlx::query_scalar::<_, i64>(
        r#"WITH updated AS (
            UPDATE notifications
            SET status = 'cancelled'
            WHERE entity_id = $1
              AND related_type = 'Invoice'
              AND related_id = $2
              AND event_type = 'InvoiceReminder'
              AND status IN ('queued', 'pending')
            RETURNING 1
        )
        SELECT COUNT(*) FROM updated"#,
    )
    .bind(entity_id)
    .bind(invoice_id)
    .fetch_one(engine.pool())
    .await
    .unwrap_or(0);

    // Step 3: Transition invoice status from Overdue to Paid or PartiallyPaid
    let new_status = if new_balance <= Decimal::ZERO {
        "paid"
    } else {
        "partially_paid"
    };

    sqlx::query(
        "UPDATE invoices SET status = $1, paid_at = CASE WHEN $1 = 'paid' THEN NOW() ELSE paid_at END WHERE id = $2 AND entity_id = $3",
    )
    .bind(new_status)
    .bind(invoice_id)
    .bind(entity_id)
    .execute(engine.pool())
    .await?;

    // Step 4: Record an audit event for the reminder cancellation
    let audit_event = serde_json::json!({
        "event_type": "reminders_cancelled",
        "object_type": "invoice",
        "object_id": invoice_id,
        "actor": "payment-engine",
        "previous_status": "overdue",
        "new_status": new_status,
        "new_balance": new_balance.to_string(),
        "reminders_cancelled": cancelled,
        "reason": "payment_received",
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

    tracing::info!(
        invoice_id = %invoice_id,
        previous_status = "overdue",
        new_status = new_status,
        reminders_cancelled = cancelled,
        "Cancelled pending reminders on payment for overdue invoice"
    );

    Ok(CancelRemindersResult {
        was_overdue: true,
        reminders_cancelled: cancelled as u32,
        new_status: new_status.to_string(),
    })
}
