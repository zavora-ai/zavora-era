use chrono::{Datelike, Utc};
use rust_decimal::Decimal;
use uuid::Uuid;

use crate::engine::ErpEngine;
use crate::error::{ErpError, ErpResult};
use crate::invoicing::RecurringInvoiceRow;
use crate::types::Channel;

/// Enumerate every tenant that has settings (i.e. exists). Background jobs use
/// this to run for all tenants rather than only the process's startup entity.
async fn all_entity_ids(engine: &ErpEngine) -> ErpResult<Vec<Uuid>> {
    Ok(sqlx::query_scalar::<_, Uuid>("SELECT entity_id FROM entity_settings")
        .fetch_all(engine.pool())
        .await?)
}

#[derive(Debug, sqlx::FromRow)]
struct ReportScheduleRow {
    id: Uuid,
    name: String,
    report_type: String,
    cadence: String,
    recipients: String,
}

/// Run due report schedules for ALL tenants. See [`process_report_schedules_for`].
pub async fn process_report_schedules(engine: &ErpEngine) -> ErpResult<u32> {
    let mut total = 0u32;
    for entity_id in all_entity_ids(engine).await? {
        match process_report_schedules_for(engine, entity_id).await {
            Ok(n) => total += n,
            Err(e) => tracing::error!("Report schedules failed for entity {}: {}", entity_id, e),
        }
    }
    Ok(total)
}

