# Reporting & Document Output — Plan

Brings Zavora reporting from "core statements, correct" to Business Central /
NetSuite parity, and closes the transactional-document output gap. The bar is
**auditable, accountant-grade, and KRA-regulatory-compliant** throughout.

Status legend: ⬜ not started · 🟡 in progress · ◻ partial · ✅ done.

---

## Quality bar (applies to every item below)

A report or document is only "done" when it meets all of:

1. **Ties out** — statements balance; derived figures reconcile to the ledger
   (debits = credits; BS balances; cash-flow closing = GL cash). Enforced by
   integration tests, not eyeballing.
2. **Traceable** — every figure drills down: statement → account → general
   ledger → journal entry → source document. No dead ends.
3. **Branded & statutory** — company name, logo, and **KRA PIN** on every
   statement/document; period or as-at date; generated-on footer; the document
   number and (for tax invoices) the eTIMS control number.
4. **Immutable source** — reports read an append-only ledger; the period-snapshot
   cache is maintained in-transaction and reconciles to raw lines.
5. **Period-aware** — respects fiscal-period locks; as-at honours closed periods.
6. **Exportable** — Print → PDF, CSV, and Excel, with a print stylesheet.
7. **Resilient UI** — explicit loading, empty, and error (with retry) states.

---

## Already shipped ✅

Core statements with audit-grade balancing/reconciliation, comparative periods
(P&L/BS), CSV + Print/PDF + Excel export, account→GL drill-down (first hop), and
a per-account/per-period **snapshot engine** for O(periods) as-at performance,
maintained transactionally and covered by `snapshots_reconcile_to_ledger` and
`reports_balance_after_posting` tests.

- ✅ Trial Balance, Balance Sheet, Profit & Loss, Cash Flow, General Ledger,
  AR/AP Ageing, VAT Return.
- ✅ Customer & Vendor statements (opening/running/closing balances).
- ✅ KRA statutory — PAYE P10, WHT schedule, VAT summary by rate. *SQL pending
  live re-validation (see Phase 4).*
- ✅ Payroll Summary; Bank Reconciliation Summary; Income-by-Customer /
  Expense-by-Vendor; Inventory Valuation & Fixed-Asset register. *Last four
  pending live re-validation (Phase 4).*
- ✅ Full-page branded layout + print/PDF/Excel export.

---

## Phase 1 — Componentize + dedicated report pages  ⬜  (foundation)

The current `ReportsPage.tsx` is a ~1,100-line monolith with a dropdown selector,
plus an orphan `reportShared.tsx`. This phase is the structural fix that unblocks
documents, drill-down, and UI states.

- ⬜ Delete the orphan `reportShared.tsx`.
- ⬜ Shared building blocks under `src/pages/reports/`:
  - `ReportLayout` — branded letterhead (company, logo, KRA PIN), title,
    period/as-at, generated-on footer, print stylesheet.
  - `ReportFilters` — date range / as-at / comparative / party / dimension.
  - `useReport(reportType, params)` — fetch hook with loading/empty/error.
  - One **view component per statement**: `TrialBalanceView`, `BalanceSheetView`,
    `ProfitAndLossView`, `CashFlowView`, `GeneralLedgerView`, `ArAgeingView`,
    `ApAgeingView`, `VatReturnView`, plus statutory/management views.
- ⬜ Dedicated routes, each its own page:
  `/reports` (index/launcher), `/reports/trial-balance`, `/reports/balance-sheet`,
  `/reports/profit-and-loss`, `/reports/cash-flow`, `/reports/general-ledger`,
  `/reports/ar-ageing`, `/reports/ap-ageing`, `/reports/vat`,
  `/reports/customer-statement`, `/reports/vendor-statement`,
  `/reports/payroll-summary`, `/reports/paye-p10`, `/reports/wht`,
  `/reports/income-by-customer`, `/reports/expense-by-vendor`,
  `/reports/inventory-valuation`, `/reports/fixed-asset-register`,
  `/reports/bank-reconciliation`.
- ⬜ Per-page loading skeletons, empty states, and error + retry.
- ⬜ Reports index grouped by category (Financial · Receivables/Payables · Tax ·
  Payroll · Management).

**Exit criteria:** monolith retired; every report on its own route; `tsc -b` +
`vite build` green; no behavioural regression vs current output.

