# Zavora vs QuickBooks — report comparison (Craig's Design & Landscaping)

Rebuild: 31 invoices + 13 bills + 15 customer payments + 10 bill payments + 49
journals replayed into a fresh Zavora tenant from the QBO general ledger
(`transactions.json`). Sales tax neutralized (zero-rated lines).

## First pass results (as_at 2026-12-31)

### Profit & Loss
| Line | QBO | Zavora | Δ |
|------|-----:|-----:|-----:|
| Total Income | 10,200.77 | 10,300.77 | **+100.00** |
| Cost of Goods Sold | 405.00 | 0.00 | **−405.00** |
| Gross Profit | 9,795.77 | 10,300.77 | +505.00 |
| Operating Expenses | 5,237.31 | 5,237.31 | **0.00 ✓** |
| Net Income | 1,642.46 | 2,147.46 | +505.00 |

### Balance Sheet
| Line | QBO | Zavora | Δ |
|------|-----:|-----:|-----:|
| Total Assets | 23,436.29 | 23,512.87 | +76.58 |
| Total Liabilities | 31,131.33 | 30,478.49 | −652.84 |
| Total Equity | −7,695.04 | −5,042.58 | +2,652.46 |

Trial balance: **NOT balanced**, Δ 224.42 — investigate first.

## Open items (to reconcile)
1. **Trial balance off by 224.42** — every entry should balance; likely an
   unapplied/over-applied customer-payment posting. Root-cause before trusting
   the rest.
2. **COGS −405** — QBO posts COGS from FIFO inventory on product sales; the
   zero-rated invoice replay doesn't move inventory/COGS. Replay the inventory
   COGS leg (or the Inventory Qty Adjust) to recover the 405.
3. **Income +100** — one extra 100 of income (round number); candidates: Refund /
   Unapplied Cash Payment Income / the 1 unresolved `Revenue::None` line.
4. **Liabilities −652.84** — sales tax payable omitted (neutralized) + credit-card
   payable differences; expected given tax neutralization.
5. The 1 Credit Memo (touches A/R control) was skipped — needs the credit-note flow.

## What already ties out
- Operating expenses match to the cent (bills + cash/card expenses).
- The parse-level income/expense totals matched QBO exactly (`transactions.json`).
- Net-income difference (+505) is fully explained by items 2 (+405) and 3 (+100).
