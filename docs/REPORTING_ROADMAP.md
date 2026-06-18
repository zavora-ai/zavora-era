# Reporting Roadmap

Backlog to bring Zavora reporting from "core statements, correct" to Business
Central / NetSuite parity. Grouped into three workstreams. Status: ⬜ not started
· 🟡 in progress · ✅ done.

Already shipped: P&L, Balance Sheet, Cash Flow, Trial Balance, General Ledger,
AR/AP Ageing, VAT Return — with audit-grade balancing/reconciliation, comparative
periods (P&L/BS), CSV export, and a period-snapshot engine for scale.

## 1. Pending reports
- ✅ **Customer & Vendor statements** — opening balance, dated activity, running/closing balance; party + date-range filters. (CustomerStatement / VendorStatement report types.)
- ✅ **KRA statutory** — PAYE P10 monthly schedule, WHT schedule, VAT summary by rate band. (PayeP10 / WhtCertificate / SalesTaxSummary report types.) *SQL pending live re-validation.*
- ✅ **Payroll Summary** — gross, PAYE, NSSF, SHA, housing levy, HELB, net; per-run and per-employee. (PayrollSummary report type.)
- ⬜ **Bank Reconciliation Summary** — GL vs statement balance, matched/unmatched, outstanding items.
- ⬜ **Income by Customer / Expense by Vendor** — revenue/expense grouped and ranked for a period.
- ⬜ **Inventory Valuation & Fixed-Asset register** — qty/cost/value; cost/accum-dep/NBV.

## 2. Presentation
- ✅ **Full-page statement layout** — company name/logo + KRA PIN header, title, period/as-at, indented sections, bold subtotals, right-aligned figures, generated-on footer.
- ✅ **Print + PDF + Excel export** — print stylesheet (Print → PDF), .xls export alongside CSV.
- ⬜ **Drill-down to transactions** — account on TB/BS/P&L → General Ledger → source document.
- ⬜ **Collapsible sections + loading/empty/error states** — expand/collapse, skeletons, empty/error with retry.

## 3. Customisation
- ⬜ **Custom financial report builder** — user-defined rows (account ranges, formulas, subtotals) and columns (periods, comparatives, %); saved per entity.
- ⬜ **Budgets + Budget vs Actual** — budget entry per account/period; actual/budget/variance/variance %.
- ⬜ **Dimensional / segment analysis** — filter/group by dimension (cost centre, project, location); key snapshots by dimension.
- ⬜ **Scheduled + emailed reports** — cron + notification-queue delivery (email/PDF).
- ⬜ **Multi-entity consolidation** — group statements across entities; currency translation + intercompany elimination (later phase).

## Suggested sequence
Presentation (full-page + print/PDF) → quick report wins reusing existing engines
(statements, payroll summary, statutory) → drill-down → customisation
(report builder is the marquee gap-closer).
