//! Leave management service: leave types, holidays, balances, and the request
//! lifecycle (create → approve/decline/cancel) with transactional balance
//! updates. Entitlement is derived from the leave type's accrual method; paid
//! requests are validated against the available balance.

use chrono::{Datelike, NaiveDate, Utc};
use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use uuid::Uuid;

use crate::engine::ErpEngine;
use crate::error::{ErpError, ErpResult};
use crate::hr::*;

// ─── Leave types ─────────────────────────────────────────────────────────────

pub async fn list_leave_types(engine: &ErpEngine, entity_id: Uuid) -> ErpResult<Vec<LeaveTypeRow>> {
    let rows = sqlx::query_as::<_, LeaveTypeRow>(
        "SELECT * FROM leave_types WHERE entity_id = $1 ORDER BY is_statutory DESC, name",
    )
    .bind(entity_id)
    .fetch_all(engine.pool())
    .await?;
    Ok(rows)
}

pub async fn create_leave_type(
    engine: &ErpEngine,
    entity_id: Uuid,
    req: CreateLeaveTypeRequest,
) -> ErpResult<Uuid> {
    let id = Uuid::new_v4();
    sqlx::query(
        r#"INSERT INTO leave_types
           (id, entity_id, name, code, paid, accrual_method, days_per_year, carryover_max,
            requires_attachment, is_statutory, active, created_at)
           VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,true,NOW())
           ON CONFLICT (entity_id, code) DO NOTHING"#,
    )
    .bind(id)
    .bind(entity_id)
    .bind(&req.name)
    .bind(&req.code)
    .bind(req.paid)
    .bind(req.accrual_method.as_str())
    .bind(req.days_per_year)
    .bind(req.carryover_max)
    .bind(req.requires_attachment)
    .bind(req.is_statutory)
    .execute(engine.pool())
    .await?;
    Ok(id)
}

pub async fn set_leave_type_active(
    engine: &ErpEngine,
    entity_id: Uuid,
    id: Uuid,
    active: bool,
) -> ErpResult<()> {
    sqlx::query("UPDATE leave_types SET active = $3 WHERE id = $1 AND entity_id = $2")
        .bind(id)
        .bind(entity_id)
        .bind(active)
        .execute(engine.pool())
        .await?;
    Ok(())
}

/// Seed the Kenyan default leave types for a tenant if it has none yet.
/// Idempotent — safe to call on every leave-page load.
pub async fn seed_default_leave_types(engine: &ErpEngine, entity_id: Uuid) -> ErpResult<()> {
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM leave_types WHERE entity_id = $1")
        .bind(entity_id)
        .fetch_one(engine.pool())
        .await?;
    if count == 0 {
        for t in kenyan_default_leave_types() {
            create_leave_type(engine, entity_id, t).await?;
        }
    }
    Ok(())
}

// ─── Holidays ────────────────────────────────────────────────────────────────

pub async fn list_holidays(engine: &ErpEngine, entity_id: Uuid) -> ErpResult<Vec<HolidayRow>> {
    let rows = sqlx::query_as::<_, HolidayRow>(
        "SELECT * FROM holidays WHERE entity_id = $1 ORDER BY date",
    )
    .bind(entity_id)
    .fetch_all(engine.pool())
    .await?;
    Ok(rows)
}

pub async fn create_holiday(
    engine: &ErpEngine,
    entity_id: Uuid,
    req: CreateHolidayRequest,
) -> ErpResult<Uuid> {
    let id = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO holidays (id, entity_id, date, name, recurring, created_at) \
         VALUES ($1,$2,$3,$4,$5,NOW()) ON CONFLICT (entity_id, date) DO UPDATE SET name = EXCLUDED.name",
    )
    .bind(id)
    .bind(entity_id)
    .bind(req.date)
    .bind(&req.name)
    .bind(req.recurring)
    .execute(engine.pool())
    .await?;
    Ok(id)
}

