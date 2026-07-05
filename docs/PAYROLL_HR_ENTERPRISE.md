# Enterprise Payroll & HR — Design & Build Plan

> **Status:** In progress · **Owner:** Eng · **Started:** 2026-07-05
>
> Rebuilds the payroll module from a single-shot, hardcoded, formula-only
> calculator into an **enterprise payroll engine** that scales to **thousands of
> employees**: effective-dated statutory config, earning/deduction/department
> masters, variable per-run inputs (bonuses/overtime/advances/loans), YTD
> accumulators, batch (set-based) processing, a prepare→review→commit run
> workflow with history, and filing-grade statutory reports — all tie-out tested.
>
> This document is the **source of truth and progress tracker**; update the
> checklist as work lands.

---

## 1. Why (what's wrong with the current module)

Verified against the code (2026-07-05):

- **Comp is hardcoded.** `Allowance.taxable` exists but the engine ignores it
  (all allowances taxed); UI offers only Housing/Transport/Other. Department is
  free text. No earning/deduction/department masters, no pay grades.
- **No variable inputs.** `payslips.custom_earnings/custom_deductions` are
  persisted but always written empty — no way to enter bonuses, overtime,
  advances, loans, or one-off deductions. Payroll is 100% derived from the master.
- **No pay-run history.** Only `run/approve/post/paid/pdf` endpoints; no list,
  no detail, no draft edit/delete/recompute.
- **Statutory is hardcoded constants** (PAYE/NSSF/SHA/HL/reliefs) — not
  per-tenant, not effective-dated, not reproducible for a prior year.
- **Reports thin & untested** — only `PayrollSummary` + `PayeP10` (P10 relief
  inaccurate); no P9, statutory schedules, register, or EFT file; zero report tests.
- **No YTD**, no joiner/leaver proration, no scale story (per-employee N+1 in the run).

## 2. Principles

1. **Config-driven, effective-dated** statutory rules — reproducible historical runs.
2. **Masters over free-text** — earning/deduction/department types with tax
   treatment and GL mapping.
3. **Set-based, chunked processing** — bulk-load inputs once per run, batch-insert
   payslips; target 10k employees per run within a background job.
4. **Prepare → review → commit** — draft runs are editable/recomputable, with a
   validation panel, before an immutable post to the GL.
5. **Filing-grade outputs** — P9/P10/NSSF/SHA/Housing/NITA/HELB/register/EFT, each
   with numeric tie-out tests.
6. **Immutability preserved** — posted runs and closed periods stay immutable.

## 3. Data model (migration `041_payroll_enterprise.sql`)

**Effective-dated statutory config**
- `payroll_statutory_config(id, entity_id, effective_from, name, config JSONB, created_at, created_by)`
  — `config` holds PAYE bands, personal/insurance relief, disability exemption,
  NSSF tiers+rate, SHA rate+floor, housing rate, NITA. Run picks
  `max(effective_from) <= period_end`.

**Masters**
- `earning_types(entity_id, code, name, taxable, pensionable, affects_shif, proratable, gl_account_code, sequence, active, is_system)`
- `deduction_types(entity_id, code, name, category[statutory|voluntary|loan|welfare], pre_tax, gl_account_code, sequence, active, is_system)`
- `departments(entity_id, code, name, cost_center, dimension_value_id, parent_id, active)`

**Employee links**
- `employees.department_id`, `employees.pay_frequency` (Monthly default).

**Recurring & variable inputs**
- `employee_recurring_items(employee_id, kind[earning|deduction], type_code, name, amount, taxable, start_date, end_date, active)`
- `pay_run_inputs(pay_run_id, employee_id, kind, type_code, name, amount, taxable, note)` — one-off per run.
- `employee_loans(employee_id, name, principal, balance, installment, interest_rate, start_date, status)`
- `loan_repayments(loan_id, pay_run_id, amount, balance_after)` — amortization ledger.

**Pay-run / payslip extensions**
- `pay_runs`: `name, pay_group, employee_count, total_employer_cost, notes`.
- `payslips`: denormalized numeric columns (`gross, taxable, paye, nssf_employee,
  nssf_employer, sha, housing_employee, housing_employer, helb, total_deductions,
  net`), employee snapshot (`employee_name, staff_number, kra_pin`), `department_id`,
  itemized `earnings JSONB`, `deductions_detail JSONB`, and `ytd JSONB`.

