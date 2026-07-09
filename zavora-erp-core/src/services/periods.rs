use chrono::{NaiveDate, Utc};
use uuid::Uuid;

use crate::engine::ErpEngine;
use crate::error::{ErpError, ErpResult};
use crate::notifications::{NotificationEventType, SendNotificationRequest};
use crate::period::*;

/// Generate fiscal periods for a year.
pub async fn generate_periods(
    engine: &ErpEngine,
    entity_id: Uuid,
    req: GeneratePeriodsRequest,
) -> ErpResult<Vec<FiscalPeriod>> {
    let mut periods = Vec::new();

    for month_offset in 0..12u32 {
        let month = ((req.year_start_month - 1 + month_offset) % 12) + 1;
        let year = if month < req.year_start_month {
            req.fiscal_year + 1
        } else {
            req.fiscal_year
        };

        let start_date = NaiveDate::from_ymd_opt(year as i32, month, 1).ok_or_else(|| {
            ErpError::ValidationFailed {
                message: format!("Invalid date: {}-{}-01", year, month),
            }
        })?;

        let end_date = if month == 12 {
            NaiveDate::from_ymd_opt(year as i32, 12, 31).unwrap()
        } else {
            NaiveDate::from_ymd_opt(year as i32, month + 1, 1).unwrap()
                - chrono::Duration::days(1)
        };

        let period_name = start_date.format("%B %Y").to_string();
        let id = Uuid::new_v4();
        let now = Utc::now();

        // Determine initial status
        let today = Utc::now().date_naive();
        let status = if start_date > today {
            "future"
        } else {
            "open"
        };

        sqlx::query(
            r#"INSERT INTO fiscal_periods 
               (id, entity_id, name, start_date, end_date, status, fiscal_year, period_number, created_at)
               VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
               ON CONFLICT (entity_id, start_date) DO NOTHING"#,
        )
        .bind(id)
        .bind(entity_id)
        .bind(&period_name)
        .bind(start_date)
        .bind(end_date)
        .bind(status)
        .bind(req.fiscal_year)
        .bind((month_offset + 1) as i32)
        .bind(now)
        .execute(engine.pool())
        .await?;

        periods.push(FiscalPeriod {
            id,
            entity_id,
            name: period_name,
            start_date,
            end_date,
            status: status.to_string(),
            fiscal_year: req.fiscal_year,
            period_number: (month_offset + 1) as i32,
            closed_by: None,
            closed_at: None,
            created_at: now,
        });
    }

    Ok(periods)
}

