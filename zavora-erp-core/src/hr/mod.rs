//! HR & People domain — Phase 1: leave management + ESS foundation.
//!
//! Models for leave types, balances, requests, and holidays, plus the pure
//! working-days calculation used to size a leave request. Business logic
//! (persistence, balance transitions, approvals) lives in
//! `crate::services::leave`.

use chrono::{Datelike, NaiveDate, Weekday, DateTime, Utc};
use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

/// How a leave type's entitlement is granted.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AccrualMethod {
    /// Full annual entitlement available from the start of the year.
    FixedAnnual,
    /// Entitlement accrues in equal monthly parts (Kenyan default for annual leave).
    MonthlyAccrual,
    /// No cap (e.g. some unpaid or compassionate policies).
    Unlimited,
}

/// Lifecycle of a leave request.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum LeaveStatus {
    Pending,
    Approved,
    Declined,
    Cancelled,
}

// ─── Leave type ──────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LeaveType {
    pub id: Uuid,
    pub entity_id: Uuid,
    pub name: String,
    pub code: String,
    pub paid: bool,
    pub accrual_method: AccrualMethod,
    pub days_per_year: Decimal,
    pub carryover_max: Decimal,
    pub requires_attachment: bool,
    pub is_statutory: bool,
    pub active: bool,
}

