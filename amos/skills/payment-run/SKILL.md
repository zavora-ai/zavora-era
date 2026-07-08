---
name: payment-run
description: Prepare a cash-flow-aware supplier payment run in Zavora ERA — what's due, what cash allows, a prioritised batch proposal, and (only after explicit approval) recording each payment. Use when the user asks who to pay, wants a payment run/batch, or asks "can we afford to pay X this week?".
license: Proprietary
compatibility: Requires mcp-erp (zavora backend).
allowed-tools: [get_dashboard, run_report, list_bills, get_bill, list_bank_accounts, list_payments, list_tax_filings, record_payment, showcase_step, plan_tasks, update_task]
metadata:
  author: Zavora AI
  category: accounts-payable
  success-criteria:
    cash-safety: "Proposed batch never exceeds available cash minus a stated buffer"
    priorities: "Statutory and overdue-critical items ranked first, with reasons"
    confirmation: "No payment recorded before the user approves the batch (or a named subset)"
---

# Supplier Payment Run (prepare → approve → record)

You PREPARE the run; the money moves outside the system (bank/M-Pesa), and you
RECORD what the user approves. Never present a batch as paid — recording follows
the user's confirmation that they are making (or have made) the payments.

## Decision Tree
```
User asks about paying suppliers
├── "who should we pay this week?" → WORKFLOW: Propose
├── "can we afford X?" → cash check (dashboard) + the single bill's impact
├── "pay these" (after a proposal) → WORKFLOW: Record (the approved subset only)
└── "what did we pay last week?" → list_payments, summarise
```

## WORKFLOW: Propose
1. `get_dashboard` + `list_bank_accounts` → available cash per account.
2. `run_report ApAgeing` + `list_bills` (posted, unpaid) → the payable universe with due dates and ageing.
3. `list_tax_filings` → any filed-but-unremitted statutory amount joins the batch at TOP priority (KRA penalties beat supplier goodwill).
4. Build the proposal, in priority order:
   - P1 statutory (KRA) · P2 overdue with supply risk (key vendors) · P3 due this week · P4 early-payment advantage (only if cash is comfortable).
5. Apply the cash constraint: total ≤ available cash − a working buffer (state the buffer, default ~2 weeks of average outflow if known, else say what you assumed).
6. Present the batch as a table: vendor · bill number · due/overdue · amount · priority · pay-from account. End with the total, the cash left after, and: **"Approve the batch (or name the ones to pay) and tell me once you've made the payments — I'll record them."**

## WORKFLOW: Record
1. Only for bills the user explicitly approved.
2. `record_payment` per bill (correct bank account, date = the user's payment date).
3. Verify each application (`get_bill` → balance reduced); summarise: paid, recorded, cash remaining.

## MUST DO
- Statutory obligations always surface first, flagged, even if the user didn't ask.
- State the cash buffer assumption every time.
- FCY bills: quote both currencies at the bill's rate.

## MUST NOT DO
- Never record a payment that wasn't approved in this conversation.
- Never propose paying more than available cash minus the buffer without an explicit warning.
- Never mark the batch "done" — recording ≠ money moved; say which is which.
