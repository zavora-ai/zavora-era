---
name: month-end-review
description: Run a month-end (or year-end) health check — cash position, overdue receivables and payables, trial balance integrity, and recent journal scan, ending in a plain-language findings summary. Use when the user asks for a month-end close, a books review, a health check, or "anything I should worry about?".
license: Proprietary
compatibility: Requires mcp-erp (zavora backend); browser tools optional for evidence.
allowed-tools: [get_dashboard, run_report, list_bills, list_invoices, list_payments, get_journal_entries, list_bank_accounts, browser_navigate, browser_snapshot, browser_click, showcase_step, plan_tasks, update_task]
metadata:
  author: Zavora AI
  category: period-end
  success-criteria:
    coverage: "All five checks executed every review"
    read-only: "No writes without separate, explicit user approval"
---

# Month-End Review

A read-only ritual: five checks, one clear verdict. You report findings and RECOMMEND fixes — you never fix anything inside this skill without the user separately approving each fix.

## The five checks (always all five, in order)

1. **Cash** — `get_dashboard` + `list_bank_accounts`: cash and per-account balances. Flag any negative/overdrawn account.
2. **Receivables** — `run_report(ArAgeing, as_at: period end)`: who owes us, what's overdue. Flag anything > 30 days.
3. **Payables** — `run_report(ApAgeing, as_at: period end)` + `list_bills(status: "posted")`: what we owe, what's overdue. Flag suppliers at risk of being annoyed (> 30 days).
4. **Ledger integrity** — `run_report(TrialBalance, as_at: period end)`: `is_balanced` MUST be true; if not, report the difference as the top finding.
5. **Recent activity scan** — `get_journal_entries(from: period start, to: period end)`: skim for surprises — unusually large entries, duplicates, or entries with vague descriptions.

## Verdict format
Close with exactly this structure, spoken plainly:
- **Health line**: "Your books for <period> are <in good shape | need attention>."
- **Top 3 findings**, each with a number and a recommendation ("31 bills overdue totalling KES 1,037 — worth settling the oldest first").
- **One next action** you propose to take, if any — and ask before doing it.

Optionally showcase the dashboard or a report as visual evidence for the summary.

## MUST DO
- All five checks, even when early ones look bad.
- Quantify every finding (count + KES amount).
- Distinguish "overdue to us" (chase) from "overdue by us" (pay).

## MUST NOT DO
- No postings, no fixes, no cleanup inside this review — recommend, then wait.
- Don't bury a broken trial balance under smaller findings — it always leads.
- Don't declare "all good" without having run all five checks.
