# Zavora ERP — HR & People Module Specification

> **Status:** Draft for review · **Owner:** Product/Eng · **Last updated:** 2026-07-05
>
> This spec turns the current **payroll-only** module into a full **HR & People**
> module covering the complete employee lifecycle — recruitment, hiring,
> onboarding, records, leave, attendance, self-service, and offboarding — with a
> Kenyan-SME focus and clean integration into the existing payroll engine.

---

## 1. Why (problem statement)

Today the "Payroll & HR" area is **payroll only**. `Employee` is a single CRUD
record that exists so a paycheck can be computed (KRA PIN, NSSF/NHIF/SHA, HELB,
salary, allowances, bank). There is:

- **No recruitment** — no jobs, candidates, or offers.
- **No hiring/onboarding** — you manually type a payroll record; no contract,
  probation, or checklist.
- **No leave** — no leave types, balances, requests, approvals, or calendar.
- **No attendance/timesheets** — casual/contract pay can't be driven by time.
- **No employee self-service** — employees are **not users**; they can't log in,
  see a payslip, update details, or request leave. Everything is done for them.

This document specifies how to close those gaps.

## 2. Goals & non-goals

**Goals**
- Full employee lifecycle: **hire → onboard → manage → leave/attendance →
  offboard**, plus a lightweight **recruitment** front door.
- **Employee Self-Service (ESS)**: employees log in to view payslips, update
  their profile, and request leave.
- **Leave management** that feeds payroll (unpaid-leave proration, leave
  liability).
- Kenyan-labour-law-aware defaults (Employment Act 2007), configurable per tenant.
- Reuse existing primitives: multi-tenancy (`entity_id`), auth (`era_users`,
  JWT + refresh cookie), role gating, audit trail, notifications, immutability.

**Non-goals (this phase)**
- Full applicant-tracking suite (job boards syndication, CV parsing, scoring).
- Performance management, learning/LMS, benefits administration beyond payroll.
- Biometric/hardware time clocks (we expose an API; hardware is out of scope).
- Payroll maths changes beyond leave integration (the engine is already solid).

## 3. Personas & primary user journeys

| Persona | Who | Key needs |
|---|---|---|
| **HR Admin** | Owner/Admin/HR Manager | Post jobs, hire, manage records, configure leave, approve/override |
| **Line Manager (Approver)** | Department head | Approve/decline leave, view team, timesheets |
| **Employee (ESS)** | Any staff member | Payslips, profile, request leave, see balances |
| **Applicant** | External candidate | Apply to a job, track application status |
| **Payroll (Accountant)** | Finance | Run payroll with leave/attendance inputs, post to GL |

### 3.1 Journey — Recruit → Hire → Onboard (HR Admin + Applicant)
1. HR Admin creates a **Job Requisition** (title, department, type, salary band, JD).
2. Job is **published** to a public **careers page** (`/careers/{tenant-slug}`).
3. **Applicant** applies (name, email, CV upload, cover note) → **Candidate** created.
4. HR moves the candidate through a **pipeline**: `applied → screening →
   interview → offer → hired / rejected`. Each stage change is audited; the
   candidate gets email updates.
5. On **offer accepted**, HR clicks **"Hire"** → an **onboarding** record + a
   draft **Employee** are created, and (optionally) an **ESS user invite** is sent.
6. HR completes an **onboarding checklist** (contract signed, KRA PIN, bank
   details, NSSF/SHA numbers, equipment). When complete → Employee `is_active`.

### 3.2 Journey — Employee requests leave (Employee + Manager)
1. Employee opens **ESS → Leave**, sees balances per leave type.
2. Submits a **leave request** (type, dates, half-day flag, reason, attachment).
3. System validates against **balance** and **overlap**, computes working days
   (excludes weekends + tenant holidays), routes to the employee's **approver**.
4. Manager gets a notification, **approves/declines** (with note).
5. On approve: balance is decremented; the days land on the **leave calendar**;
   if the type is **unpaid**, the next payroll run prorates salary.

