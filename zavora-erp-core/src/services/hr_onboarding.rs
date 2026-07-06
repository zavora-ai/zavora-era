//! Onboarding (and offboarding) case management: a case tracks a hire/leaver
//! through a checklist. New-hire cases seed a default onboarding checklist.

use chrono::Utc;
use uuid::Uuid;

use crate::engine::ErpEngine;
use crate::error::{ErpError, ErpResult};
use crate::hr::*;

/// Create an onboarding case for an employee with a checklist (default template
/// unless custom tasks are supplied).
pub async fn create_onboarding(
    engine: &ErpEngine,
    entity_id: Uuid,
    created_by: Option<Uuid>,
    req: CreateOnboardingRequest,
) -> ErpResult<Uuid> {
    // Guard: one active onboarding case per employee.
    let existing: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM onboarding_cases WHERE entity_id=$1 AND employee_id=$2 AND type='Onboarding' AND status='InProgress'",
    )
    .bind(entity_id).bind(req.employee_id).fetch_one(engine.pool()).await?;
    if existing > 0 {
        return Err(ErpError::ValidationFailed { message: "This employee already has an onboarding in progress".into() });
    }

    let case_id = Uuid::new_v4();
    let mut tx = engine.pool().begin().await?;
    sqlx::query(
        r#"INSERT INTO onboarding_cases
           (id, entity_id, employee_id, type, status, start_date, target_date, probation_end, notes, created_by, created_at)
           VALUES ($1,$2,$3,'Onboarding','InProgress',$4,$5,$6,$7,$8,NOW())"#,
    )
    .bind(case_id).bind(entity_id).bind(req.employee_id)
    .bind(req.start_date).bind(req.target_date).bind(req.probation_end).bind(&req.notes).bind(created_by)
    .execute(&mut *tx).await?;

    let titles: Vec<String> = match req.tasks {
        Some(t) if !t.is_empty() => t,
        _ => default_onboarding_tasks().into_iter().map(String::from).collect(),
    };
    for (i, title) in titles.iter().enumerate() {
        sqlx::query(
            "INSERT INTO onboarding_tasks (id, entity_id, case_id, title, sort_order) VALUES ($1,$2,$3,$4,$5)",
        )
        .bind(Uuid::new_v4()).bind(entity_id).bind(case_id).bind(title).bind(i as i32)
        .execute(&mut *tx).await?;
    }
    tx.commit().await?;
    Ok(case_id)
}

pub async fn list_cases(engine: &ErpEngine, entity_id: Uuid, case_type: &str) -> ErpResult<serde_json::Value> {
    use sqlx::Row;
    let rows = sqlx::query(
        r#"SELECT c.*, e.full_name, e.job_title,
                  (SELECT COUNT(*) FROM onboarding_tasks t WHERE t.case_id=c.id) AS total,
                  (SELECT COUNT(*) FROM onboarding_tasks t WHERE t.case_id=c.id AND t.is_done) AS done
           FROM onboarding_cases c JOIN employees e ON e.id=c.employee_id
           WHERE c.entity_id=$1 AND c.type=$2 ORDER BY c.created_at DESC"#,
    )
    .bind(entity_id).bind(case_type).fetch_all(engine.pool()).await?;
    let list: Vec<serde_json::Value> = rows.iter().map(|r| serde_json::json!({
        "id": r.get::<Uuid,_>("id"),
        "employee_id": r.get::<Uuid,_>("employee_id"),
        "employee_name": r.get::<String,_>("full_name"),
        "job_title": r.get::<Option<String>,_>("job_title"),
        "status": r.get::<String,_>("status"),
        "start_date": r.get::<chrono::NaiveDate,_>("start_date").to_string(),
        "target_date": r.get::<Option<chrono::NaiveDate>,_>("target_date").map(|d| d.to_string()),
        "probation_end": r.get::<Option<chrono::NaiveDate>,_>("probation_end").map(|d| d.to_string()),
        "total": r.get::<i64,_>("total"),
        "done": r.get::<i64,_>("done"),
    })).collect();
    Ok(serde_json::json!(list))
}

pub async fn get_case(engine: &ErpEngine, entity_id: Uuid, case_id: Uuid) -> ErpResult<serde_json::Value> {
    let case = sqlx::query_as::<_, OnboardingCaseRow>(
        "SELECT * FROM onboarding_cases WHERE id=$1 AND entity_id=$2",
    )
    .bind(case_id).bind(entity_id).fetch_optional(engine.pool()).await?
    .ok_or_else(|| ErpError::NotFound { entity_type: "OnboardingCase".into(), id: case_id })?;
    let tasks = sqlx::query_as::<_, OnboardingTaskRow>(
        "SELECT * FROM onboarding_tasks WHERE case_id=$1 ORDER BY sort_order",
    )
    .bind(case_id).fetch_all(engine.pool()).await?;
    Ok(serde_json::json!({ "case": case, "tasks": tasks }))
}

pub async fn set_task_done(engine: &ErpEngine, entity_id: Uuid, task_id: Uuid, done: bool) -> ErpResult<()> {
    sqlx::query(
        "UPDATE onboarding_tasks SET is_done=$3, done_at=CASE WHEN $3 THEN NOW() ELSE NULL END WHERE id=$1 AND entity_id=$2",
    )
    .bind(task_id).bind(entity_id).bind(done).execute(engine.pool()).await?;
    Ok(())
}

/// Complete a case (marks status Complete). If onboarding, ensures the employee
/// is active.
pub async fn complete_case(engine: &ErpEngine, entity_id: Uuid, case_id: Uuid) -> ErpResult<()> {
    let case = sqlx::query_as::<_, OnboardingCaseRow>(
        "SELECT * FROM onboarding_cases WHERE id=$1 AND entity_id=$2",
    )
    .bind(case_id).bind(entity_id).fetch_optional(engine.pool()).await?
    .ok_or_else(|| ErpError::NotFound { entity_type: "OnboardingCase".into(), id: case_id })?;
    let mut tx = engine.pool().begin().await?;
    sqlx::query("UPDATE onboarding_cases SET status='Complete', completed_at=NOW() WHERE id=$1 AND entity_id=$2")
        .bind(case_id).bind(entity_id).execute(&mut *tx).await?;
    if case.r#type == "Onboarding" {
        sqlx::query("UPDATE employees SET is_active=true WHERE id=$1 AND entity_id=$2")
            .bind(case.employee_id).bind(entity_id).execute(&mut *tx).await?;
    }
    tx.commit().await?;
    Ok(())
}