/// Close a fiscal period (soft or hard).
///
/// Soft close: Open → SoftClosed. While SoftClosed, only manual journal entries allowed.
/// Hard close: SoftClosed → HardClosed. If period is still Open, reject (must soft-close first).
pub async fn close_period(engine: &ErpEngine, entity_id: Uuid, req: ClosePeriodRequest) -> ErpResult<FiscalPeriod> {
    let period = get_period(engine, entity_id, req.period_id).await?;

    // Validate current state allows the requested close type
    match (&period.parsed_status(), &req.close_type) {
        // Soft close: from Open or Future. Future periods are postable, so they
        // must be lockable too (e.g. locking a not-yet-started year while
        // back-booking a prior one).
        (PeriodStatus::Open, PeriodCloseType::Soft) => {}
        (PeriodStatus::Future, PeriodCloseType::Soft) => {}
        // Hard close: only from SoftClosed
        (PeriodStatus::SoftClosed, PeriodCloseType::Hard) => {}
        // Hard close from Open/Future is rejected — must soft-close first
        (PeriodStatus::Open, PeriodCloseType::Hard) | (PeriodStatus::Future, PeriodCloseType::Hard) => {
            return Err(ErpError::ValidationFailed {
                message: format!(
                    "Period '{}' is not soft-closed; you must soft-close it before hard-closing",
                    period.name
                ),
            });
        }
        // Already soft-closed
        (PeriodStatus::SoftClosed, PeriodCloseType::Soft) => {
            return Err(ErpError::ValidationFailed {
                message: "Period is already soft-closed".to_string(),
            });
        }
        // Already hard-closed
        (PeriodStatus::HardClosed, _) => {
            return Err(ErpError::ValidationFailed {
                message: "Period is already hard-closed and cannot be modified".to_string(),
            });
        }
    }

    // Hard close is (near) irreversible — run the pre-close checklist and
    // refuse while known work is unfinished, unless the caller forces it.
    if req.close_type == PeriodCloseType::Hard && !req.force {
        let blockers = pre_close_checklist(engine, entity_id, &period).await?;
        if !blockers.is_empty() {
            return Err(ErpError::ValidationFailed {
                message: format!(
                    "Pre-close checklist failed for '{}': {}. Resolve these or pass force=true to close anyway.",
                    period.name,
                    blockers.join("; ")
                ),
            });
        }
    }

    let new_status = match req.close_type {
        PeriodCloseType::Soft => "soft_closed",
        PeriodCloseType::Hard => "hard_closed",
    };
    let now = Utc::now();

    sqlx::query(
        "UPDATE fiscal_periods SET status = $1, closed_by = $2, closed_at = $3 WHERE id = $4",
    )
    .bind(new_status)
    .bind(serde_json::to_value(&req.closed_by).unwrap_or_default())
    .bind(now)
    .bind(req.period_id)
    .execute(engine.pool())
    .await?;

    let mut updated = period.clone();
    updated.status = new_status.to_string();
    updated.closed_at = Some(now);
    updated.closed_by = Some(serde_json::to_value(&req.closed_by).unwrap_or_default());

    // On soft close, warn Accountant and Admin users — channels per tenant prefs.
    if req.close_type == PeriodCloseType::Soft {
        let (enabled, channels) = crate::services::notification_prefs::effective_channels(
            engine,
            entity_id,
            &NotificationEventType::PeriodCloseWarning,
        )
        .await;
        if enabled && !channels.is_empty() {
            let notification = SendNotificationRequest {
                event_type: NotificationEventType::PeriodCloseWarning,
                channels,
                recipients: vec!["role:Accountant".to_string(), "role:Admin".to_string()],
                subject: Some(format!("Period '{}' has been soft-closed", updated.name)),
                body: format!(
                    "Fiscal period '{}' has been soft-closed. Only manual journal entries (prior-period adjustments) are allowed until the period is hard-closed or reopened.",
                    updated.name
                ),
                related_type: Some("fiscal_period".to_string()),
                related_id: Some(updated.id),
                schedule_at: None,
                attachments: Vec::new(),
            };
            // Best-effort — don't fail the close operation on notification errors.
            let _ = super::notifications::send_notification(engine, entity_id, notification).await;
        }
    }

    // Record PeriodClosed audit event
    let audit_event = serde_json::json!({
        "event_type": "PeriodClosed",
        "object_type": "fiscal_period",
        "object_id": updated.id,
        "actor": req.closed_by,
        "close_type": new_status,
        "period_name": updated.name,
        "before_status": period.status,
        "after_status": new_status,
        "checklist_forced": req.force,
        "timestamp": now,
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

    Ok(updated)
}

/// Pre-close checklist for a hard close: names the unfinished work that would
/// be locked wrong forever. Each blocker is a human sentence; an empty list
/// means the period is clean to close.
pub async fn pre_close_checklist(
    engine: &ErpEngine,
    entity_id: Uuid,
    period: &FiscalPeriod,
) -> ErpResult<Vec<String>> {
    let mut blockers = Vec::new();

    let draft_invoices: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM invoices WHERE entity_id = $1 AND issue_date BETWEEN $2 AND $3 AND status = 'draft'",
    )
    .bind(entity_id)
    .bind(period.start_date)
    .bind(period.end_date)
    .fetch_one(engine.pool())
    .await?;
    if draft_invoices > 0 {
        blockers.push(format!("{draft_invoices} draft invoice(s) dated in the period (post or re-date them)"));
    }

    let unposted_bills: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM bills WHERE entity_id = $1 AND issue_date BETWEEN $2 AND $3 AND status IN ('draft', 'pending_approval', 'approved')",
    )
    .bind(entity_id)
    .bind(period.start_date)
    .bind(period.end_date)
    .fetch_one(engine.pool())
    .await?;
    if unposted_bills > 0 {
        blockers.push(format!("{unposted_bills} unposted bill(s) dated in the period (post, cancel or re-date them)"));
    }

    let draft_journals: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM journal_entries WHERE entity_id = $1 AND date BETWEEN $2 AND $3 AND status = 'draft'",
    )
    .bind(entity_id)
    .bind(period.start_date)
    .bind(period.end_date)
    .fetch_one(engine.pool())
    .await?;
    if draft_journals > 0 {
        blockers.push(format!("{draft_journals} draft journal entry(ies) dated in the period"));
    }

    // Depreciation not caught up through the period end for active assets that
    // existed during the period.
    let behind_assets: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM fixed_assets
         WHERE entity_id = $1 AND status = 'active' AND acquisition_date <= $2
           AND (depreciated_through IS NULL OR depreciated_through < $2)",
    )
    .bind(entity_id)
    .bind(period.end_date)
    .fetch_one(engine.pool())
    .await?;
    if behind_assets > 0 {
        blockers.push(format!("depreciation not run through {} for {behind_assets} active asset(s)", period.end_date));
    }

    Ok(blockers)
}