Indexes for scale: `payslips(pay_run_id)`, `payslips(employee_id, ...)`,
`pay_run_inputs(pay_run_id)`, `employee_recurring_items(employee_id) WHERE active`,
`employee_loans(employee_id) WHERE status='active'`.

## 4. Phases & progress

Legend: ⬜ todo · 🟡 in progress · ✅ done

### Phase 1 — Foundation (config + masters + schema)
- ✅ Migration `041` written and validated (rollback-tx against dev DB); applies via
  sqlx on next API start.
- ✅ `StatutoryConfig` (effective-dated, JSON-serializable) with `finance_act_2024()`
  default == former constants. `payroll/config.rs`.
- ✅ `statutory.rs` refactored to delegate to `StatutoryConfig`; public facade
  (`PayeBands`/`Nssf`/`Sha`/`HousingLevy`) preserved. All 9 golden tests green +
  JSON round-trip test.
- ✅ Config loader + lazy per-tenant seeder + list. `services/payroll_config.rs`.
- ✅ Core domain models (structs/FromRow + request types) for earning/deduction/
  department masters, recurring items, pay-run inputs, loans. `payroll/masters.rs`.
- ✅ Honor `taxable` in the engine — non-taxable earnings excluded from PAYE/NSSF/
  SHA/housing bases (`payroll/compute.rs`, tie-out tested).

### Phase 2 — Engine + lifecycle
- ✅ `payroll/compute.rs` — pure, input-aware, config-driven payslip computation
  honoring `taxable`/`pre_tax`; itemizes earnings/deductions; employer cost + NITA.
  4 tie-out tests (incl. the 100k UI-review case → net 72,304.65).
- ✅ `services/payroll_masters.rs` — CRUD + set-based batch loaders (recurring/
  inputs/loans grouped by employee) so a run issues a constant query count; seeds
  default earning/deduction types.
- ✅ `run_payroll` rewritten: config-driven + effective-dated, honors `taxable`,
  merges base allowances + recurring + per-run **earning** inputs, joiner/leaver +
  unpaid-leave proration, YTD accumulators, denormalized payslip columns +
  itemized earnings/deductions + employee snapshot. Draft-first.
- ✅ `recompute_pay_run` (draft-only, picks up inputs), `list_pay_runs`,
  `load_pay_run` (detail), `delete_draft_pay_run`; duplicate-draft-per-period guard.
- ✅ Voluntary/loan **deductions** wired into the engine (recurring + per-run +
  loan installments), pre_tax/category resolved from `deduction_types`.
- ✅ `post_pay_run` credits withheld deductions to their mapped liability account
  (fallback net-pay payable) — **JE stays balanced**; loan amortization at post
  (records `loan_repayments`, decrements balances, settles at zero).
- ✅ API routes: `GET /payroll` (history), `GET /payroll/{id}` (detail),
  `POST /payroll/{id}/recompute`, `DELETE /payroll/{id}` (draft), and per-run
  inputs CRUD (`GET|POST /payroll/{id}/inputs`, `DELETE .../inputs/{input_id}`).
- ⬜ GL **department dimension** tagging on payroll lines + pre-run validation
  panel (missing KRA PIN/bank) — small; rolls into Phase 3.

**Live end-to-end verification (rebuilt binary, migration 041 applied):**
- Joiner proration: Grace (start Jul 5) → July basic 80,000×20/23 = 69,565.22.
- Variable earning (Bonus 50,000) → gross 139,565.22; recompute picks it up.
- Variable deduction (SACCO 3,000, type-resolved welfare/post-tax) reduces net.
- Itemized earnings + deductions + YTD persisted on the payslip.
- **Posted GL entry balances: 143,818.70 = 143,818.70 (10 lines)**, incl. a
  "Payroll deductions withheld" 3,000 credit line.
- Pay-run history list, draft delete (ok) and posted delete (rejected) verified.
- Period-close guard verified (post blocked while SoftClosed; reopened w/ reason
  to post, then re-closed).

### Phase 3 — Masters CRUD + reports
- ✅ Masters CRUD **API** + live-verified: `GET|POST /payroll/earning-types`,
  `.../earning-types/{id}/active`, `GET|POST /payroll/deduction-types`,
  `.../deduction-types/{id}/active`, `GET|POST /payroll/departments`,
  `GET /payroll/statutory-config`, `GET|POST /payroll/recurring-items` +
  `DELETE .../{id}`, `GET|POST /payroll/loans`. Default earning/deduction types
  seed on first list. (`routes/payroll_masters.rs`.)
