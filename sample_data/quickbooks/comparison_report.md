# Zavora vs QuickBooks — report comparison (Craig's Design & Landscaping)

The QuickBooks sample company was rebuilt in a fresh Zavora tenant through real
flows: **31 invoices, 13 bills, 15 customer payments, 10 bill payments, 1 credit
note, 52 journals** (plus per-invoice COGS legs), replayed from the QBO general
ledger (`transactions.json`). Sales tax neutralized per the agreed approach.

## Final result (as_at 2026-12-31)

### Profit & Loss — matches to the cent ✅
| Line | QBO | Zavora | Δ |
|------|-----:|-----:|-----:|
| Total Income | 10,200.77 | 10,200.77 | **0.00 ✓** |
| Cost of Goods Sold | 405.00 | 405.00 | **0.00 ✓** |
| Gross Profit | 9,795.77 | 9,795.77 | **0.00 ✓** |
| Operating Expenses | 5,237.31 | 5,237.31 | **0.00 ✓** |
| **Net Income** | **1,642.46** | **1,642.46** | **0.00 ✓** |

### Balance Sheet — balances; equity base exact
| Line | QBO | Zavora | Δ |
|------|-----:|-----:|-----:|
| Total Assets | 23,436.29 | 23,107.87 | −328.42 |
| Total Liabilities | 31,131.33 | 30,802.91 | −328.42 |
| Total Equity | −7,695.04 | −7,695.04* | **0.00 ✓** |

\* Zavora reports equity base −7,695.04 + current-year earnings 1,642.46 folded in;
the equity *base* matches QBO exactly. Trial balance balances (debits = credits =
41,170.08); Balance Sheet balances (Assets = Liabilities + Equity).

The single remaining Balance-Sheet delta is **−328.42 on both Assets and
Liabilities** (so the sheet stays balanced). It is entirely the **neutralized US
sales tax**: QBO carries 370.94 of Sales Tax Payable (and the matching tax inside
A/R); Zavora omits the tax charges but still replays the Sales Tax Payment, which
nets to the 328.42 difference. This is by design (tax neutralization), not an
accounting error.

## Bugs found & fixed during the rebuild
1. **`unapplied_payments` default pointed at a non-existent account (3050)** —
   customer overpayments orphaned their journal line and broke the Trial Balance.
   Fixed to the seeded `9100 Unapplied Customer Payments`. (committed)
2. **Manual journal posting used the server's startup entity** instead of the
   caller's tenant — fixed earlier so the rebuild could post at all. (committed)
3. **Multi-tenant login is scoped to the served entity** — new tenants can sign
   up but cannot log back into this instance (email is unique per-entity, not
   global). Flagged; the rebuild uses the signup token. (not fixed — needs a
   product decision + global-email migration)

## Modelling differences (expected, documented)
- **Sales tax** neutralized → the 328.42 Balance-Sheet delta above.
- **FIFO inventory** — Zavora has no FIFO engine; COGS (405) was replayed
  explicitly as the cost leg of inventory sales, so the P&L still matches.
- **Undeposited Funds** clearing reproduced via bank-account routing + deposit
  journals.

## Bottom line
Zavora reproduces QuickBooks' **Profit & Loss exactly** and its **Balance Sheet
within 1.4%**, with the entire residual explained by the deliberate sales-tax
neutralization. The exercise validated Zavora's invoice / bill / payment / credit
-note / journal posting against a real QuickBooks dataset and surfaced two real
posting bugs (now fixed) plus one multi-tenancy limitation.
