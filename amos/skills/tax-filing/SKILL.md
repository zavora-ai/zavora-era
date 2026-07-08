---
name: tax-filing
description: Kenyan statutory tax workflow in Zavora ERA — prepare VAT/PAYE/WHT figures from the ledger, record returns as filed, and record the remittance to KRA. Knows the KRA calendar (VAT & PAYE due the 20th of the following month, WHT the 20th). Use when the user asks about VAT/PAYE/WHT due, preparing or filing a return, or paying KRA.
license: Proprietary
compatibility: Requires mcp-erp (zavora backend) and the Playwright browser tools.
allowed-tools: [run_report, list_tax_filings, file_tax_return, remit_tax_filing, cit_estimate, list_fiscal_periods, list_bank_accounts, record_payment, browser_navigate, browser_snapshot, browser_click, showcase_step, plan_tasks, update_task]
metadata:
  author: Zavora AI
  category: tax
  success-criteria:
    figures: "Every filed amount traces to a ledger report run in the same session"
    calendar: "Deadlines quoted from the KRA calendar, not guessed"
    confirmation: "Filing and remittance both explicitly confirmed by the user"
---

# Statutory Tax Filing (Kenya)

Zavora ERA records what was filed and paid; the actual submission happens on KRA iTax. Your job: produce accurate figures from the ledger, record the filing, and record the remittance — with a clean trail from report → return → payment.

## KRA calendar (quote these, don't guess)
- **Corporation tax installments**: the **20th of the 4th, 6th, 9th and 12th months** of the fiscal year (25% cumulative each); balance of tax by the **end of the 4th month after year end**. Use `cit_estimate` for the figures and schedule.
- **VAT**: monthly; return + payment due by the **20th of the following month**.
- **PAYE**: monthly; remit by the **9th of the following month** for employers under standard rules — confirm the company's applicable date if unsure (many SMEs use the 9th; iTax shows the exact obligation).
- **WHT**: remit by the **20th of the month after deduction**; issue certificates to payees.
- Late filing attracts penalties even when no tax is due — nil returns still get filed.

## Decision Tree
```
User mentions tax
├── "how much VAT do we owe for <month>?" → run_report VatReturn for the period; walk through output vs input VAT
├── "prepare/record the VAT return" → WORKFLOW: File
├── "PAYE for the payroll run" → run_report PayeP (or PayrollSummary) for the period
├── "WHT certificates" → run_report WhtCertificate
├── "did we file/pay?" → list_tax_filings → status per period
├── "corporation tax / installment tax?" → cit_estimate → walk through: accounting profit, depreciation add-back, capital allowances, taxable estimate, the installment schedule and what's paid. It is an ESTIMATE — say so; record payments as tax_type 'CIT-installment'
└── "pay KRA" → WORKFLOW: Remit
```

## WORKFLOW: File
1. Resolve the period (`list_fiscal_periods` or the user's month). 
2. Run the matching report: `run_report VatReturn` / `PayeP` / `WhtCertificate` for `period_from..period_to`. Present the figure and its makeup.
3. Cross-check: `list_tax_filings` — has this period already been filed? If yes, STOP and show it.
4. CONFIRM: "Recording the <type> return for <period> at <amount>, due <deadline>. This mirrors what you'll submit on iTax — shall I record it as filed?"
5. `file_tax_return {tax_type, period_from, period_to, amount}`.
6. Evidence: browser → **Tax Filings** → `showcase_step` ("VAT Jun 2026 filed — KES 142,300, due 20 Jul").

## WORKFLOW: Remit
1. `list_tax_filings` → find the filed, un-remitted return.
2. CONFIRM the amount and paying bank account.
3. `remit_tax_filing(id, body)` with the payment details — this books the payment against the filing.
4. Verify status flipped to remitted; showcase the result.

## MUST DO
- Figures come from the ledger reports run NOW — never reuse a stale number.
- Quote the statutory deadline with every figure.
- File nil returns when the period is nil — say so explicitly.
- Remind the user the ERP record mirrors iTax; iTax itself is the legal submission.

## MUST NOT DO
- Never invent a tax figure or "roughly estimate" a return amount.
- Never record a filing for a period that already has one.
- Never remit without the user naming/confirming the bank account.
