---
name: bank-reconciliation
description: Reconcile a bank account in Zavora ERA against a bank statement — compute the GL vs cleared position, tick off cleared entries, investigate differences, and complete-and-lock the reconciliation. Use when the user asks to reconcile a bank/M-Pesa account, check why the bank balance differs from the books, or review reconciliation status at month end.
license: Proprietary
compatibility: Requires mcp-erp (zavora backend) and the Playwright browser tools.
allowed-tools: [list_bank_accounts, list_reconciliations, compute_reconciliation, complete_reconciliation, import_bank_statement, get_journal_entries, list_payments, run_report, browser_navigate, browser_snapshot, browser_click, showcase_step, plan_tasks, update_task]
metadata:
  author: Zavora AI
  category: banking
  success-criteria:
    balance-proof: "Completion only when cleared balance == statement closing balance, to the cent"
    explanations: "Every unreconciled difference is itemised, never plugged"
    confirmation: "Lock only after the user confirms"
---

# Bank Reconciliation

The reconciliation proves the books against the bank. The ERP enforces the golden rule — `complete_reconciliation` refuses unless cleared balance equals the statement closing balance — so your job is matching and explaining, never forcing.

## Decision Tree
```
User mentions the bank
├── "reconcile <account> for <month>" → WORKFLOW: Reconcile (need the statement closing balance + date)
├── "why doesn't the bank match the books?" → compute_reconciliation, explain the uncleared items
├── "are we reconciled?" → list_reconciliations + run_report BankReconSummary
└── Statement attached? → WORKFLOW: Ingest first
```

## WORKFLOW: Ingest (statement attached)
1. `analyze_attachment` → extract: statement date, closing balance, and EVERY transaction line (date, description, amount signed from the account's view). For CSV attachments, extract the raw rows.
2. `import_bank_statement {bank_account_id, filename, content}` with the lines as CSV (`date,description,amount,reference`) — idempotent, duplicates skipped; they land in the bank feed for categorisation.
3. Continue with WORKFLOW: Reconcile using the extracted closing balance + date — match the statement lines you just read against the uncleared entries.


## WORKFLOW: Reconcile
1. `list_bank_accounts` → resolve the account id. Ask for the **statement date** and **closing balance** if not given (or read them from an attached statement).
2. `compute_reconciliation {bank_account_id, statement_date}` → returns GL balance, already-cleared balance, and the uncleared entries.
3. Match: walk the uncleared entries against the statement. Build the list of `cleared_entry_ids` that appear on the statement.
4. Report the position: cleared-so-far + newly-matched vs statement closing balance. If there is a residual difference, itemise the suspects (timing: uncleared cheques/transfers; missing: bank charges, M-Pesa fees, interest) — offer to record what's missing (e.g. bank charges via the ERP) BEFORE completing.
5. CONFIRM: "Cleared balance will be <x>, statement says <x> — difference nil. Complete and lock <account> as at <date>?"
6. `complete_reconciliation {bank_account_id, statement_date, statement_closing_balance, cleared_entry_ids}`.
7. Verify with `list_reconciliations`; evidence: browser → **Banking → Reconciliation** → `showcase_step` ("KCB current a/c reconciled to 30 Jun, locked").

## MUST DO
- Get the closing balance from the STATEMENT (or the user), never from the GL.
- Itemise every shilling of difference before proposing completion.
- Record missing real transactions (charges, interest) rather than leaving them to "next month".

## MUST NOT DO
- Never post a balancing "plug" journal to force agreement.
- Never mark an entry cleared that isn't on the statement.
- Never complete-and-lock without explicit confirmation — locking is hard to undo.
