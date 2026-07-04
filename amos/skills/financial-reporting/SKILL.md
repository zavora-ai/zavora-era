---
name: financial-reporting
description: Run and explain financial reports — trial balance, balance sheet, profit & loss, cash flow, AR/AP ageing, GL detail — in plain language for a business owner. Use when the user asks how the business is doing, requests any report, or asks about profit, position, ageing, or specific account activity.
license: Proprietary
compatibility: Requires mcp-erp (zavora backend).
allowed-tools: [run_report, get_dashboard, list_bank_accounts, browser_navigate, browser_snapshot, browser_click, showcase_step]
metadata:
  author: Zavora AI
  category: reporting
  success-criteria:
    balance-integrity: "Trial balance is_balanced verified on every TB run"
    plain-language: "Every figure explained in owner-friendly terms"
---

# Financial Reporting

You run reports with `run_report` and translate them for a non-accountant. Numbers come from the tool result only.

## Report catalog (report_type → when to use → parameters)
```
TrialBalance     → "does everything balance?", period-end checks   → as_at
BalanceSheet     → "what do we own and owe?"                       → as_at
ProfitAndLoss    → "how did we do?", income/expenses               → from + to
CashFlow         → "where did the money go?"                       → from + to
ArAgeing         → "who owes us and how late?"                     → as_at
ApAgeing         → "who do we owe and how late?"                   → as_at
GlDetail         → "show me activity on account X"                 → from + to
IncomeByCustomer → "who is our best customer?"                     → from + to
ExpenseByVendor  → "where do we spend the most?"                   → from + to
VatReturn        → VAT period figures (Zavora is NOT VAT-registered — usually nil)
EquityChanges    → "what happened to the owners' money?"           → from + to
```
Quick glance instead of a full report: `get_dashboard` (cash, receivables, payables, overdue counts).

## Decision Tree
```
User asks about the numbers
├── "how are we doing / this year / quarter"? → ProfitAndLoss (from/to = the period)
├── "cash position / bank balances"? → get_dashboard (+ list_bank_accounts for detail)
├── "who owes / do we owe"? → ArAgeing / ApAgeing (as_at today)
├── "does it balance / year-end check"? → TrialBalance (as_at period end)
└── Specific account? → GlDetail with the account code
```

## Workflow
1. Choose the report and date parameters. FY 2025 = 2025-01-01 → 2025-12-31; "as of today" = today's date.
2. `run_report(report_type, as_at | from+to)`.
3. For TrialBalance: check `is_balanced` — if false, that is the headline; report the `difference` and recommend investigating before anything else.
4. Translate: lead with the one number the user asked about, then 2–3 supporting figures. "Money in the bank", "customers still owe you", "you owe suppliers" — jargon only after the plain phrase.
5. FCY: quote KES first (functional currency), mention original currency when it matters.
6. If visual proof helps, showcase the Reports page (`/reports`) with the report on screen.

## MUST DO
- Verify is_balanced on every trial balance.
- Use the exact PascalCase report_type strings above.
- Round for speech (KES 254,000, not 254,194.07) but keep exact figures available if asked.

## MUST NOT DO
- Never mix as_at with from/to on the same call.
- Never present a figure that isn't in the tool result.
- Don't dump line-by-line report rows into speech — summarise, offer detail.
