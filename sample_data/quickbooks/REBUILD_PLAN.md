# QuickBooks → Zavora rebuild & report comparison

Goal: recreate the QuickBooks sample company **Craig's Design and Landscaping
Services** inside Zavora through its real AR/AP/banking flows, then compare
Zavora's Balance Sheet and Profit & Loss against QuickBooks' to the cent.

Source data was extracted from QBO (Simple Start) via the `playwright-cli`
browser session using QBO's native CSV export (complete, not the virtualized DOM).

## Decisions taken
- **Fidelity**: full transaction rebuild through Zavora's real flows (not just
  report parity).
- **Tax/inventory/undeposited**: QBO uses US sales tax + FIFO inventory +
  Undeposited Funds, which don't map 1:1 to Zavora's KE-VAT model. Chosen
  approach — **real flows with tax neutralized**: post invoice/bill lines
  zero-rated, and replicate QBO's sales-tax / inventory / undeposited / payment
  movements as journal entries so the books still reconcile.

## Status
| Stage | State | Artifact |
|-------|-------|----------|
| 1. Extract QBO masters + transactions | ✅ done | `chart_of_accounts.json`, `customers.json`, `vendors.json`, `products_services.json`, `pnl_detail.csv`, `balancesheet_detail.csv`, `pnl_summary.csv`, `balancesheet_summary.csv`, `comparison_targets.json` |
| 2. Zavora tenant + chart + masters | ✅ done | `scripts/qbo/stage2_setup.py`, `zavora_maps.json` |
| 3a. Parse detail → transactions | ✅ done | `scripts/qbo/stage3_parse.py`, `transactions.json` |
| 3b. Replay transactions into Zavora | ⏳ TODO | `scripts/qbo/stage3_replay.py` (to build) |
| 4. Compare Zavora vs QBO reports | ⏳ TODO | `scripts/qbo/stage4_compare.py` (to build) |

Parse reconciles to QBO exactly: P&L income **10,200.77**, expenses **8,558.31**,
net income **1,642.46**. QBO targets: Net Income 1,642.46; Total Assets
23,436.29; Total Liabilities 31,131.33; Total Equity −7,695.04.

---

## Stage 3b — replay plan (`stage3_replay.py`)

Input: `transactions.json` (121 transactions, each with `pnl_lines` and
`bs_lines`) + `zavora_maps.json` (account name→code, customer/vendor name→id).
Target tenant: the one created in Stage 2 (see `zavora_maps.json.token_email`);
log in with the standard test password to obtain a token. Post-date: use each
transaction's QBO date; ensure fiscal periods exist for those dates first
(`POST /periods` for the relevant year(s); Craig's data is all in the current year).

### Account resolution
`code_for(ztype, name)` from `zavora_maps.json["accounts"]` keyed `"<ztype>||<name>"`.
- P&L line account → `("Revenue"|"Expense", line.account)`.
- BS line account → `("Asset"|"Liability"|"Equity", line.account)`.
- The `split` column names the contra account (e.g. `Accounts Receivable (A/R)`,
  `Checking`, `Undeposited Funds`) — resolve the same way.

### Customer/vendor resolution
Names may be `Customer:Sub` (e.g. `Shara Barnett:Barnett Design`). Resolve by
trying the full string, then the leaf after `:`, then the head before `:`,
against `zavora_maps.json["customers"]` / `["vendors"]`.

### Per-type mapping
| QBO type (count) | Zavora action |
|------------------|---------------|
| **Invoice (31)** | `POST /invoices` — customer + one line per P&L income line (`account_code`, `unit_price=amount`, `vat_treatment=ZeroRated`); discounts are negative lines. Then `POST /invoices/{id}/post`. **Sales-tax top-up**: `tax = A/R(BS line for this num) − Σ income`; if `>0`, queue a journal `DR A/R / CR Sales Tax Payable`. |
| **Bill (14)** | `POST /bills` — vendor + one line per P&L expense line (zero-rated) → `POST /bills/{id}/approve` + `/post`. |
| **Payment (16)** | customer receipt: journal `DR <split: Undeposited/Checking> / CR A/R` (amounts from BS lines). (Real `POST /payments` apply-to-invoice is a stretch goal once invoice ids are tracked.) |
| **Bill Payment (Check/Credit Card) (10)** | journal `DR A/P / CR <bank/credit card>`. |
| **Sales Receipt (4)** | journal `DR <split bank/undeposited> / CR income account(s)`. |
| **Deposit (5)** | journal `DR Checking / CR Undeposited Funds` (move from clearing). |
| **Expense / Cash Expense / Check / Credit Card Expense (34)** | journal `DR expense account(s) / CR <bank/credit card>`. |
| **Credit Card Credit (1)** | journal (reverse of CC expense). |
| **Credit Memo (1)** | journal `DR income (contra) / CR A/R` (or Zavora credit note). |
| **Refund (1)** | journal replicating the GL lines. |
| **Inventory Qty Adjust (1)** | journal `DR/CR Inventory Asset / adjustment account`. |
| **Sales Tax Payment (1)** | journal `DR Sales Tax Payable / CR Checking`. |
| **Journal Entry (2)** | journal — replicate lines directly. |

### Journal construction for the "rest"
For non-invoice/bill transactions, build ONE balanced journal per transaction
from the union of its `pnl_lines` + `bs_lines`. Convert each report `amount` to
debit/credit using the account's normal side:
- P&L Revenue line amount `a` → **credit** `a` to the income account.
- P&L Expense line amount `a` → **debit** `a` to the expense account.
- BS line: the `amount` is the signed effect on that account's balance →
  Asset/Expense increase = debit; Liability/Equity/Revenue increase = credit
  (negative flips the side).
Each QBO transaction is balanced, so the derived journal should net to zero; if a
residual ≤ 0.01 remains (rounding), plug it to a dedicated `QBO Rounding` expense
account rather than silently failing. Log any transaction whose lines don't
balance for manual review.

### Robustness
- Idempotency: the tenant is single-use; re-running means a fresh Stage 2 tenant.
- Validate each `POST` status; collect failures with the transaction key.
- Commit + push after 3b with a short load report (counts posted per type, failures).

## Stage 4 — comparison plan (`stage4_compare.py`)
1. `POST /reports` TrialBalance (as_at = QBO BS date) — assert balanced.
2. `POST /reports` ProfitAndLoss (period = this year) and BalanceSheet.
3. Build a table: for each QBO summary line (Income, COGS, Gross Profit,
   Expenses, Net Income; Total Assets/Liabilities/Equity) show **QBO vs Zavora vs
   Δ**, plus per-account deltas using the `zavora_maps` mapping.
4. Write `comparison_report.md` with matches and any residual differences, and
   explain expected structural deltas (sales tax presented as a single
   liability, inventory valuation, undeposited-funds clearing).
5. Commit + push.

## Known limitations to call out in the comparison
- QBO sales tax (multiple jurisdictions) is collapsed into one "Sales Tax
  Payable" liability in Zavora; total liability should still match.
- QBO FIFO inventory cost vs Zavora costing may differ on COGS for inventory
  items; Craig's COGS is small (405.00).
- "Unapplied Cash Payment/Bill" income/expense and Opening Balance Equity must be
  carried over for the balance sheet to tie.
