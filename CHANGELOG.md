# Changelog

All notable changes to Zavora ERP are recorded here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/). The project is not yet
versioned/tagged; entries are grouped by the date the work landed on `main`.

For what is **not** yet built, see [`REMAINING.md`](REMAINING.md).

## [Unreleased]

### 2026-06-27 — End-to-end audit: idempotency, atomicity & multi-tenancy

Backend and UI fixes from a full end-to-end accounting audit, plus a complete
QuickBooks → Zavora rebuild used as the correctness oracle.

#### Added
- **QuickBooks rebuild harness** (`scripts/qbo/`): extract a QBO company, set up
  a matching Zavora tenant + chart + masters, replay transactions through the
  real AR/AP/banking flows, and compare Zavora reports against the QBO reports.
  Result: **P&L matches QuickBooks to the cent**; Balance Sheet reconciled (tax
  neutralised). See `sample_data/quickbooks/comparison_report.md`.
- **Bank statement import** is now wired end-to-end (CSV / MT940 / OFX) and
  **idempotent** — re-importing the same file is rejected (file `content_hash`),
  and duplicate lines are skipped (`dedup_key`). Migration 027.
- **Year-end close** is now functional, atomic and idempotent: closing + opening
  entries post in one transaction; `YearEndClose`/`OpeningBalance` sources are
  allowed into hard-closed periods (migration 026); `POST /periods/year-end-close`
  wired. UI gains a "Close Year" action (shown once all of a year's periods are
  hard-closed).
- **Asset depreciation** rewritten as an idempotent **catch-up** run
  (`depreciated_through`, migration 025): a run books every missed month up to the
  target and cannot double-post. The scheduler now books prior months **for all
  tenants** automatically.
- **AR/AP Ageing** reports render as proper tables (were raw JSON).
- **Audit Trail** is now audit-grade: who / what / when / before→after, with
  actor names and emails resolved server-side.

#### Fixed
- **Multi-tenancy**: report schedules, recurring invoices/journals and reminders
  now iterate **all tenants** (were bound to the startup entity). Recurring
  journals advance `next_run` in the same transaction as the post; recurring
  invoices use the scheduled date, not "today".
- **Posting**: `unapplied_payments` now defaults to a seeded account (`9100`)
  instead of a non-existent `3050` that broke the trial balance.
- **Auth**: login resolves the user globally instead of scoping to the served
  entity (new tenants could not sign in).
- **Journal validation** uses the same sub-cent rounding tolerance as the poster.
- **Invoicing**: discount lines post as a positive debit to revenue (no negative
  journal lines).
- **Payments**: the M-Pesa callback recovers orphaned receipt claims instead of
  rejecting them forever.
- **Reports UI**: Balance Sheet "Total Liabilities + Equity" no longer shows
  `NaN`; report drill-down no longer hangs on "Generating report…".
- **Assets UI**: register form sent a display label instead of the category enum
  (422), and depreciation posted to phantom GL codes — now uses the seeded chart.
- **UI contracts**: Record Payment, New Journal Entry, Import Statement,
  Auto-Categorise and Close Year actions were broken or dead — payloads fixed and
  actions wired.

### 2026-06-22 — Accounting feature completion (onboarding → period-end → tax)

#### Added
- **Onboarding**: opening-balances entry; bulk CSV import for
  customers/vendors/products.
- **Period-end**: recurring / accrual / prepayment journals.
- **AR**: bad-debt write-off.
- **Tax**: VAT / PAYE / WHT filing + remittance workflow.
- **Inventory**: stock-take adjustment.
- **Banking**: formal bank reconciliation (complete & lock).
- **Reporting**: Statement of Changes in Equity + direct-method cash flow.
- **Release hardening (R3–R10)**: list-endpoint pagination; loading/error states
  + shared components; persisted recurring invoices; estimate draft edit/delete;
  vendor detail page; in-app notification inbox; new-tenant dashboard empty
  state; customer-statement send action.
- Inventory **Add Item**, `GET /payments/{id}` receipt preview, **Document
  Numbers** settings persistence, **asset depreciation** run, **FX revaluation**
  run (engines existed; routes were stubs — now wired).
- Supplier credit notes from posted bills (mirrors invoice credit notes).

### 2026-06-17 → 06-19 — Reporting & document output

#### Added
- **Reports**: typed Reports UI with balancing badges, CSV/Print-PDF/Excel
  export and comparative periods; full-page branded statement layout.
- Full statement set: Trial Balance, Balance Sheet, P&L, Cash Flow, General
  Ledger, AR/AP Ageing, VAT Return; customer & vendor statements; KRA statutory
  (PAYE P10, WHT schedule, VAT by rate); payroll summary; bank-reconciliation
  summary; income-by-customer / expense-by-vendor; inventory valuation &
  fixed-asset register.
- **Componentized** the reports monolith into per-report pages (Phase 1) and
  added **branded preview & print** for invoices, estimates, credit notes, bills
  and receipts (Phase 2).
- **Drill-down**: statement → GL → source document (migration 015).
- **Dimensions** (analytical accounting): masters, capture on journal & invoice
  lines, dimensional-analysis report (migrations 017/020).
- **Customisation**: budgets + Budget-vs-Actual (016); custom report builder
  (018); scheduled + emailed reports (019); multi-entity consolidated trial
  balance.
- **Performance**: per-account period-balance snapshots for O(periods) as-at;
  denormalised `entity_id`/`date` onto journal lines; composite indexes.

### Earlier — Foundation & auth hardening

#### Added
- Estimates, supplier credit notes, eTIMS status, journal-entry reversal,
  period-management UI; role-aware action gating.
- **Auth hardening**: JWT + Argon2id password hashing; global middleware gates
  every protected route; role checks on all master-data writes; access token in
  memory only, refresh token in an httpOnly SameSite=Strict cookie;
  `/auth/logout` revokes + clears.
- **Posting setup**: per-tenant GL resolver + editable Settings → Posting
  Accounts UI with live reload (see `docs/POSTING_SETUP.md`).
- M-Pesa webhook idempotency; atomic draft creation for invoices/estimates.
