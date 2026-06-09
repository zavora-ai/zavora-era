use chrono::{Datelike, NaiveDate, Utc};
use uuid::Uuid;

use crate::engine::ErpEngine;
use crate::error::{ErpError, ErpResult};
use crate::period::*;
use crate::types::AgentOrUserId;

/// Generate fiscal periods for a year.
pub async fn generate_periods(
    engine: &ErpEngine,
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
        .bind(engine.entity_id())
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
            entity_id: engine.entity_id(),
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
pub async fn close_period(engine: &ErpEngine, req: ClosePeriodRequest) -> ErpResult<FiscalPeriod> {
    let period = get_period(engine, req.period_id).await?;

    // Validate current state allows closing
    match period.parsed_status() {
        PeriodStatus::Open => {} // Can soft or hard close
        PeriodStatus::SoftClosed => {
            if req.close_type == PeriodCloseType::Soft {
                return Err(ErpError::ValidationFailed {
                    message: "Period is already soft-closed".to_string(),
                });
            }
            // Can hard close from soft-closed
        }
        PeriodStatus::HardClosed => {
            return Err(ErpError::ValidationFailed {
                message: "Period is already hard-closed and cannot be modified".to_string(),
            });
        }
        PeriodStatus::Future => {
            return Err(ErpError::ValidationFailed {
                message: "Cannot close a future period".to_string(),
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

    let mut updated = period;
    updated.status = new_status.to_string();
    updated.closed_at = Some(now);
    updated.closed_by = Some(serde_json::to_value(&req.closed_by).unwrap_or_default());

    Ok(updated)
}

/// Reopen a soft-closed period.
pub async fn reopen_period(engine: &ErpEngine, req: ReopenPeriodRequest) -> ErpResult<FiscalPeriod> {
    let period = get_period(engine, req.period_id).await?;

    if period.parsed_status() != PeriodStatus::SoftClosed {
        return Err(ErpError::ValidationFailed {
            message: "Only soft-closed periods can be reopened".to_string(),
        });
    }

    sqlx::query(
        "UPDATE fiscal_periods SET status = 'open', closed_by = NULL, closed_at = NULL WHERE id = $1",
    )
    .bind(req.period_id)
    .execute(engine.pool())
    .await?;

    let mut updated = period;
    updated.status = "open".to_string();
    updated.closed_by = None;
    updated.closed_at = None;

    Ok(updated)
}

/// Get a fiscal period by ID.
pub async fn get_period(engine: &ErpEngine, period_id: Uuid) -> ErpResult<FiscalPeriod> {
    sqlx::query_as::<_, FiscalPeriod>("SELECT * FROM fiscal_periods WHERE id = $1")
        .bind(period_id)
        .fetch_optional(engine.pool())
        .await?
        .ok_or_else(|| ErpError::NotFound {
            entity_type: "FiscalPeriod".to_string(),
            id: period_id,
        })
}

/// Get the period for a specific date.
pub async fn period_for_date(engine: &ErpEngine, date: NaiveDate) -> ErpResult<FiscalPeriod> {
    sqlx::query_as::<_, FiscalPeriod>(
        "SELECT * FROM fiscal_periods WHERE entity_id = $1 AND start_date <= $2 AND end_date >= $2",
    )
    .bind(engine.entity_id())
    .bind(date)
    .fetch_optional(engine.pool())
    .await?
    .ok_or_else(|| ErpError::ValidationFailed {
        message: format!("No fiscal period found for date {}", date),
    })
}

/// List all periods for the entity.
pub async fn list_periods(engine: &ErpEngine) -> ErpResult<Vec<FiscalPeriod>> {
    let periods = sqlx::query_as::<_, FiscalPeriod>(
        "SELECT * FROM fiscal_periods WHERE entity_id = $1 ORDER BY start_date",
    )
    .bind(engine.entity_id())
    .fetch_all(engine.pool())
    .await?;

    Ok(periods)
}
