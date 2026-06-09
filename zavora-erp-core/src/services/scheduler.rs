use chrono::Utc;
use uuid::Uuid;

use crate::engine::ErpEngine;
use crate::error::ErpResult;
use crate::invoicing::RecurringInvoiceRow;

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
        match crate::services::invoicing::create_invoice(engine, template, &actor).await {
            Ok(invoice) => {
                created_ids.push(invoice.id);

                // If auto_send, post it
                if rec.auto_send {
                    let _ =
                        crate::services::invoicing::post_invoice(engine, invoice.id, &actor).await;
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
             AND i.status IN ('sent', 'viewed', 'overdue', 'partially_paid')
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

                let _ = crate::services::notifications::send_notification(engine, req).await;
                sent_count += 1;
            }
        }
    }

    // Mark overdue invoices
    sqlx::query(
        "UPDATE invoices SET status = 'overdue' WHERE entity_id = $1 AND status IN ('sent', 'viewed') AND due_date < $2 AND balance_due > 0",
    )
    .bind(engine.entity_id())
    .bind(today)
    .execute(engine.pool())
    .await?;

    Ok(sent_count)
}

#[derive(Debug, sqlx::FromRow)]
struct UnpaidInvoiceRow {
    id: uuid::Uuid,
    number: String,
    #[allow(dead_code)]
    customer_id: uuid::Uuid,
    due_date: chrono::NaiveDate,
    balance_due: rust_decimal::Decimal,
    #[allow(dead_code)]
    status: String,
    customer_name: String,
    reminder_policy: serde_json::Value,
}
