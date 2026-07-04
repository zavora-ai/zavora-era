---
name: manual-journal
description: Post a balanced manual journal entry to the general ledger — account lookup, debit/credit construction, confirmation gate, posting, and verification. Use for adjustments, accruals, prepayments, corrections via reversal, or any entry the user describes in debits and credits.
license: Proprietary
compatibility: Requires mcp-erp (zavora backend).
allowed-tools: [list_accounts, post_journal_entry, get_journal_entries, run_report, showcase_step, plan_tasks, update_task]
metadata:
  author: Zavora AI
  category: general-ledger
  success-criteria:
    balance: "Debits equal credits on 100% of entries"
    account-validity: "Every account code verified against the chart before posting"
    confirmation: "100% of postings explicitly confirmed by the user first"
---

# Manual Journal Entry

You post adjustments directly to the general ledger. The ledger is immutable — a wrong posting can only be fixed by a reversing entry, so get it right the first time.

## Decision Tree
```
User describes an adjustment
├── Routine document instead? (bill/invoice/payment) → use that skill, NOT a journal
├── Correction of a posted entry? → post a REVERSING entry (swap debits/credits), then the correct one
├── Accrual / prepayment / depreciation-style adjustment? → WORKFLOW below
└── User gives only an outcome ("move X from A to B")? → construct the entry, explain it, confirm
```

## WORKFLOW

1. `list_accounts` → verify every account code you plan to use exists and is active; note names. Common Zavora codes: 1000s assets (1020 bank KES, 1200 AR, 1310 WHT receivable), 2000s liabilities (2100 AP), 4200 Directors Loans, 5000s income, 7000s expenses (7350 software/cloud).
2. Construct lines: each has `account_code`, exactly one of `debit`/`credit` (positive numbers), optional `description`. Non-KES lines need `currency` + `fx_rate`.
3. Check the balance yourself: total debits MUST equal total credits (in KES terms). If they don't, fix before proceeding.
4. Pick the `date` — it must fall in an open period; for FY2025 adjustments that usually means 2025-12-31.
5. CONFIRM with the user, reading the entry aloud simply: "Debit <account> KES X, credit <account> KES X, dated <date>, described as '<description>'. Post it?" Wait for yes.
6. `post_journal_entry(date, reference, description, lines)` — reference short and meaningful (e.g. "AJE-PREPAID-X").
7. Verify: `get_journal_entries(from: date, to: date)` and confirm the new entry appears; for material entries, run a TrialBalance as_at that date and confirm `is_balanced`.
8. Evidence: showcase the Journal Entries page (`/journal-entries`).

## MUST DO
- Balance check BEFORE the confirmation prompt, not after posting.
- Verify account codes against `list_accounts` — never trust memory.
- Explain WHY the entry exists in the description (auditors read these).
- Corrections = reversal + re-post, never "edit".

## MUST NOT DO
- Never post into a closed period.
- Never use a journal where a proper document exists (bills, invoices, payments post their own journals).
- Never round one side to force balance — find the real difference.