pub async fn delete_holiday(engine: &ErpEngine, entity_id: Uuid, id: Uuid) -> ErpResult<()> {
    sqlx::query("DELETE FROM holidays WHERE id = $1 AND entity_id = $2")
        .bind(id)
        .bind(entity_id)
        .execute(engine.pool())
        .await?;
    Ok(())
}

/// Holiday dates in a range (for the working-days calculation). Public so
/// payroll can prorate unpaid leave on the period's working days.
pub async fn holiday_dates_pub(
    engine: &ErpEngine,
    entity_id: Uuid,
    from: NaiveDate,
    to: NaiveDate,
) -> ErpResult<Vec<NaiveDate>> {
    holiday_dates(engine, entity_id, from, to).await
}

/// Holiday dates in a range (for the working-days calculation).
async fn holiday_dates(
    engine: &ErpEngine,
    entity_id: Uuid,
    from: NaiveDate,
    to: NaiveDate,
) -> ErpResult<Vec<NaiveDate>> {
    // Include both fixed-date rows in range and recurring rows matching month/day.
    let rows = sqlx::query_as::<_, HolidayRow>("SELECT * FROM holidays WHERE entity_id = $1")
        .bind(entity_id)
        .fetch_all(engine.pool())
        .await?;
    let mut out = Vec::new();
    for h in rows {
        if h.recurring {
            // Match by month/day within [from, to].
            let mut y = from.year();
            while y <= to.year() {
                if let Some(dte) = NaiveDate::from_ymd_opt(y, h.date.month(), h.date.day()) {
                    if dte >= from && dte <= to {
                        out.push(dte);
                    }
                }
                y += 1;
            }
        } else if h.date >= from && h.date <= to {
            out.push(h.date);
        }
    }
    Ok(out)
}

// ─── Balances ────────────────────────────────────────────────────────────────

/// Entitlement for a leave type in a given year, honouring the accrual method.
/// MonthlyAccrual grants pro-rata by elapsed months of the current year.
fn entitlement_for(days_per_year: Decimal, method: AccrualMethod, year: i32) -> Decimal {
    match method {
        AccrualMethod::FixedAnnual | AccrualMethod::Unlimited => days_per_year,
        AccrualMethod::MonthlyAccrual => {
            let now = Utc::now().date_naive();
            let months = if now.year() > year {
                12
            } else if now.year() < year {
                0
            } else {
                now.month() as i64 // Jan..current month elapsed
            };
            (days_per_year * Decimal::from(months) / dec!(12)).round_dp(2)
        }
    }
}

/// Ensure a balance row exists for (employee, type, year) and return it, with
/// `entitled`/`accrued` refreshed from the type's current accrual.
pub async fn ensure_balance(
    engine: &ErpEngine,
    entity_id: Uuid,
    employee_id: Uuid,
    leave_type_id: Uuid,
    year: i32,
) -> ErpResult<LeaveBalanceRow> {
    let lt = sqlx::query_as::<_, LeaveTypeRow>(
        "SELECT * FROM leave_types WHERE id = $1 AND entity_id = $2",
    )
    .bind(leave_type_id)
    .bind(entity_id)
    .fetch_optional(engine.pool())
    .await?
    .ok_or_else(|| ErpError::NotFound { entity_type: "LeaveType".into(), id: leave_type_id })?;

    let accrued = entitlement_for(lt.days_per_year, AccrualMethod::parse(&lt.accrual_method), year);

    sqlx::query(
        r#"INSERT INTO leave_balances
             (id, entity_id, employee_id, leave_type_id, year, entitled_days, accrued_days, updated_at)
           VALUES ($1,$2,$3,$4,$5,$6,$7,NOW())
           ON CONFLICT (employee_id, leave_type_id, year)
           DO UPDATE SET entitled_days = EXCLUDED.entitled_days,
                         accrued_days  = EXCLUDED.accrued_days,
                         updated_at    = NOW()"#,
    )
    .bind(Uuid::new_v4())
    .bind(entity_id)
    .bind(employee_id)
    .bind(leave_type_id)
    .bind(year)
    .bind(lt.days_per_year)
    .bind(accrued)
    .execute(engine.pool())
    .await?;

    let row = sqlx::query_as::<_, LeaveBalanceRow>(
        "SELECT * FROM leave_balances WHERE employee_id = $1 AND leave_type_id = $2 AND year = $3",
    )
    .bind(employee_id)
    .bind(leave_type_id)
    .bind(year)
    .fetch_one(engine.pool())
    .await?;
    Ok(row)
}