/// Reopen a soft-closed period.
///
/// Only SoftClosed periods can be reopened (HardClosed periods are immutable).
/// A reason must be provided for audit trail purposes.
pub async fn reopen_period(engine: &ErpEngine, entity_id: Uuid, req: ReopenPeriodRequest) -> ErpResult<FiscalPeriod> {
    let period = get_period(engine, entity_id, req.period_id).await?;

    if period.parsed_status() != PeriodStatus::SoftClosed {
        return Err(ErpError::ValidationFailed {
            message: "Only soft-closed periods can be reopened".to_string(),
        });
    }

    if req.reason.trim().is_empty() {
        return Err(ErpError::ValidationFailed {
            message: "A reason is required to reopen a period".to_string(),
        });
    }

    let now = Utc::now();

    sqlx::query(
        "UPDATE fiscal_periods SET status = 'open', closed_by = NULL, closed_at = NULL WHERE id = $1",
    )
    .bind(req.period_id)
    .execute(engine.pool())
    .await?;

    let mut updated = period.clone();
    updated.status = "open".to_string();
    updated.closed_by = None;
    updated.closed_at = None;

    // Record PeriodReopened audit event
    let audit_event = serde_json::json!({
        "event_type": "PeriodReopened",
        "object_type": "fiscal_period",
        "object_id": updated.id,
        "actor": req.reopened_by,
        "reason": req.reason,
        "period_name": updated.name,
        "before_status": "soft_closed",
        "after_status": "open",
        "timestamp": now,
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

    Ok(updated)
}

/// Get a fiscal period by ID, scoped to the tenant.
pub async fn get_period(engine: &ErpEngine, entity_id: Uuid, period_id: Uuid) -> ErpResult<FiscalPeriod> {
    sqlx::query_as::<_, FiscalPeriod>("SELECT * FROM fiscal_periods WHERE id = $1 AND entity_id = $2")
        .bind(period_id)
        .bind(entity_id)
        .fetch_optional(engine.pool())
        .await?
        .ok_or_else(|| ErpError::NotFound {
            entity_type: "FiscalPeriod".to_string(),
            id: period_id,
        })
}

/// Get the period for a specific date.
pub async fn period_for_date(engine: &ErpEngine, entity_id: Uuid, date: NaiveDate) -> ErpResult<FiscalPeriod> {
    sqlx::query_as::<_, FiscalPeriod>(
        "SELECT * FROM fiscal_periods WHERE entity_id = $1 AND start_date <= $2 AND end_date >= $2",
    )
    .bind(entity_id)
    .bind(date)
    .fetch_optional(engine.pool())
    .await?
    .ok_or_else(|| ErpError::ValidationFailed {
        message: format!("No fiscal period found for date {}", date),
    })
}

/// The fiscal year a document `date` belongs to — i.e. the fiscal year of the
/// period it posts into. Document numbers use THIS (not the calendar year the
/// document is keyed in), so a 2025-dated invoice entered in 2026 is still
/// numbered for 2025. Falls back to the calendar year when no period exists yet.
pub async fn fiscal_year_for_date(engine: &ErpEngine, entity_id: Uuid, date: NaiveDate) -> i32 {
    use chrono::Datelike;
    period_for_date(engine, entity_id, date)
        .await
        .map(|p| p.fiscal_year)
        .unwrap_or_else(|_| date.year())
}

/// List all periods for the entity.
pub async fn list_periods(engine: &ErpEngine, entity_id: Uuid) -> ErpResult<Vec<FiscalPeriod>> {
    let periods = sqlx::query_as::<_, FiscalPeriod>(
        "SELECT * FROM fiscal_periods WHERE entity_id = $1 ORDER BY start_date",
    )
    .bind(entity_id)
    .fetch_all(engine.pool())
    .await?;

    Ok(periods)
}