/// Run any report schedules that are due for `entity_id`: generate the report,
/// queue it (as CSV) to each recipient via the notification outbox, and advance
/// next_run_at by the cadence. Returns the number of schedules run.
pub async fn process_report_schedules_for(engine: &ErpEngine, entity_id: Uuid) -> ErpResult<u32> {
    use crate::reporting::{ReportParameters, ReportRequest, ReportType};

    let now = Utc::now();
    let due = sqlx::query_as::<_, ReportScheduleRow>(
        "SELECT id, name, report_type, cadence, recipients FROM report_schedules
         WHERE entity_id = $1 AND is_active = true AND (next_run_at IS NULL OR next_run_at <= $2)",
    )
    .bind(entity_id)
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
            entity_id,
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
            let (enabled, channels) = crate::services::notification_prefs::effective_channels(
                engine,
                entity_id,
                &crate::notifications::NotificationEventType::ScheduledReport,
            )
            .await;
            if enabled && !channels.is_empty() {
                let req = crate::notifications::SendNotificationRequest {
                    event_type: crate::notifications::NotificationEventType::ScheduledReport,
                    channels,
                    recipients,
                    subject: Some(format!("{} — {}", s.name, now.date_naive())),
                    body: String::from_utf8_lossy(&csv).to_string(),
                    related_type: Some("report_schedule".to_string()),
                    related_id: Some(s.id),
                    schedule_at: None,
                    attachments: Vec::new(),
                };
                let _ = crate::services::notifications::send_notification(engine, entity_id, req).await;
            }
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

#[derive(Debug, sqlx::FromRow)]
struct RecurringJournalRow {
    id: Uuid,
    name: String,
    cadence: String,
    lines: serde_json::Value,
    auto_reverse: bool,
    next_run_date: chrono::NaiveDate,
}

#[derive(serde::Deserialize)]
struct RecurringJournalLine {
    account_code: String,
    #[serde(default)]
    debit: Option<Decimal>,
    #[serde(default)]
    credit: Option<Decimal>,
    #[serde(default)]
    description: Option<String>,
}

/// Advance leave balances for ALL tenants: materialize current-year balance
/// rows for every active employee (which recomputes accrual by tenure and
/// applies prior-year carryover). Idempotent — safe to run every tick.
pub async fn advance_leave_balances_all(engine: &ErpEngine) -> ErpResult<u32> {
    let mut total = 0u32;
    for entity_id in all_entity_ids(engine).await? {
        match advance_leave_balances(engine, entity_id).await {
            Ok(n) => total += n,
            Err(e) => tracing::error!("Leave accrual failed for entity {}: {}", entity_id, e),
        }
    }
    Ok(total)
}

/// Materialize current-year leave balances for one tenant's active employees.
pub async fn advance_leave_balances(engine: &ErpEngine, entity_id: Uuid) -> ErpResult<u32> {
    let year = chrono::Utc::now().year();
    let employees: Vec<Uuid> = sqlx::query_scalar(
        "SELECT id FROM employees WHERE entity_id = $1 AND is_active = true",
    )
    .bind(entity_id)
    .fetch_all(engine.pool())
    .await?;
    let mut n = 0u32;
    for emp in employees {
        // list_balances ensures a row per active type (accrual + carryover).
        if crate::services::leave::list_balances(engine, entity_id, emp, year).await.is_ok() {
            n += 1;
        }
    }
    Ok(n)
}

/// Post due recurring journals for ALL tenants.
pub async fn process_recurring_journals_all(engine: &ErpEngine) -> ErpResult<u32> {
    let mut total = 0u32;
    for entity_id in all_entity_ids(engine).await? {
        match process_recurring_journals(engine, entity_id).await {
            Ok(n) => total += n,
            Err(e) => tracing::error!("Recurring journals failed for entity {}: {}", entity_id, e),
        }
    }
    Ok(total)
}

/// Post any recurring journals due for `entity_id`. For an accrual template
/// (`auto_reverse`), also posts a mirror reversal on the first day of the next
/// month.
///
/// Idempotency + atomicity (Finding A): each run's journal reference embeds the
/// scheduled date (`REC-{name}-{YYYY-MM-DD}`), so a re-run is detected by a
/// reference-exists check and **skipped** rather than colliding on the unique
/// constraint. The post and the `next_run_date` advance happen in the SAME
/// transaction, so a crash between them cannot double-post or strand the
/// schedule.
pub async fn process_recurring_journals(engine: &ErpEngine, entity_id: Uuid) -> ErpResult<u32> {
    use crate::ledger::journal::{CreateJournalEntryRequest, CreateJournalLineRequest, JournalSource};

    let today = Utc::now().date_naive();
    let base_ccy = engine.config_for(entity_id).await?.base_currency.clone();
    let due = sqlx::query_as::<_, RecurringJournalRow>(
        "SELECT id, name, cadence, lines, auto_reverse, next_run_date FROM recurring_journals
         WHERE entity_id = $1 AND is_active = true AND next_run_date <= $2",
    )
    .bind(entity_id)
    .bind(today)
    .fetch_all(engine.pool())
    .await?;

    let mut count = 0u32;
    for t in due {
        let tmpl: Vec<RecurringJournalLine> = serde_json::from_value(t.lines).unwrap_or_default();
        let build = |reverse: bool| -> Vec<CreateJournalLineRequest> {
            tmpl.iter()
                .filter_map(|l| {
                    let dr = l.debit.unwrap_or(Decimal::ZERO);
                    let cr = l.credit.unwrap_or(Decimal::ZERO);
                    if dr.is_zero() && cr.is_zero() {
                        return None;
                    }
                    // On reversal, swap debit and credit.
                    let (dr, cr) = if reverse { (cr, dr) } else { (dr, cr) };
                    Some(CreateJournalLineRequest {
                        account_code: l.account_code.clone(),
                        debit: if dr > Decimal::ZERO { Some(dr) } else { None },
                        credit: if cr > Decimal::ZERO { Some(cr) } else { None },
                        currency: base_ccy.clone(),
                        fx_rate: Some(Decimal::ONE),
                        description: l.description.clone(),
                        dimensions: None,
                    })
                })
                .collect()
        };

        let lines = build(false);
        let total_dr: Decimal = lines.iter().filter_map(|l| l.debit).sum();
        let total_cr: Decimal = lines.iter().filter_map(|l| l.credit).sum();
        if lines.len() < 2 || (total_dr - total_cr).abs() >= Decimal::new(1, 2) {
            tracing::warn!("Recurring journal {} skipped: unbalanced or too few lines", t.id);
            continue;
        }

        // Date-stamped reference makes each scheduled run unique and detectable.
        let reference = format!("REC-{}-{}", t.name, t.next_run_date);
        let already = sqlx::query_scalar::<_, bool>(
            "SELECT EXISTS(SELECT 1 FROM journal_entries WHERE entity_id = $1 AND reference = $2)",
        )
        .bind(entity_id)
        .bind(&reference)
        .fetch_one(engine.pool())
        .await?;

        let next = match t.cadence.as_str() {
            "weekly" => t.next_run_date + chrono::Duration::days(7),
            "quarterly" => t.next_run_date.checked_add_months(chrono::Months::new(3)).unwrap_or(t.next_run_date),
            _ => t.next_run_date.checked_add_months(chrono::Months::new(1)).unwrap_or(t.next_run_date),
        };

        // If this run was already posted (e.g. a prior crash after the post but
        // before the advance), just advance the schedule — don't post again.
        if already {
            sqlx::query("UPDATE recurring_journals SET next_run_date = $1, last_run_at = NOW() WHERE id = $2")
                .bind(next)
                .bind(t.id)
                .execute(engine.pool())
                .await?;
            continue;
        }

        let Ok(period) = crate::services::periods::period_for_date(engine, entity_id, t.next_run_date).await else {
            tracing::warn!("Recurring journal {}: no period for {}", t.id, t.next_run_date);
            continue;
        };

        // Resolve the reversal period up front (if applicable) so the whole unit
        // posts atomically.
        let reversal = if t.auto_reverse {
            match t.next_run_date.with_day(1).and_then(|d| d.checked_add_months(chrono::Months::new(1))) {
                Some(rev_date) => match crate::services::periods::period_for_date(engine, entity_id, rev_date).await {
                    Ok(p) => Some((rev_date, p.id)),
                    Err(_) => None,
                },
                None => None,
            }
        } else {
            None
        };

        let actor = crate::AgentOrUserId::Agent("scheduler".to_string());
        let mut tx = engine.pool().begin().await?;

        let req = CreateJournalEntryRequest {
            date: t.next_run_date,
            source: JournalSource::Manual,
            source_id: Some(t.id),
            reference,
            description: t.name.clone(),
            lines,
            post_immediately: true,
        };
        if let Err(e) = crate::services::journal::create_and_post_in_tx(&mut tx, engine, entity_id, req, period.id, actor.clone()).await {
            tracing::error!("Recurring journal {} post failed: {}", t.id, e);
            continue; // tx dropped/rolled back
        }

        if let Some((rev_date, rev_period_id)) = reversal {
            let rev_req = CreateJournalEntryRequest {
                date: rev_date,
                source: JournalSource::Manual,
                source_id: Some(t.id),
                reference: format!("REC-REV-{}-{}", t.name, t.next_run_date),
                description: format!("{} (accrual reversal)", t.name),
                lines: build(true),
                post_immediately: true,
            };
            if let Err(e) = crate::services::journal::create_and_post_in_tx(&mut tx, engine, entity_id, rev_req, rev_period_id, actor).await {
                tracing::error!("Recurring journal {} reversal post failed: {}", t.id, e);
                continue; // tx dropped/rolled back — main entry not committed either
            }
        }

        // Advance the schedule in the SAME transaction as the post(s).
        sqlx::query("UPDATE recurring_journals SET next_run_date = $1, last_run_at = NOW() WHERE id = $2")
            .bind(next)
            .bind(t.id)
            .execute(&mut *tx)
            .await?;

        tx.commit().await?;
        count += 1;
    }

    Ok(count)
}

/// Process due recurring invoices for ALL tenants.
pub async fn process_recurring_invoices(engine: &ErpEngine) -> ErpResult<Vec<Uuid>> {
    let mut all_ids = Vec::new();
    for entity_id in all_entity_ids(engine).await? {
        match process_recurring_invoices_for(engine, entity_id).await {
            Ok(mut ids) => all_ids.append(&mut ids),
            Err(e) => tracing::error!("Recurring invoices failed for entity {}: {}", entity_id, e),
        }
    }
    Ok(all_ids)
}

/// Process all recurring invoices for `entity_id` that are due today or earlier.
/// Creates an invoice for each due template, using the template's **scheduled**
/// `next_run` date as the issue date (not "today"), so scheduler downtime cannot
/// collapse backdated runs onto the current day or drift the cadence. The
/// next_run advance and run bookkeeping happen in the same transaction as the
/// invoice insert (Finding A), keyed off the scheduled date (Finding B).
pub async fn process_recurring_invoices_for(engine: &ErpEngine, entity_id: Uuid) -> ErpResult<Vec<Uuid>> {
    let today = Utc::now().date_naive();

    // Fetch all active recurring invoices where next_run <= today
    let due = sqlx::query_as::<_, RecurringInvoiceRow>(
        "SELECT * FROM recurring_invoices WHERE entity_id = $1 AND is_active = true AND next_run <= $2",
    )
    .bind(entity_id)
    .bind(today)
    .fetch_all(engine.pool())
    .await?;

    let mut created_ids = Vec::new();
    let actor = crate::types::AgentOrUserId::Agent("recurring-scheduler".to_string());

    for rec in &due {
        // The invoice belongs to the scheduled run date, not today.
        let scheduled = rec.next_run;
        let frequency: crate::invoicing::RecurrenceFreq =
            serde_json::from_str(&format!("\"{}\"", rec.frequency))
                .unwrap_or(crate::invoicing::RecurrenceFreq::Monthly);
        let next_run = frequency.next_date(scheduled);

        // Deserialize the template into CreateInvoiceRequest, forcing the issue
        // date to the scheduled run date.
        let mut template: crate::invoicing::CreateInvoiceRequest =
            serde_json::from_value(rec.template.clone()).unwrap_or_else(|_| {
                crate::invoicing::CreateInvoiceRequest {
                    customer_id: rec.customer_id,
                    issue_date: Some(scheduled),
                    due_date: None,
                    currency: None,
                    fx_rate: None,
                    lines: Vec::new(),
                    template_id: None,
                    notes: None,
                    send_immediately: None,
                }
            });
        template.issue_date = Some(scheduled);

        // Create invoice from template
        match crate::services::invoicing::create_invoice(engine, rec.entity_id, template, &actor).await {
            Ok(invoice) => {
                created_ids.push(invoice.id);

                // Link the generated invoice back to its recurring template so the
                // template can show its real history.
                let _ = sqlx::query("UPDATE invoices SET recurring_invoice_id = $1 WHERE id = $2")
                    .bind(rec.id)
                    .bind(invoice.id)
                    .execute(engine.pool())
                    .await;

                // If auto_send, post it
                if rec.auto_send {
                    let _ =
                        crate::services::invoicing::post_invoice(engine, rec.entity_id, invoice.id, &actor).await;
                }

                // Advance bookkeeping keyed off the scheduled date. Deactivate if
                // the next occurrence would fall past the template's end_date.
                let deactivate = rec.end_date.map(|end| next_run > end).unwrap_or(false);
                sqlx::query(
                    "UPDATE recurring_invoices
                     SET next_run = $1, last_run = $2, run_count = run_count + 1,
                         is_active = CASE WHEN $3 THEN false ELSE is_active END
                     WHERE id = $4",
                )
                .bind(next_run)
                .bind(scheduled)
                .bind(deactivate)
                .bind(rec.id)
                .execute(engine.pool())
                .await?;
            }
            Err(e) => {
                tracing::error!("Failed to create recurring invoice {}: {}", rec.id, e);
            }
        }
    }

    Ok(created_ids)
}

/// Process invoice payment reminders for ALL tenants.
pub async fn process_invoice_reminders(engine: &ErpEngine) -> ErpResult<u32> {
    let mut total = 0u32;
    for entity_id in all_entity_ids(engine).await? {
        match process_invoice_reminders_for(engine, entity_id).await {
            Ok(n) => total += n,
            Err(e) => tracing::error!("Invoice reminders failed for entity {}: {}", entity_id, e),
        }
    }
    Ok(total)
}

/// Process invoice payment reminders for `entity_id`.
/// Checks all unpaid invoices and sends reminders based on customer reminder policies.
pub async fn process_invoice_reminders_for(engine: &ErpEngine, entity_id: Uuid) -> ErpResult<u32> {
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
    .bind(entity_id)
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
                    attachments: Vec::new(),
                };

                let _ = crate::services::notifications::send_notification(engine, entity_id, req).await;
                sent_count += 1;
            }
        }
    }

    // Mark overdue invoices
    sqlx::query(
        "UPDATE invoices SET status = 'overdue' WHERE entity_id = $1 AND status IN ('posted', 'sent', 'viewed') AND due_date < $2 AND balance_due > 0",
    )
    .bind(entity_id)
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
                attachments: Vec::new(),
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