/// All balances for an employee in a year (ensures a row per active type).
pub async fn list_balances(
    engine: &ErpEngine,
    entity_id: Uuid,
    employee_id: Uuid,
    year: i32,
) -> ErpResult<Vec<LeaveBalanceRow>> {
    let types = list_leave_types(engine, entity_id).await?;
    for t in types.iter().filter(|t| t.active) {
        ensure_balance(engine, entity_id, employee_id, t.id, year).await?;
    }
    let rows = sqlx::query_as::<_, LeaveBalanceRow>(
        "SELECT * FROM leave_balances WHERE entity_id = $1 AND employee_id = $2 AND year = $3",
    )
    .bind(entity_id)
    .bind(employee_id)
    .bind(year)
    .fetch_all(engine.pool())
    .await?;
    Ok(rows)
}

// ─── Requests ────────────────────────────────────────────────────────────────

pub async fn create_leave_request(
    engine: &ErpEngine,
    entity_id: Uuid,
    employee_id: Uuid,
    req: CreateLeaveRequest,
) -> ErpResult<Uuid> {
    if req.end_date < req.start_date {
        return Err(ErpError::ValidationFailed { message: "End date is before start date".into() });
    }
    let lt = sqlx::query_as::<_, LeaveTypeRow>(
        "SELECT * FROM leave_types WHERE id = $1 AND entity_id = $2",
    )
    .bind(req.leave_type_id)
    .bind(entity_id)
    .fetch_optional(engine.pool())
    .await?
    .ok_or_else(|| ErpError::NotFound { entity_type: "LeaveType".into(), id: req.leave_type_id })?;

    if lt.requires_attachment && req.attachment_url.as_deref().unwrap_or("").is_empty() {
        return Err(ErpError::ValidationFailed {
            message: format!("{} requires a supporting document", lt.name),
        });
    }

    // Overlap guard: reject a new request colliding with a pending/approved one.
    let overlap: i64 = sqlx::query_scalar(
        r#"SELECT COUNT(*) FROM leave_requests
           WHERE entity_id = $1 AND employee_id = $2 AND status IN ('Pending','Approved')
             AND NOT (end_date < $3 OR start_date > $4)"#,
    )
    .bind(entity_id)
    .bind(employee_id)
    .bind(req.start_date)
    .bind(req.end_date)
    .fetch_one(engine.pool())
    .await?;
    if overlap > 0 {
        return Err(ErpError::ValidationFailed {
            message: "This overlaps an existing leave request".into(),
        });
    }

    let holidays = holiday_dates(engine, entity_id, req.start_date, req.end_date).await?;
    let days = working_days(req.start_date, req.end_date, req.half_day_start, req.half_day_end, &holidays);
    if days <= Decimal::ZERO {
        return Err(ErpError::ValidationFailed {
            message: "The selected dates contain no working days".into(),
        });
    }

    let year = req.start_date.year();

    // Balance check for paid, capped types (Unlimited/unpaid skip the cap).
    let method = AccrualMethod::parse(&lt.accrual_method);
    if lt.paid && method != AccrualMethod::Unlimited {
        let bal = ensure_balance(engine, entity_id, employee_id, lt.id, year).await?;
        let available = bal.accrued_days + bal.carried_over - bal.taken_days - bal.pending_days;
        if days > available {
            return Err(ErpError::ValidationFailed {
                message: format!(
                    "Insufficient {} balance: {} day(s) available, {} requested",
                    lt.name, available, days
                ),
            });
        }
    }

    let mut tx = engine.pool().begin().await?;
    let id = Uuid::new_v4();
    sqlx::query(
        r#"INSERT INTO leave_requests
           (id, entity_id, employee_id, leave_type_id, start_date, end_date, half_day_start,
            half_day_end, working_days, reason, attachment_url, status, created_at)
           VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,'Pending',NOW())"#,
    )
    .bind(id)
    .bind(entity_id)
    .bind(employee_id)
    .bind(req.leave_type_id)
    .bind(req.start_date)
    .bind(req.end_date)
    .bind(req.half_day_start)
    .bind(req.half_day_end)
    .bind(days)
    .bind(&req.reason)
    .bind(&req.attachment_url)
    .execute(&mut *tx)
    .await?;

    // Reserve the days as pending against the balance.
    sqlx::query(
        "UPDATE leave_balances SET pending_days = pending_days + $4, updated_at = NOW() \
         WHERE employee_id = $1 AND leave_type_id = $2 AND year = $3",
    )
    .bind(employee_id)
    .bind(req.leave_type_id)
    .bind(year)
    .bind(days)
    .execute(&mut *tx)
    .await?;

    tx.commit().await?;
    Ok(id)
}