### 3.3 Journey — Run payroll with leave/attendance (Accountant)
1. Accountant starts a **payroll run** for a period.
2. Engine pulls **approved unpaid-leave days** and **timesheet hours** (casual/
   contract) for that period and adjusts gross accordingly.
3. Run → approve → post to GL → mark paid (existing flow). **Payslips** become
   visible to employees in ESS.

### 3.4 Journey — Offboarding (HR Admin)
1. HR initiates **termination/resignation** (last day, reason, notice).
2. Checklist: final dues, leave encashment (accrued balance × daily rate),
   equipment return, ESS access revoked, `end_date` set, `is_active=false`.
3. Final payslip includes leave encashment / deductions.

## 4. Roles & permissions

Extend the existing role set (`Owner, Admin, Accountant, Approver, Editor,
Viewer`) with HR-specific capabilities. Backend `require_role` remains the source
of truth; `roles.ts` mirrors for UX.

| Capability | Owner | Admin | HR Manager* | Approver | Accountant | Employee** |
|---|---|---|---|---|---|---|
| Configure leave types/holidays | ✓ | ✓ | ✓ | | | |
| Manage jobs/candidates | ✓ | ✓ | ✓ | | | |
| Hire / onboard / offboard | ✓ | ✓ | ✓ | | | |
| Edit any employee record | ✓ | ✓ | ✓ | | | |
| Approve leave (their team) | ✓ | ✓ | ✓ | ✓ | | |
| Run/post payroll | ✓ | ✓ | | | ✓ | |
| View own payslip/profile/leave | | | | | | ✓ |
| Request leave / edit own profile | | | | | | ✓ |

- `*` **New role: `HrManager`** — full HR without finance/GL access.
- `**` **New role: `Employee`** — ESS-scoped; can only see/act on **their own**
  records. This is the key new principle: **row-level scoping to `self`**.

**New authorization primitive:** an `Employee` user's requests must be filtered
to `employee.user_id = ctx.user_id`. Add a `ROLES_SELF_SERVICE` guard and a
per-endpoint "own-record" check (never trust the client).

## 5. Data model

New tables (all carry `entity_id` for tenancy; timestamps + audit as per
existing convention; enums stored PascalCase to match the codebase).

### 5.1 Link employees to logins (ESS foundation)
```
ALTER TABLE employees ADD COLUMN user_id UUID NULL REFERENCES era_users(id);
ALTER TABLE employees ADD COLUMN personal_email TEXT NULL;
ALTER TABLE employees ADD COLUMN manager_id UUID NULL REFERENCES employees(id);
ALTER TABLE employees ADD COLUMN department TEXT NULL;
ALTER TABLE employees ADD COLUMN job_title TEXT NULL;
```
`user_id` links an employee to their ESS login. `manager_id` drives leave-approval
routing. (Nullable — existing payroll-only employees keep working.)

### 5.2 Recruitment
```
job_requisitions(id, entity_id, title, department, employment_type,
  salary_band_min, salary_band_max, description, location, status
  [Draft|Published|Closed], published_at, created_by, created_at)

candidates(id, entity_id, job_id, full_name, email, phone, cv_url,
  cover_note, source, stage [Applied|Screening|Interview|Offer|Hired|Rejected],
  rating, notes, created_at, updated_at)

candidate_events(id, entity_id, candidate_id, from_stage, to_stage, note,
  actor_id, at)   -- audit of pipeline moves
```

### 5.3 Onboarding / offboarding
```
onboarding_cases(id, entity_id, employee_id, candidate_id NULL, type
  [Onboarding|Offboarding], status [InProgress|Complete|Cancelled],
  start_date, target_date, created_by, created_at)

onboarding_tasks(id, entity_id, case_id, title, assignee_id NULL,
  is_done, done_at, sort_order)
```
Default checklist templates seeded per tenant (contract, KRA PIN, bank, NSSF/SHA,
equipment, ESS invite) — configurable.