#[derive(Debug, Clone, FromRow, Serialize)]
pub struct LeaveTypeRow {
    pub id: Uuid,
    pub entity_id: Uuid,
    pub name: String,
    pub code: String,
    pub paid: bool,
    pub accrual_method: String,
    pub days_per_year: Decimal,
    pub carryover_max: Decimal,
    pub requires_attachment: bool,
    pub is_statutory: bool,
    pub active: bool,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateLeaveTypeRequest {
    pub name: String,
    pub code: String,
    #[serde(default = "default_true")]
    pub paid: bool,
    pub accrual_method: AccrualMethod,
    pub days_per_year: Decimal,
    #[serde(default)]
    pub carryover_max: Decimal,
    #[serde(default)]
    pub requires_attachment: bool,
    #[serde(default)]
    pub is_statutory: bool,
}

fn default_true() -> bool {
    true
}

// ─── Leave balance ─────────────────────────────────────────────────────────

#[derive(Debug, Clone, FromRow, Serialize)]
pub struct LeaveBalanceRow {
    pub id: Uuid,
    pub entity_id: Uuid,
    pub employee_id: Uuid,
    pub leave_type_id: Uuid,
    pub year: i32,
    pub entitled_days: Decimal,
    pub accrued_days: Decimal,
    pub taken_days: Decimal,
    pub pending_days: Decimal,
    pub carried_over: Decimal,
    pub updated_at: DateTime<Utc>,
}

// ─── Leave request ─────────────────────────────────────────────────────────

#[derive(Debug, Clone, FromRow, Serialize)]
pub struct LeaveRequestRow {
    pub id: Uuid,
    pub entity_id: Uuid,
    pub employee_id: Uuid,
    pub leave_type_id: Uuid,
    pub start_date: NaiveDate,
    pub end_date: NaiveDate,
    pub half_day_start: bool,
    pub half_day_end: bool,
    pub working_days: Decimal,
    pub reason: Option<String>,
    pub attachment_url: Option<String>,
    pub status: String,
    pub approver_id: Option<Uuid>,
    pub decided_at: Option<DateTime<Utc>>,
    pub decision_note: Option<String>,
    pub created_at: DateTime<Utc>,
    #[serde(default)]
    pub assigned_approver_id: Option<Uuid>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateLeaveRequest {
    /// Omitted for ESS self-service (derived from the caller); required for admin.
    #[serde(default)]
    pub employee_id: Option<Uuid>,
    pub leave_type_id: Uuid,
    pub start_date: NaiveDate,
    pub end_date: NaiveDate,
    #[serde(default)]
    pub half_day_start: bool,
    #[serde(default)]
    pub half_day_end: bool,
    #[serde(default)]
    pub reason: Option<String>,
    #[serde(default)]
    pub attachment_url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DecideLeaveRequest {
    #[serde(default)]
    pub note: Option<String>,
}

// ─── Holiday ─────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, FromRow, Serialize)]
pub struct HolidayRow {
    pub id: Uuid,
    pub entity_id: Uuid,
    pub date: NaiveDate,
    pub name: String,
    pub recurring: bool,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateHolidayRequest {
    pub date: NaiveDate,
    pub name: String,
    #[serde(default)]
    pub recurring: bool,
}

// ─── Employee self-service principal (employee_users) ───────────────────────
// A separate principal class from back-office `era_users`, mirroring
// `vendor_users`. Logins carry a distinct 'Employee' JWT role.

#[derive(Debug, Clone, FromRow, Serialize)]
pub struct EmployeeUserRow {
    pub id: Uuid,
    pub entity_id: Uuid,
    pub email: String,
    pub display_name: String,
    pub status: String, // invited|active|suspended
    pub employee_id: Option<Uuid>,
    pub last_login: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StaffLoginRequest {
    pub email: String,
    pub password: String,
}

/// HR invites an employee to self-service. Optionally sets an initial password
/// (account immediately `active`); otherwise the account is `invited` until a
/// password is set.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InviteStaffRequest {
    pub email: String,
    #[serde(default)]
    pub password: Option<String>,
}

// ─── Onboarding / offboarding cases ──────────────────────────────────────────

#[derive(Debug, Clone, FromRow, Serialize)]
pub struct OnboardingCaseRow {
    pub id: Uuid,
    pub entity_id: Uuid,
    pub employee_id: Uuid,
    pub r#type: String,
    pub status: String,
    pub start_date: NaiveDate,
    pub target_date: Option<NaiveDate>,
    pub probation_end: Option<NaiveDate>,
    pub notes: Option<String>,
    pub created_by: Option<Uuid>,
    pub created_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, FromRow, Serialize)]
pub struct OnboardingTaskRow {
    pub id: Uuid,
    pub entity_id: Uuid,
    pub case_id: Uuid,
    pub title: String,
    pub is_done: bool,
    pub done_at: Option<DateTime<Utc>>,
    pub sort_order: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateOnboardingRequest {
    pub employee_id: Uuid,
    pub start_date: NaiveDate,
    #[serde(default)]
    pub target_date: Option<NaiveDate>,
    #[serde(default)]
    pub probation_end: Option<NaiveDate>,
    #[serde(default)]
    pub notes: Option<String>,
    #[serde(default)]
    pub tasks: Option<Vec<String>>,
}

/// Default Kenyan-SME onboarding checklist.
pub fn default_onboarding_tasks() -> Vec<&'static str> {
    vec![
        "Signed employment contract on file",
        "KRA PIN recorded",
        "NSSF & SHA numbers recorded",
        "Bank account details captured",
        "Statutory IDs verified (ID/passport)",
        "Workstation & equipment issued",
        "Email & system access provisioned",
        "Employee self-service invite sent",
        "Company induction completed",
    ]
}

// ─── Working-days calculation (pure) ─────────────────────────────────────────

/// Count the working days a leave request consumes: every calendar day from
/// `start` to `end` inclusive, excluding weekends and the given `holidays`.
/// Half-day flags on the first/last day each subtract 0.5 (only when that day
/// is itself a working day). Returns a `Decimal` so half-days are exact.
pub fn working_days(
    start: NaiveDate,
    end: NaiveDate,
    half_day_start: bool,
    half_day_end: bool,
    holidays: &[NaiveDate],
) -> Decimal {
    if end < start {
        return Decimal::ZERO;
    }
    let is_working = |d: NaiveDate| {
        !matches!(d.weekday(), Weekday::Sat | Weekday::Sun) && !holidays.contains(&d)
    };

    let mut total = Decimal::ZERO;
    let mut d = start;
    while d <= end {
        if is_working(d) {
            total += dec!(1);
        }
        d = d.succ_opt().unwrap();
    }

    // Half-day adjustments (guard against over-subtracting a zero-day request).
    if half_day_start && is_working(start) {
        total -= dec!(0.5);
    }
    if half_day_end && end != start && is_working(end) {
        total -= dec!(0.5);
    }
    if total < Decimal::ZERO {
        Decimal::ZERO
    } else {
        total
    }
}

/// Kenyan-common default leave types, seeded per tenant on first use. These
/// reflect commonly-cited Employment Act 2007 provisions and are fully editable
/// — the system treats them as defaults, not legal advice.
pub fn kenyan_default_leave_types() -> Vec<CreateLeaveTypeRequest> {
    vec![
        CreateLeaveTypeRequest {
            name: "Annual Leave".into(),
            code: "ANNUAL".into(),
            paid: true,
            accrual_method: AccrualMethod::MonthlyAccrual,
            days_per_year: dec!(21),
            carryover_max: dec!(0),
            requires_attachment: false,
            is_statutory: true,
        },
        CreateLeaveTypeRequest {
            name: "Sick Leave".into(),
            code: "SICK".into(),
            paid: true,
            accrual_method: AccrualMethod::FixedAnnual,
            days_per_year: dec!(14),
            carryover_max: dec!(0),
            requires_attachment: true,
            is_statutory: true,
        },
        CreateLeaveTypeRequest {
            name: "Maternity Leave".into(),
            code: "MATERNITY".into(),
            paid: true,
            accrual_method: AccrualMethod::FixedAnnual,
            days_per_year: dec!(90),
            carryover_max: dec!(0),
            requires_attachment: true,
            is_statutory: true,
        },
        CreateLeaveTypeRequest {
            name: "Paternity Leave".into(),
            code: "PATERNITY".into(),
            paid: true,
            accrual_method: AccrualMethod::FixedAnnual,
            days_per_year: dec!(14),
            carryover_max: dec!(0),
            requires_attachment: false,
            is_statutory: true,
        },
        CreateLeaveTypeRequest {
            name: "Compassionate Leave".into(),
            code: "COMPASSIONATE".into(),
            paid: true,
            accrual_method: AccrualMethod::FixedAnnual,
            days_per_year: dec!(3),
            carryover_max: dec!(0),
            requires_attachment: false,
            is_statutory: false,
        },
        CreateLeaveTypeRequest {
            name: "Unpaid Leave".into(),
            code: "UNPAID".into(),
            paid: false,
            accrual_method: AccrualMethod::Unlimited,
            days_per_year: dec!(0),
            carryover_max: dec!(0),
            requires_attachment: false,
            is_statutory: false,
        },
    ]
}

impl AccrualMethod {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::FixedAnnual => "FixedAnnual",
            Self::MonthlyAccrual => "MonthlyAccrual",
            Self::Unlimited => "Unlimited",
        }
    }
    pub fn parse(s: &str) -> Self {
        match s {
            "FixedAnnual" => Self::FixedAnnual,
            "Unlimited" => Self::Unlimited,
            _ => Self::MonthlyAccrual,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn d(y: i32, m: u32, day: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(y, m, day).unwrap()
    }

    #[test]
    fn full_working_week_is_five_days() {
        // Mon 2025-06-02 .. Fri 2025-06-06
        assert_eq!(working_days(d(2025, 6, 2), d(2025, 6, 6), false, false, &[]), dec!(5));
    }

    #[test]
    fn weekend_is_excluded() {
        // Fri..Mon spans a weekend → Fri + Mon = 2 working days.
        assert_eq!(working_days(d(2025, 6, 6), d(2025, 6, 9), false, false, &[]), dec!(2));
    }

    #[test]
    fn holidays_are_excluded() {
        // Mon..Fri with Wed a holiday = 4 days.
        let hol = vec![d(2025, 6, 4)];
        assert_eq!(working_days(d(2025, 6, 2), d(2025, 6, 6), false, false, &hol), dec!(4));
    }

    #[test]
    fn half_days_subtract_half_each() {
        // Mon..Fri, half start + half end = 5 - 1 = 4.
        assert_eq!(working_days(d(2025, 6, 2), d(2025, 6, 6), true, true, &[]), dec!(4));
    }

    #[test]
    fn single_half_day() {
        // One day, half-day start only = 0.5.
        assert_eq!(working_days(d(2025, 6, 2), d(2025, 6, 2), true, false, &[]), dec!(0.5));
    }

    #[test]
    fn all_weekend_is_zero() {
        // Sat..Sun = 0 working days.
        assert_eq!(working_days(d(2025, 6, 7), d(2025, 6, 8), false, false, &[]), dec!(0));
    }
}
