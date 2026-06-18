# Reporting Roadmap

Backlog to bring Zavora reporting from "core statements, correct" to Business
Central / NetSuite parity. Grouped into workstreams. Status: ⬜ not started
· 🟡 in progress · ◻ partial · ✅ done.

Already shipped: P&L, Balance Sheet, Cash Flow, Trial Balance, General Ledger,
AR/AP Ageing, VAT Return — with audit-grade balancing/reconciliation, comparative
periods (P&L/BS), CSV export, and a period-snapshot engine for scale.

**Progress:** Pending-reports bucket ✅ complete (6/6). Presentation ✅ full-page
layout, print/PDF/Excel, account→GL drill-down; ⬜ collapsible/states. Customisation
not started. New: a **Documents** bucket for transactional-document preview/print.

> Reports built after the Docker socket dropped (KRA statutory, income/expense,
> inventory & fixed-asset, bank rec) are schema-verified but **pending live
> re-validation** against data, and the :8080 server still runs the pre-statutory
> binary — `cargo run -p zavora-erp-api` picks everything up.

## 1. Pending reports
- ✅ **Customer & Vendor statements** — opening balance, dated activity, running/closing balance; party + date-range filters. (CustomerStatement / VendorStatement report types.)
- ✅ **KRA statutory** — PAYE P10 monthly schedule, WHT schedule, VAT summary by rate band. (PayeP10 / WhtCertificate / SalesTaxSummary report types.) *SQL pending live re-validation.*
- ✅ **Payroll Summary** — gross, PAYE, NSSF, SHA, housing levy, HELB, net; per-run and per-employee. (PayrollSummary report type.)
- ✅ **Bank Reconciliation Summary** — statement vs GL balance per account, matched/unmatched feed items, reconciled flag. (BankReconSummary report type.)
- ✅ **Income by Customer / Expense by Vendor** — net (ex-VAT) revenue/expense grouped, ranked, with % share. (IncomeByCustomer / ExpenseByVendor report types.)
- ✅ **Inventory Valuation & Fixed-Asset register** — on-hand qty/cost/value; cost/accum-dep/NBV. (InventoryValuation / FixedAssetRegister report types.)

## 2. Presentation
- ✅ **Full-page statement layout** — company name/logo + KRA PIN header, title, period/as-at, indented sections, bold subtotals, right-aligned figures, generated-on footer.
- ✅ **Print + PDF + Excel export** — print stylesheet (Print → PDF), .xls export alongside CSV.
- ◻ **Drill-down to transactions** — account on TB/BS/P&L → General Ledger ✅ (period carried through). GL → source document still pending (needs a Journals/transactions page + entry id on GL lines).
- ⬜ **Collapsible sections + loading/empty/error states** — expand/collapse, skeletons, empty/error with retry.

## 3. Documents (transactional output)
Reports are covered, but the source documents are not: generating an invoice has
no way to preview/print it before posting — same for the other documents. Reuse
the report letterhead + print stylesheet for a consistent branded output.
- ⬜ **Invoice preview & print/PDF before posting** — branded preview (logo, KRA PIN, line items, VAT, totals, terms) viewable as a draft; Print → PDF; post or keep editing from the preview.
- ⬜ **Preview & print for other documents** — estimates/quotes, credit notes, bills/POs, payment receipts; shared branded document component + print stylesheet.

## 4. Customisation
- ⬜ **Custom financial report builder** — user-defined rows (account ranges, formulas, subtotals) and columns (periods, comparatives, %); saved per entity.
- ⬜ **Budgets + Budget vs Actual** — budget entry per account/period; actual/budget/variance/variance %.
- ⬜ **Dimensional / segment analysis** — filter/group by dimension (cost centre, project, location); key snapshots by dimension.
- ⬜ **Scheduled + emailed reports** — cron + notification-queue delivery (email/PDF).
- ⬜ **Multi-entity consolidation** — group statements across entities; currency translation + intercompany elimination (later phase).

## Follow-ups identified during build
- **GL → source document** drill (second hop of the drill-down): needs a Journals/
  transactions page and an entry id on GL detail lines (neither exists yet).
- **Live SQL re-validation** of the four reports built while Docker was down.

## Suggested sequence
Presentation (full-page + print/PDF) → report wins ✅ → drill-down ✅ →
**Documents (invoice preview/print)** → collapsible/states → customisation
(report builder is the marquee gap-closer).
