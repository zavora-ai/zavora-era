---
name: cash-forecast
description: Build a 13-week rolling cash-flow forecast from the live ledger — opening cash, AR receipts by due date (haircut by payment behaviour), AP and statutory payments by deadline, payroll cycle — and answer "will we make payroll in week 9?" or "can we afford X?". Use when the user asks about future cash, runway, affordability, or a cash forecast.
license: Proprietary
compatibility: Requires mcp-erp (zavora backend).
allowed-tools: [get_dashboard, list_bank_accounts, list_invoices, list_bills, run_report, list_pay_runs, list_tax_filings, cit_estimate, list_purchase_orders, showcase_step, plan_tasks, update_task]
metadata:
  author: Zavora AI
  category: management-accounting
  success-criteria:
    grounding: "Every inflow/outflow traces to a document, a run, or a statutory deadline"
    honesty: "Assumptions (collection haircuts, recurring costs) are stated in the output"
    alerting: "Any week that goes negative (or below the buffer) is flagged first, not buried"
---

# 13-Week Cash Forecast

Forward-looking, assumption-honest. You assemble it from documents and
calendars — the ledger knows what's due; you lay it on the timeline.

## Inputs (gather all, then build)
1. **Opening cash** — `get_dashboard` + `list_bank_accounts` (exclude overdrawn FCY from "available" unless asked).
2. **Inflows** — `list_invoices` (posted, unpaid): slot each by due date. Haircut for behaviour: `run_report CustomerPaymentHistory` (or ArAgeing bucket drift) — a customer who pays 30 days late gets slotted 30 days late, and say so. Overdue 90+ = assume nothing unless the user says otherwise.
3. **Outflows** —
   - `list_bills` (posted, unpaid) by due date; `list_purchase_orders` (sent/acknowledged, not yet billed) as committed spend near their delivery dates.
   - **Payroll**: `list_pay_runs` — last net + statutory cost, recurring monthly on the usual pay date.
   - **Statutory calendar**: PAYE by the 9th, VAT & WHT by the 20th (`list_tax_filings` + the reports for the coming months' figures — estimate from the latest month and label it), CIT installments from `cit_estimate`.
   - Known recurring operating costs (rent etc.): infer from `run_report ExpenseByVendor` monthly pattern; label as assumption.
4. Build 13 weekly buckets from Monday of the current week: opening → +in → −out → closing per week.

## Output (max ~20 lines)
- ⚠ FIRST: any week closing below zero (or the user's buffer) — which week, how deep, and the single biggest cause.
- The 13-week table (week ending · in · out · closing) as a markdown table.
- Assumptions block: collection haircuts applied, recurring costs assumed, statutory estimates used.
- 2–3 levers if there's a crunch: collectable AR (name the customers), deferrable AP (name the bills and their real risk), the payment-run skill for the batch decision.

## "Can we afford X?"
Insert X in its week, rerun the closing balances, answer with the first week that breaks (or "yes, lowest point becomes KES … in week …").

## MUST NOT DO
- Never present the forecast as certain — it is a model of documents + assumptions, and the assumptions are printed.
- Never assume overdue-90+ money arrives.
- Never omit statutory outflows — KRA is the least deferrable creditor in Kenya.