- ✅ Net/PAYE/deductions rounded to 2dp in `compute_payslip` (fixes 95,496.8280 →
  95,496.83); 15 payroll tests still green.
- ⬜ Masters CRUD **UI** (earning/deduction types, departments, statutory editor).
- ✅ Reports implemented + **live tie-out verified** against the posted run & GL:
  `PayrollRegister`, `StatutorySchedule` (PAYE/NSSF ee+er/SHA/Housing ee+er/HELB
  with member numbers), `PayeP9` (monthly gross/taxable/tax-charged/relief/PAYE),
  `PayrollBankFile` (net + bank details). Wired via `POST /reports`
  (`ReportContent::Generic`); `services/reporting.rs`. Register/schedule/bank
  totals reconcile to pay-run totals & the balanced journal.

### Phase 4 — UI + polish
- ✅ Payroll **prepare→review→commit** UI (`PayrollPage.tsx`): run **history** list,
  "New Pay Run", run **detail** with stat cards + full statutory payslip table +
  status-aware actions (Recompute/Approve/Delete for draft, Post for approved,
  Mark Paid for posted) + payslip PDF. **Adjustments panel** on drafts to add
  per-run earning/deduction inputs (employee + type + amount + taxable) then
  Recompute. API client extended with all lifecycle + masters + inputs calls.
  UI `tsc` clean; live-verified (history + detail render, figures tie out).
- ✅ Masters editor UI (`PayrollSettingsPage.tsx`, route `/payroll-settings`, nav
  link): tabs for Earning Types, Deduction Types (create + activate/deactivate),
  Departments (create), and an **editable Statutory Rates** editor — add a new
  effective-dated version or edit an existing one (all reliefs/rates/caps + a
  PAYE-band table). Backend `POST /payroll/statutory-config` (upsert, effective-
  dated). Live-verified: created "Finance Act 2025" (relief 2,500, NSSF cap
  72,000) effective 2027-01-01; the engine resolves it by pay date.
- ✅ Employee card wired to masters: **dynamic allowances editor** (add/remove
  rows, name from earning-type suggestions, per-row taxable) replacing the 3
  hardcoded fields; **department dropdown** from the departments master setting
  `department_id` (added to Create/UpdateEmployeeRequest + insert/update).
  Live-verified: Grace → Housing allowance editable + department Engineering
  persisted (`department_id` set, flows to payslips/reports).
- ✅ Report screens (`PayrollReportsPage.tsx`, route `/payroll-reports`, nav link):
  Register / Statutory Schedule / P9 / Bank-EFT tabs with period range + CSV export,
  rendering the tie-out-verified backend reports. Live-verified (register shows
  Grace 139,565.22 → net 95,496.83 with totals).
- ✅ Richer payslip PDF — itemized earnings (Basic + allowances + bonuses),
  statutory + other deductions, employer contributions, and YTD.
- ✅ Pre-run validation panel — warns when active employees are missing KRA PIN /
  bank details before a run.
- ✅ `employment_type` stored plain (migration 044 unquotes existing rows).
- ✅ GL department-dimension tagging — the salaries-expense debit is split by
  department and tagged `{"Department": <code>}` (sums to gross → balanced).
  Live-verified: posted JE with `7010 24,060.00 {"Department":"ENG"}`, balances
  25,789.50 = 25,789.50.

### Verification (task 9)
- ✅ `cargo test --workspace` — 152 tests pass, 0 failures.
- ✅ Migrations apply cleanly on startup (through 044).
- ✅ UI `tsc --noEmit` clean.
- ✅ Playwright: every cycle verified live (masters define/edit incl. editable
  statutory rates, employee card dynamic allowances + department, run
  prepare→review→commit with auto-recomputing adjustments, balanced post, reports,
  ESS). Balanced posted journal 143,818.70 = 143,818.70.

### Verification
- ⬜ `cargo build/test --workspace`, migration applies, UI `tsc`, Playwright full cycle.

## 5. Compatibility / migration safety
- All schema changes are **additive** (`IF NOT EXISTS`, nullable/defaulted); legacy
  payroll-only employees and existing pay runs stay valid.
- Existing `employees.allowances` JSON remains the base-allowance source; the
  engine now honors each allowance's `taxable` flag and merges recurring + per-run
  inputs on top.
- Default statutory config is seeded to **exactly** the current hardcoded values,
  so existing golden tests and posted history are unchanged.