---

## Phase 2 — Transactional document output  ⬜  (priority gap)

Source documents currently cannot be previewed or printed. Reuse the Phase 1
letterhead + print stylesheet for a consistent branded `DocumentView`.

- ⬜ Shared `DocumentView` + document print stylesheet (logo, KRA PIN, party
  details, line items, VAT breakdown, totals, terms/footer).
- ⬜ **Invoice** preview & print/PDF **before posting** (draft watermark) and
  **after posting** (tax invoice; shows eTIMS control number once transmitted).
  Post or keep editing from the preview.
- ⬜ **Estimate / quote** preview & print/PDF.
- ⬜ **Credit note** (customer & supplier) preview & print/PDF, referencing the
  original document.
- ⬜ **Bill / purchase order** preview & print.
- ⬜ **Payment receipt** preview & print.

**Exit criteria:** every transactional document is previewable + printable in a
branded layout, pre- and post-posting where applicable.

---

## Phase 3 — Drill-down completeness  ◻

First hop (statement → account → GL) is done. Close the loop to source.

- ◻ Add journal-entry id to GL detail lines.
- ⬜ Journal entry detail page (header + balanced lines + source link).
- ⬜ Wire GL line → journal entry → source document (invoice/bill/payment/etc.).
- ⬜ Collapsible/expandable sections on TB/BS/P&L.

**Exit criteria:** no figure is a dead end — every number drills to its source
document.

---

## Phase 4 — Live re-validation  ⬜

The four reports built while the DB was unavailable are schema-verified only.

- ⬜ Re-validate KRA statutory (P10, WHT, VAT-by-rate), Income/Expense,
  Inventory & Fixed-Asset, and Bank-Rec against real posted data; add tie-out
  integration tests for each; fix discrepancies.

---

## Phase 5 — Dimensions (analytical accounting)  ⬜  (scope decision needed)

Today `journal_lines.dimensions` (JSONB) exists but is **always empty** — no
masters, no capture, no reporting. Build it as a first-class subsystem.

- ⬜ **Masters** (Settings): `dimension_types` (e.g. Cost Centre, Project,
  Department, Location) and `dimension_values` per type — code, name, active.
- ⬜ **Capture**: optional dimension selectors on each transaction line
  (invoice/bill/journal/expense), stored as `{ type_code: value_code }`.
  Inherit defaults from customer/vendor/account to reduce keying.
- ⬜ **Controls**: per-account rules (e.g. expense accounts require a Cost
  Centre) — enforced analytical coding.
- ⬜ **Reporting**: filter/group statements by dimension; dimensional P&L.

**Open decision — dimensional as-at performance:**
- *Option A (simpler):* dimensional queries scan raw lines (bounded by date +
  dimension filter). Lower build cost; slower on very large dimensional history.
- *Option B (fuller):* extend the snapshot key to `account + dimension + period`.
  Higher build + storage cost; keeps O(periods) for dimensional as-at too.
- **Recommendation:** start with Option A, add Option B only if dimensional
  reporting volume warrants it. **Confirm before building.**

---

## Phase 6 — Customisation & advanced  ⬜

- ⬜ **Budgets + Budget vs Actual** — budget per account/period;
  actual/budget/variance/variance %.
- ⬜ **Custom report builder** — user-defined rows (account ranges, formulas,
  subtotals) and columns (periods, comparatives, %); saved per entity.
- ⬜ **Scheduled + emailed reports** — cron + notification-queue delivery
  (depends on the notification workers — separate production-readiness item).
- ⬜ **Multi-entity consolidation** — group statements; FX translation +
  intercompany elimination. (Later phase.)

---

## Sequencing

1. **Phase 1 — Componentize + own pages** (unblocks the rest).
2. **Phase 2 — Document output** (explicit gap: invoice/estimate/bill/CN/receipt
   preview + print, before and after posting).
3. **Phase 3 — Drill-down completeness** (GL → source document).
4. **Phase 4 — Live re-validation** of the four newer reports.
5. **Phase 5 — Dimensions** (after scope/perf decision).
6. **Phase 6 — Customisation** (budgets → report builder → scheduled →
   consolidation).

Each phase ships in small, build- and test-verified increments and is committed
+ pushed before the next begins.
