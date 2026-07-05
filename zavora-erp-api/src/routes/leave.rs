//! HR leave & ESS routes.
//!
//! Two audiences:
//! - **Admin/HR** (`ROLES_HR_MANAGE` / `ROLES_LEAVE_APPROVE`): configure leave
//!   types & holidays, view all requests/balances, approve/decline.
//! - **Employee self-service** (`/me/*`): a signed-in employee acting only on
//!   their **own** records — resolved from `employees.user_id = ctx.user_id`
//!   and never trusting a client-supplied employee id.

use std::sync::Arc;
use axum::{extract::{Path, Query, State}, Json};
use chrono::Datelike;
use serde::Deserialize;
use uuid::Uuid;

use crate::AppState;
use super::err_response;
use crate::middleware::auth::{require_role, AuthContext, ROLES_HR_MANAGE, ROLES_LEAVE_APPROVE};
use crate::middleware::staff_auth::StaffContext;
use zavora_erp_core::hr::*;
use zavora_erp_core::services::leave as svc;
use zavora_erp_core::ErpError;

type ApiResult = Result<Json<serde_json::Value>, axum::response::Response>;
fn er(e: ErpError) -> axum::response::Response { use axum::response::IntoResponse; err_response(e).into_response() }

#[derive(Deserialize)]
pub struct YearQuery { pub year: Option<i32>, pub employee_id: Option<Uuid>, pub status: Option<String>, #[serde(default)] pub mine: bool }

fn this_year() -> i32 { chrono::Utc::now().year() }

// ─── Leave types (admin) ─────────────────────────────────────────────────────

pub async fn list_types(ctx: AuthContext, State(state): State<Arc<AppState>>) -> ApiResult {
    // Seed Kenyan defaults on first access so the tenant is never empty.
    svc::seed_default_leave_types(&state.engine, ctx.entity_id).await.map_err(er)?;
    let rows = svc::list_leave_types(&state.engine, ctx.entity_id).await.map_err(er)?;
    Ok(Json(serde_json::to_value(rows).unwrap_or_default()))
}

pub async fn create_type(ctx: AuthContext, State(state): State<Arc<AppState>>, Json(req): Json<CreateLeaveTypeRequest>) -> ApiResult {
    require_role(ROLES_HR_MANAGE, &ctx, "manage leave types").map_err(er)?;
    let id = svc::create_leave_type(&state.engine, ctx.entity_id, req).await.map_err(er)?;
    Ok(Json(serde_json::json!({"id": id})))
}

#[derive(Deserialize)]
pub struct ActivePatch { pub active: bool }
pub async fn set_type_active(ctx: AuthContext, State(state): State<Arc<AppState>>, Path(id): Path<Uuid>, Json(p): Json<ActivePatch>) -> ApiResult {
    require_role(ROLES_HR_MANAGE, &ctx, "manage leave types").map_err(er)?;
    svc::set_leave_type_active(&state.engine, ctx.entity_id, id, p.active).await.map_err(er)?;
    Ok(Json(serde_json::json!({"status": "ok"})))
}

// ─── Holidays (admin) ────────────────────────────────────────────────────────

pub async fn list_holidays(ctx: AuthContext, State(state): State<Arc<AppState>>) -> ApiResult {
    let rows = svc::list_holidays(&state.engine, ctx.entity_id).await.map_err(er)?;
    Ok(Json(serde_json::to_value(rows).unwrap_or_default()))
}
pub async fn create_holiday(ctx: AuthContext, State(state): State<Arc<AppState>>, Json(req): Json<CreateHolidayRequest>) -> ApiResult {
    require_role(ROLES_HR_MANAGE, &ctx, "manage holidays").map_err(er)?;
    let id = svc::create_holiday(&state.engine, ctx.entity_id, req).await.map_err(er)?;
    Ok(Json(serde_json::json!({"id": id})))
}
pub async fn delete_holiday(ctx: AuthContext, State(state): State<Arc<AppState>>, Path(id): Path<Uuid>) -> ApiResult {
    require_role(ROLES_HR_MANAGE, &ctx, "manage holidays").map_err(er)?;
    svc::delete_holiday(&state.engine, ctx.entity_id, id).await.map_err(er)?;
    Ok(Json(serde_json::json!({"status": "ok"})))
}

// ─── Balances & requests (admin) ─────────────────────────────────────────────

pub async fn list_balances(ctx: AuthContext, State(state): State<Arc<AppState>>, Query(q): Query<YearQuery>) -> ApiResult {
    require_role(ROLES_HR_MANAGE, &ctx, "view leave balances").map_err(er)?;
    let emp = q.employee_id.ok_or_else(|| er(ErpError::ValidationFailed { message: "employee_id required".into() }))?;
    let rows = svc::list_balances(&state.engine, ctx.entity_id, emp, q.year.unwrap_or_else(this_year)).await.map_err(er)?;
    Ok(Json(serde_json::to_value(rows).unwrap_or_default()))
}

pub async fn list_requests(ctx: AuthContext, State(state): State<Arc<AppState>>, Query(q): Query<YearQuery>) -> ApiResult {
    require_role(ROLES_LEAVE_APPROVE, &ctx, "view leave requests").map_err(er)?;
    let assigned = if q.mine { Some(ctx.user_id) } else { None };
    let rows = svc::list_leave_requests(&state.engine, ctx.entity_id, q.employee_id, q.status, assigned).await.map_err(er)?;
    Ok(Json(serde_json::to_value(rows).unwrap_or_default()))
}

/// Admin creates a request on behalf of an employee (employee_id required).
pub async fn create_request(ctx: AuthContext, State(state): State<Arc<AppState>>, Json(req): Json<CreateLeaveRequest>) -> ApiResult {
    require_role(ROLES_HR_MANAGE, &ctx, "create leave request").map_err(er)?;
    let emp = req.employee_id.ok_or_else(|| er(ErpError::ValidationFailed { message: "employee_id required".into() }))?;
    let id = svc::create_leave_request(&state.engine, ctx.entity_id, emp, req).await.map_err(er)?;
    Ok(Json(serde_json::json!({"id": id})))
}

pub async fn approve(ctx: AuthContext, State(state): State<Arc<AppState>>, Path(id): Path<Uuid>, Json(d): Json<DecideLeaveRequest>) -> ApiResult {
    require_role(ROLES_LEAVE_APPROVE, &ctx, "approve leave").map_err(er)?;
    svc::approve_leave(&state.engine, ctx.entity_id, id, ctx.user_id, d.note).await.map_err(er)?;
    Ok(Json(serde_json::json!({"status": "approved"})))
}
pub async fn decline(ctx: AuthContext, State(state): State<Arc<AppState>>, Path(id): Path<Uuid>, Json(d): Json<DecideLeaveRequest>) -> ApiResult {
    require_role(ROLES_LEAVE_APPROVE, &ctx, "decline leave").map_err(er)?;
    svc::decline_leave(&state.engine, ctx.entity_id, id, ctx.user_id, d.note).await.map_err(er)?;
    Ok(Json(serde_json::json!({"status": "declined"})))
}

// ─── Employee self-service (/api/v1/staff/*) ─────────────────────────────────
// Gated by StaffContext (employee_users principal, role 'Employee'). All queries
// are scoped to ctx.employee_id — an employee only ever sees their own records.

/// Active leave types for the staff portal (names for the request form / labels).
pub async fn my_leave_types(ctx: StaffContext, State(state): State<Arc<AppState>>) -> ApiResult {
    svc::seed_default_leave_types(&state.engine, ctx.entity_id).await.map_err(er)?;
    let rows = svc::list_leave_types(&state.engine, ctx.entity_id).await.map_err(er)?;
    Ok(Json(serde_json::to_value(rows).unwrap_or_default()))
}

pub async fn my_leave_balances(ctx: StaffContext, State(state): State<Arc<AppState>>, Query(q): Query<YearQuery>) -> ApiResult {
    svc::seed_default_leave_types(&state.engine, ctx.entity_id).await.map_err(er)?;
    let rows = svc::list_balances(&state.engine, ctx.entity_id, ctx.employee_id, q.year.unwrap_or_else(this_year)).await.map_err(er)?;
    Ok(Json(serde_json::to_value(rows).unwrap_or_default()))
}

pub async fn my_leave_requests(ctx: StaffContext, State(state): State<Arc<AppState>>) -> ApiResult {
    let rows = svc::list_leave_requests(&state.engine, ctx.entity_id, Some(ctx.employee_id), None, None).await.map_err(er)?;
    Ok(Json(serde_json::to_value(rows).unwrap_or_default()))
}

/// Employee submits their own leave request (employee scope from the token).
pub async fn my_create_request(ctx: StaffContext, State(state): State<Arc<AppState>>, Json(req): Json<CreateLeaveRequest>) -> ApiResult {
    let id = svc::create_leave_request(&state.engine, ctx.entity_id, ctx.employee_id, req).await.map_err(er)?;
    Ok(Json(serde_json::json!({"id": id})))
}

/// Employee cancels their own request (ownership verified against the token).
pub async fn my_cancel_request(ctx: StaffContext, State(state): State<Arc<AppState>>, Path(id): Path<Uuid>) -> ApiResult {
    let owner: Option<Uuid> = sqlx::query_scalar(
        "SELECT employee_id FROM leave_requests WHERE id = $1 AND entity_id = $2",
    ).bind(id).bind(ctx.entity_id).fetch_optional(state.engine.pool()).await.map_err(|e| er(ErpError::Database(e)))?;
    if owner != Some(ctx.employee_id) {
        return Err(er(ErpError::PermissionDenied { action: "cancel leave".into(), required_role: "owner of the request".into() }));
    }
    svc::cancel_leave(&state.engine, ctx.entity_id, id).await.map_err(er)?;
    Ok(Json(serde_json::json!({"status": "cancelled"})))
}

/// The signed-in employee's own profile record.
pub async fn my_profile(ctx: StaffContext, State(state): State<Arc<AppState>>) -> ApiResult {
    let row = sqlx::query_as::<_, zavora_erp_core::parties::EmployeeRow>(
        "SELECT * FROM employees WHERE id = $1 AND entity_id = $2",
    ).bind(ctx.employee_id).bind(ctx.entity_id)
    .fetch_optional(state.engine.pool()).await.map_err(|e| er(ErpError::Database(e)))?;
    match row {
        Some(r) => Ok(Json(serde_json::to_value(r).unwrap_or_default())),
        None => Err(er(ErpError::NotFound { entity_type: "Employee".into(), id: ctx.employee_id })),
    }
}

#[derive(Deserialize)]
pub struct MyProfilePatch { pub phone: Option<String>, pub personal_email: Option<String> }

/// ESS profile edit — limited to non-payroll contact fields (phone, personal
/// email). Payroll-sensitive fields (salary, KRA PIN, bank) are HR-only.
pub async fn my_profile_update(ctx: StaffContext, State(state): State<Arc<AppState>>, Json(p): Json<MyProfilePatch>) -> ApiResult {
    if let Some(phone) = p.phone {
        sqlx::query("UPDATE employees SET phone = $1 WHERE id = $2 AND entity_id = $3")
            .bind(phone).bind(ctx.employee_id).bind(ctx.entity_id).execute(state.engine.pool()).await.map_err(|e| er(ErpError::Database(e)))?;
    }
    if let Some(email) = p.personal_email {
        sqlx::query("UPDATE employees SET personal_email = $1 WHERE id = $2 AND entity_id = $3")
            .bind(email).bind(ctx.employee_id).bind(ctx.entity_id).execute(state.engine.pool()).await.map_err(|e| er(ErpError::Database(e)))?;
    }
    Ok(Json(serde_json::json!({"status": "ok"})))
}

/// The signed-in employee's payslips (from posted/paid pay runs).
pub async fn my_payslips(ctx: StaffContext, State(state): State<Arc<AppState>>) -> ApiResult {
    let rows = sqlx::query(
        r#"SELECT pr.id AS pay_run_id, pr.pay_date, pr.status, ps.deductions,
                  ps.custom_deductions, ps.custom_earnings
           FROM payslips ps JOIN pay_runs pr ON pr.id = ps.pay_run_id
           WHERE ps.employee_id = $1 AND pr.entity_id = $2
             AND pr.status IN ('posted','paid','approved')
           ORDER BY pr.pay_date DESC"#,
    )
    .bind(ctx.employee_id).bind(ctx.entity_id)
    .fetch_all(state.engine.pool()).await;
    match rows {
        Ok(rows) => {
            use sqlx::Row;
            let list: Vec<serde_json::Value> = rows.iter().map(|r| serde_json::json!({
                "pay_run_id": r.get::<Uuid, _>("pay_run_id"),
                "pay_date": r.get::<chrono::NaiveDate, _>("pay_date").to_string(),
                "status": r.get::<String, _>("status"),
                "deductions": r.get::<serde_json::Value, _>("deductions"),
                "custom_deductions": r.get::<serde_json::Value, _>("custom_deductions"),
                "custom_earnings": r.get::<serde_json::Value, _>("custom_earnings"),
            })).collect();
            Ok(Json(serde_json::json!(list)))
        }
        Err(e) => Err(er(ErpError::Database(e))),
    }
}

// ─── ESS invite (admin / HR) ─────────────────────────────────────────────────

/// HR links an employee to a self-service login: create/reuse an `employee_users`
/// account (separate principal, NOT era_users) linked via employee_id. When a
/// password is supplied the account is `active` and can sign in immediately;
/// otherwise it lands as `invited`. Idempotent on email within the tenant.
pub async fn invite_ess(ctx: AuthContext, State(state): State<Arc<AppState>>, Path(employee_id): Path<Uuid>, Json(req): Json<InviteStaffRequest>) -> ApiResult {
    require_role(ROLES_HR_MANAGE, &ctx, "invite employee to self-service").map_err(er)?;

    let emp = sqlx::query_as::<_, zavora_erp_core::parties::EmployeeRow>(
        "SELECT * FROM employees WHERE id = $1 AND entity_id = $2",
    ).bind(employee_id).bind(ctx.entity_id)
    .fetch_optional(state.engine.pool()).await.map_err(|e| er(ErpError::Database(e)))?
    .ok_or_else(|| er(ErpError::NotFound { entity_type: "Employee".into(), id: employee_id }))?;

    let (password_hash, status) = match req.password.as_deref() {
        Some(pw) if pw.len() >= 8 => (Some(zavora_erp_core::auth::hash_password(pw).map_err(er)?), "active"),
        Some(_) => return Err(er(ErpError::ValidationFailed { message: "Password must be at least 8 characters".into() })),
        None => (None, "invited"),
    };

    // Reuse an existing employee_users row for this email, else create it.
    let existing: Option<Uuid> = sqlx::query_scalar(
        "SELECT id FROM employee_users WHERE entity_id = $1 AND lower(email) = lower($2)",
    ).bind(ctx.entity_id).bind(&req.email).fetch_optional(state.engine.pool()).await.map_err(|e| er(ErpError::Database(e)))?;

    let staff_id = match existing {
        Some(id) => {
            sqlx::query("UPDATE employee_users SET employee_id = $1, status = $2, password_hash = COALESCE($3, password_hash) WHERE id = $4")
                .bind(employee_id).bind(status).bind(&password_hash).bind(id)
                .execute(state.engine.pool()).await.map_err(|e| er(ErpError::Database(e)))?;
            id
        }
        None => {
            let id = Uuid::new_v4();
            sqlx::query(
                "INSERT INTO employee_users (id, entity_id, email, display_name, password_hash, status, employee_id) \
                 VALUES ($1,$2,$3,$4,$5,$6,$7)",
            )
            .bind(id).bind(ctx.entity_id).bind(req.email.trim().to_lowercase()).bind(&emp.full_name)
            .bind(&password_hash).bind(status).bind(employee_id)
            .execute(state.engine.pool()).await.map_err(|e| er(ErpError::Database(e)))?;
            id
        }
    };

    Ok(Json(serde_json::json!({"employee_user_id": staff_id, "email": req.email, "status": status})))
}
