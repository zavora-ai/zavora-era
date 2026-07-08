---
name: inventory-ops
description: Stock operations in Zavora ERA — check stock levels, record adjustments (count corrections, write-offs, shrinkage), transfer between locations, maintain product and vendor masters, and run the inventory valuation report. Use when the user asks about stock on hand, a stock take, adjusting/writing off stock, moving stock, or adding/updating products or vendors.
license: Proprietary
compatibility: Requires mcp-erp (zavora backend) and the Playwright browser tools.
allowed-tools: [list_products, get_product, create_product, update_product, get_stock_levels, adjust_stock, transfer_stock, create_vendor, update_vendor, run_report, browser_navigate, browser_snapshot, browser_click, showcase_step, plan_tasks, update_task]
metadata:
  author: Zavora AI
  category: inventory
  success-criteria:
    accuracy: "Adjustments carry a reason; quantities confirmed before writing"
    valuation: "Material adjustments are followed by an InventoryValuation report"
    confirmation: "100% of stock writes explicitly confirmed by the user first"
---

# Inventory Operations

Stock writes hit the ledger (inventory asset ↔ COGS/shrinkage), so treat an adjustment like a posting: verify, confirm, then write.

## Decision Tree
```
User mentions stock
├── "how much X do we have?" → get_stock_levels (read-only, no confirmation needed)
├── Stock take found a difference → WORKFLOW: Adjust
├── Damaged / expired / lost → WORKFLOW: Adjust (write-off, negative)
├── Move between shops/warehouses → WORKFLOW: Transfer
├── New product/service to sell → create_product (confirm name, price, VAT treatment, whether stock-tracked)
└── "what is our stock worth?" → run_report InventoryValuation
```

## WORKFLOW: Adjust
1. `list_products` / `get_product` → identify the exact product; `get_stock_levels` → current system quantity.
2. Compute the delta (counted − system). State both numbers and the delta to the user.
3. CONFIRM: "System shows <n>, you counted <m> — I'll adjust by <±delta> with reason '<reason>'. Proceed?"
4. `adjust_stock` with product, quantity delta, and the reason (stock take / damage / shrinkage…).
5. `get_stock_levels` → verify the new quantity. For material value changes, `run_report InventoryValuation` and tell the user the effect.
6. Evidence: browser → **Inventory** → `showcase_step` ("Stock take: SKU-014 adjusted −3, shrinkage").

## WORKFLOW: Transfer
1. Identify product, from-location, to-location, quantity; check availability with `get_stock_levels`.
2. CONFIRM the movement, then `transfer_stock`.
3. Verify levels at both locations afterwards.

## MUST DO
- Always read current levels BEFORE writing — never adjust blind.
- Every adjustment carries a reason; the auditor will read it.
- Quote the valuation impact of large write-offs.

## MUST NOT DO
- Never adjust stock to "make the numbers match" without the user naming a cause.
- Never create products with guessed prices or VAT treatment — ask.
- Transfers move quantity, not value — don't use adjust for a move.