fn fetch_request<'a>(
    engine: &'a ErpEngine,
    entity_id: Uuid,
    id: Uuid,
) -> impl std::future::Future<Output = ErpResult<LeaveRequestRow>> + 'a {
    async move {
        sqlx::query_as::<_, LeaveRequestRow>(
            "SELECT * FROM leave_requests WHERE id = $1 AND entity_id = $2",
        )
        .bind(id)
        .bind(entity_id)
        .fetch_optional(engine.pool())
        .await?
        .ok_or_else(|| ErpError::NotFound { entity_type: "LeaveRequest".into(), id })
    }
}

pub async fn approve_leave(
    engine: &ErpEngine,
    entity_id: Uuid,
    id: Uuid,
    approver_id: Uuid,
    note: Option<String>,
) -> ErpResult<()> {
    let r = fetch_request(engine, entity_id, id).await?;
    if r.status != "Pending" {
        return Err(ErpError::ValidationFailed { message: format!("Request is {}", r.status) });
    }
    let year = r.start_date.year();
    let mut tx = engine.pool().begin().await?;
    sqlx::query(
        "UPDATE leave_requests SET status='Approved', approver_id=$3, decided_at=NOW(), decision_note=$4 \
         WHERE id=$1 AND entity_id=$2",
    )
    .bind(id).bind(entity_id).bind(approver_id).bind(&note)
    .execute(&mut *tx).await?;
    // pending → taken
    sqlx::query(
        "UPDATE leave_balances SET pending_days = GREATEST(pending_days - $4, 0), \
         taken_days = taken_days + $4, updated_at = NOW() \
         WHERE employee_id=$1 AND leave_type_id=$2 AND year=$3",
    )
    .bind(r.employee_id).bind(r.leave_type_id).bind(year).bind(r.working_days)
    .execute(&mut *tx).await?;
    tx.commit().await?;
    Ok(())
}

pub async fn decline_leave(
    engine: &ErpEngine,
    entity_id: Uuid,
    id: Uuid,
    approver_id: Uuid,
    note: Option<String>,
) -> ErpResult<()> {
    let r = fetch_request(engine, entity_id, id).await?;
    if r.status != "Pending" {
        return Err(ErpError::ValidationFailed { message: format!("Request is {}", r.status) });
    }
    let year = r.start_date.year();
    let mut tx = engine.pool().begin().await?;
    sqlx::query(
        "UPDATE leave_requests SET status='Declined', approver_id=$3, decided_at=NOW(), decision_note=$4 \
         WHERE id=$1 AND entity_id=$2",
    )
    .bind(id).bind(entity_id).bind(approver_id).bind(&note)
    .execute(&mut *tx).await?;
    // release the pending reservation
    sqlx::query(
        "UPDATE leave_balances SET pending_days = GREATEST(pending_days - $4, 0), updated_at = NOW() \
         WHERE employee_id=$1 AND leave_type_id=$2 AND year=$3",
    )
    .bind(r.employee_id).bind(r.leave_type_id).bind(year).bind(r.working_days)
    .execute(&mut *tx).await?;
    tx.commit().await?;
    Ok(())
}