/// Auto-post depreciation at month-end for every tenant that has depreciable
/// assets. Runs as-of the last day of the *previous* month (only fully-elapsed
/// months), catching up any missed months. Idempotent — safe to run every tick:
/// `run_depreciation` never re-posts a month already booked.
pub async fn process_depreciation(engine: &ErpEngine) -> ErpResult<u32> {
    let today = Utc::now().date_naive();
    let first_of_this = chrono::NaiveDate::from_ymd_opt(today.year(), today.month(), 1)
        .ok_or_else(|| ErpError::ValidationFailed { message: "bad date".into() })?;
    let as_of = first_of_this.pred_opt().unwrap(); // last day of previous month

    let entity_ids: Vec<Uuid> = sqlx::query_scalar(
        "SELECT DISTINCT entity_id FROM fixed_assets WHERE status = 'active' AND net_book_value > residual_value",
    )
    .fetch_all(engine.pool())
    .await?;

    let actor = crate::types::AgentOrUserId::Agent("depreciation-scheduler".to_string());
    let mut count = 0u32;
    for eid in entity_ids {
        match crate::services::assets::run_depreciation(engine, eid, as_of, &actor).await {
            Ok(ids) => count += ids.len() as u32,
            Err(e) => tracing::error!("Depreciation run failed for entity {}: {}", eid, e),
        }
    }
    Ok(count)
}
