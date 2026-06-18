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
- ✅ KRA statutory — PAYE P10, WHT schedule, VAT summary by rate.
- ✅ Payroll Summary; Bank Reconciliation Summary; Income-by-Customer /
  Expense-by-Vendor; Inventory Valuation & Fixed-Asset register.
- ✅ Full-page branded layout + print/PDF/Excel export.
- ✅ Drill GL line → source document (journal_entries.source_id, migration 015).

All report SQL re-validated live (executes clean; VAT-by-rate and
Income-by-Customer return correct figures on real data). Numeric tie-out for
bills/payroll/inventory/bank awaits seed data in those modules (Phase 4).

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

## Phase 3 — Drill-down completeness  ✅

- ✅ Add entry id + source + source_id to GL detail lines.
- ✅ Wire GL line → source document (invoice/credit note/bill) via source_id —
  verified live (38/38 invoice entries link, 0 id mismatches).
- ✅ Journal entry detail page (header + balanced lines + source link); GL JE #
  links to it.
- ✅ Collapsible/expandable sections on BS/P&L (forced expanded for print).

**Exit criteria met:** no figure is a dead end — every number drills to its
source document.

---

## Phase 4 — Live re-validation  ◻

- ✅ All report SQL re-validated against the live schema (every query executes
  clean under ON_ERROR_STOP; VAT-by-rate and Income-by-Customer return correct
  figures on real invoice data). Schema-correctness risk closed.
- ⬜ Numeric tie-out for bills/payroll/inventory/bank-rec awaits seed data in
  those modules (currently empty).
- ⬜ Add tie-out integration tests for each statutory/management report.

---

## Phase 5 — Dimensions (analytical accounting)  ◻  (Option A shipped)

- ✅ **Masters**: `dimension_types` + `dimension_values` (migration 017) with a
  Dimensions management page and API.
- ✅ **Capture**: journal lines persist `{ type_code: value_code }` (already
  supported end-to-end); validated live.
- ✅ **Reporting**: Dimensional Analysis report groups movement by a chosen
  dimension type for a period (Option A — scans date-bounded lines, reads the
  JSONB key), values resolved to names.
- ⬜ **Capture on every form**: dimension selectors on invoice/bill/expense
  lines (only the journal-entry path is wired so far) — larger UI effort.
- ⬜ **Controls**: per-account rules (e.g. expense accounts require a Cost
  Centre).
- ⬜ **Option B**: extend the snapshot key to `account + dimension + period`
  (only if dimensional volume warrants it).

---

## Phase 6 — Customisation & advanced  ✅

- ✅ **Budgets + Budget vs Actual** — budget per account/period (migration 016);
  actual/budget/variance/variance % report. Budgets page.
- ✅ **Custom report builder** — saved row-based definitions (header / account
  range / subtotal) computed over a period (migration 018), branded printable
  output. `/reports/custom`.
- ✅ **Scheduled + emailed reports** — schedules (migration 019) run on the
  hourly scheduler tick, queued to recipients via the notification outbox.
  Actual SMTP send-out is the notification worker's job (production-readiness).
- ✅ **Multi-entity consolidation** — consolidated trial balance across the
  entities the user is a member of (safe by construction); FX translation +
  intercompany elimination deferred (mixed-currency flagged).

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