/// Cancel a request. Releases the reservation (pending) or restores balance
/// (approved). Allowed for the requesting employee or an admin.
pub async fn cancel_leave(engine: &ErpEngine, entity_id: Uuid, id: Uuid) -> ErpResult<()> {
    let r = fetch_request(engine, entity_id, id).await?;
    if r.status != "Pending" && r.status != "Approved" {
        return Err(ErpError::ValidationFailed { message: format!("Cannot cancel a {} request", r.status) });
    }
    let year = r.start_date.year();
    let was_approved = r.status == "Approved";
    let mut tx = engine.pool().begin().await?;
    sqlx::query("UPDATE leave_requests SET status='Cancelled' WHERE id=$1 AND entity_id=$2")
        .bind(id).bind(entity_id).execute(&mut *tx).await?;
    if was_approved {
        sqlx::query(
            "UPDATE leave_balances SET taken_days = GREATEST(taken_days - $4, 0), updated_at=NOW() \
             WHERE employee_id=$1 AND leave_type_id=$2 AND year=$3",
        ).bind(r.employee_id).bind(r.leave_type_id).bind(year).bind(r.working_days)
        .execute(&mut *tx).await?;
    } else {
        sqlx::query(
            "UPDATE leave_balances SET pending_days = GREATEST(pending_days - $4, 0), updated_at=NOW() \
             WHERE employee_id=$1 AND leave_type_id=$2 AND year=$3",
        ).bind(r.employee_id).bind(r.leave_type_id).bind(year).bind(r.working_days)
        .execute(&mut *tx).await?;
    }
    tx.commit().await?;
    Ok(())
}

/// List requests. When `employee_id` is set, scoped to that employee (ESS);
/// otherwise all requests for the tenant (admin/approver view).
pub async fn list_leave_requests(
    engine: &ErpEngine,
    entity_id: Uuid,
    employee_id: Option<Uuid>,
    status: Option<String>,
) -> ErpResult<Vec<LeaveRequestRow>> {
    let rows = sqlx::query_as::<_, LeaveRequestRow>(
        r#"SELECT * FROM leave_requests
           WHERE entity_id = $1
             AND ($2::uuid IS NULL OR employee_id = $2)
             AND ($3::text IS NULL OR status = $3)
           ORDER BY created_at DESC"#,
    )
    .bind(entity_id)
    .bind(employee_id)
    .bind(status)
    .fetch_all(engine.pool())
    .await?;
    Ok(rows)
}

/// Approved **unpaid**-leave working days for an employee within a period —
/// used by payroll to prorate salary.
pub async fn unpaid_leave_days(
    engine: &ErpEngine,
    entity_id: Uuid,
    employee_id: Uuid,
    from: NaiveDate,
    to: NaiveDate,
) -> ErpResult<Decimal> {
    let total: Option<Decimal> = sqlx::query_scalar(
        r#"SELECT COALESCE(SUM(lr.working_days), 0) FROM leave_requests lr
           JOIN leave_types lt ON lt.id = lr.leave_type_id
           WHERE lr.entity_id = $1 AND lr.employee_id = $2 AND lr.status = 'Approved'
             AND lt.paid = FALSE
             AND lr.start_date <= $4 AND lr.end_date >= $3"#,
    )
    .bind(entity_id)
    .bind(employee_id)
    .bind(from)
    .bind(to)
    .fetch_one(engine.pool())
    .await?;
    Ok(total.unwrap_or(Decimal::ZERO))
}