### 5.4 Leave
```
leave_types(id, entity_id, name, code, paid [bool], accrual_method
  [FixedAnnual|MonthlyAccrual|Unlimited], days_per_year, carryover_max,
  requires_attachment [bool], is_statutory [bool], active)

leave_balances(id, entity_id, employee_id, leave_type_id, year,
  entitled_days, accrued_days, taken_days, pending_days, carried_over)
  -- UNIQUE(employee_id, leave_type_id, year)

leave_requests(id, entity_id, employee_id, leave_type_id, start_date,
  end_date, half_day_start [bool], half_day_end [bool], working_days,
  reason, attachment_url, status [Pending|Approved|Declined|Cancelled],
  approver_id, decided_at, decision_note, created_at)

holidays(id, entity_id, date, name, recurring [bool])  -- public/company holidays
```
**Working-days computation** excludes weekends and `holidays` rows; half-day flags
subtract 0.5. Balances update transactionally on approve/cancel.

### 5.5 Attendance (phase 3, optional)
```
timesheets(id, entity_id, employee_id, period_start, period_end, status
  [Draft|Submitted|Approved], total_hours, approver_id)
timesheet_entries(id, timesheet_id, work_date, hours, project_ref NULL, note)
```
Feeds payroll for `Casual`/`Contract` employees (hours × rate).

### 5.6 Payslip access
No new table needed — payroll runs already produce per-employee lines. Add a
**read view** scoped to `employee.user_id` so ESS can fetch **only its own**
posted payslips (PDF export reuses `exportHelpers`).

## 6. Kenyan compliance (configurable defaults)

Seed these as editable `leave_types` per tenant. **These reflect commonly-cited
Employment Act 2007 provisions and must be verified against current law / counsel
before relying on them — the system treats them as configurable defaults, not
legal advice.**

| Leave type | Default entitlement | Notes |
|---|---|---|
| Annual | 21 working days/yr | after 12 months service; accrues monthly |
| Sick | 7 days full + 7 days half pay | after 2 months service |
| Maternity | 90 days (paid) | statutory; job protected |
| Paternity | 14 days (paid) | statutory |
| Compassionate | configurable | often drawn from annual |
| Unpaid | n/a | prorates payroll |

Payroll integration: **unpaid** and **half-pay** leave reduce gross for the
period; **leave encashment** on exit = accrued annual × (basic/26 or per policy).

## 7. API surface (new endpoints, under `/api/v1`)

**Recruitment**
- `GET|POST /jobs`, `GET|PUT /jobs/{id}`, `POST /jobs/{id}/publish|close`
- `GET /careers/{tenant}` (public), `POST /careers/{tenant}/apply` (public, rate-limited)
- `GET|POST /candidates`, `PUT /candidates/{id}`, `POST /candidates/{id}/advance`
- `POST /candidates/{id}/hire` → creates onboarding case + draft employee

**Onboarding/Offboarding**
- `GET|POST /onboarding`, `GET /onboarding/{id}`, `POST /onboarding/{id}/tasks/{t}/done`
- `POST /employees/{id}/offboard`

**Employees (extend)**
- existing CRUD + `POST /employees/{id}/invite-ess` (link/create `era_users` account)

**Leave**
- `GET|POST /leave-types`, `PUT /leave-types/{id}`
- `GET /leave-balances?employee_id=` (self or admin)
- `GET|POST /leave-requests`, `POST /leave-requests/{id}/approve|decline|cancel`
- `GET /leave-calendar?from=&to=` · `GET|POST /holidays`

**ESS (self-scoped)**
- `GET /me/employee`, `PUT /me/employee` (allowed fields only)
- `GET /me/payslips`, `GET /me/payslips/{run_id}`
- `GET /me/leave-balances`, `POST /me/leave-requests`

**Attendance (phase 3)**
- `GET|POST /timesheets`, `POST /timesheets/{id}/submit|approve`

## 8. UI, navigation & routing

Regroup the sidebar so HR is real. Proposed **PAYROLL & HR** group:
- **People** (`/employees` — enhanced: department, manager, status, ESS state)
- **Recruitment** (`/recruitment` — jobs + candidate pipeline board)
- **Onboarding** (`/onboarding`)
- **Leave** (`/leave` — requests, balances, calendar, approvals)
- **Attendance** (`/timesheets`) *(phase 3)*
- **Payroll** (`/payroll` — existing)
- **HR Settings** (leave types, holidays, checklist templates) — under Settings hub

**ESS shell:** when the logged-in user's role is `Employee`, the app renders a
**reduced sidebar** (My Payslips, My Leave, My Profile) — no finance/admin nav.
Reuse the existing layout; gate `navigation` by role.

**Public careers page:** unauthenticated route `/careers/{tenant-slug}` +
apply form (separate minimal layout; no ERP shell).

## 9. Cross-cutting concerns

- **Multi-tenancy:** every table `entity_id`-scoped; every query filtered by
  `ctx.entity_id`. Public careers routes resolve tenant by slug and are
  read/write-limited to job applications only.
- **Security:** ESS is the first **row-level self-scoped** area — enforce
  `employee.user_id = ctx.user_id` server-side on every `/me/*` endpoint. CV/
  attachment uploads: validate type/size, store like existing source docs, scan
  filenames. Public apply endpoint: rate-limit + captcha to prevent spam.
- **Audit:** pipeline moves, hires, leave decisions, offboarding all emit audit
  entries (reuse existing audit trail).
- **Notifications:** reuse the notification worker for leave-request/decision,
  candidate stage changes, onboarding task assignments (email/in-app).
- **Immutability & periods:** payslips tie to posted payroll (already immutable);
  leave affecting a posted period follows the same closed-period rules.
- **Migrations:** additive, auto-applied on API startup; nullable columns keep
  existing payroll-only employees valid.

## 10. Phased delivery plan

Each phase is independently shippable via the existing CI/CD (PR → main → deploy).

**Phase 1 — ESS foundation + Leave (highest value)**
- `employees.user_id/manager_id/department/job_title`; `POST /employees/{id}/invite-ess`.
- New `Employee` + `HrManager` roles; self-scoped guard; reduced ESS sidebar.
- Leave: types, balances, requests, approvals, holidays, calendar; payroll
  unpaid-leave proration.
- ESS: My Profile, My Payslips, My Leave.
- **Outcome:** employees log in, see payslips, request leave; managers approve.

**Phase 2 — Hiring & Onboarding**
- Onboarding cases + checklist templates; `hire` flow that provisions employee +
  ESS invite; offboarding with leave encashment.

**Phase 3 — Recruitment (ATS) + Attendance**
- Job requisitions, public careers page, candidate pipeline board.
- Timesheets feeding casual/contract payroll.

**Phase 4 — Polish**
- Leave accrual automation in the scheduler, leave liability report, org chart
  from `manager_id`, HR dashboard widgets.

## 11. Testing & acceptance

- Core unit tests: working-days/holiday math, balance transitions, accrual,
  unpaid-leave proration, self-scope enforcement (an `Employee` cannot read
  another's payslip/leave).
- UI: `tsc` clean; Playwright journeys — request→approve leave; apply→hire;
  ESS payslip download.
- Golden-value tests for any payroll figure changes (leave proration/encashment).

## 12. Open decisions (need product input)

1. **Leave accrual model** — fixed annual vs monthly accrual as the default?
2. **Approver routing** — always `manager_id`, or a configurable approval chain?
3. **ESS scope at launch** — payslip + leave only, or also personal-detail edits
   (which may need HR approval before they hit payroll master data)?
4. **Careers page** — needed for launch, or defer ATS to phase 3 as proposed?
5. **Sidebar** — keep HR pages in the main sidebar, or move config (leave types,
   holidays, checklists) into the Settings hub (ties into the earlier nav
   phase-2 idea)?
6. **Casual/contract pay** — is attendance/timesheet-driven pay actually needed,
   or are all staff salaried today?

---

### Appendix A — mapping to existing system
- Auth/roles: `zavora-erp-core/src/auth`, `zavora-erp-api/src/routes/users.rs`,
  `zavora-erp-ui/src/utils/roles.ts`.
- Employee/payroll: `zavora-erp-core/src/parties/employee.rs`,
  `.../services/payroll.rs`, `zavora-erp-api/src/routes/payroll.rs`.
- Patterns to reuse: entity-scoping, `require_role`, audit trail, notification
  worker, PDF export (`exportHelpers`), source-doc attachment, period immutability.
