---
name: erp-showcase
description: Show work visually in the Zavora ERA browser — navigate to any ERP page, verify what's on screen, and file screenshot evidence cards. Use when the user asks to see something in the ERP, to show/prove work done, or after any posting that deserves visible evidence.
license: Proprietary
compatibility: Requires the Playwright browser tools (browser MCP) with ERP auto-login.
allowed-tools: [browser_navigate, browser_snapshot, browser_click, browser_type, browser_wait_for, browser_take_screenshot, showcase_step, update_task]
metadata:
  author: Zavora AI
  category: evidence
  success-criteria:
    evidence-rate: "Every write showcased; every showcase has a meaningful caption"
    snapshot-discipline: "No click/type without a fresh snapshot ref"
---

# ERP Showcase (Browser Evidence)

You drive a real Chrome window through Zavora ERA and file evidence cards the user sees next to the chat. Navigation to the ERP signs you in automatically — never handle the login form yourself.

## ERP page map (sidebar routes)
```
/                  Dashboard (cash, AR/AP, P&L snapshot)
/invoices          Sales invoices        /estimates        Estimates
/customers         Customers             /bills            Vendor bills (AP)
/vendors           Suppliers             /payments         Payments (in & out)
/banking           Bank accounts         /reconciliation   Bank reconciliation
/transactions      Transactions feed     /accounts         Chart of accounts
/journal-entries   Journal entries       /reports          Financial reports
/products          Products              /assets           Fixed assets
```

## WORKFLOW

1. `browser_navigate` to the ERP root URL → auto-login lands you on the dashboard.
2. `browser_snapshot` → read the page; every subsequent click/type MUST use a ref from the latest snapshot.
3. Navigate: click the sidebar link for the page you need (see map). For a specific document, click its row in the list.
4. `browser_snapshot` again → CONFIRM the target content is actually on screen (the bill number, the report title). Never showcase a page you haven't verified.
5. `showcase_step(caption)` — the caption states what the evidence proves: "BILL-2025-0013 posted, EUR 4.58" beats "Bills page".
6. Update the workplan task and tell the user what they're looking at.

## Recovery ladder (when an action fails)
1. Fresh `browser_snapshot` → retry with the correct ref.
2. `browser_wait_for` (text you expect) → snapshot → retry.
3. `browser_navigate` directly to the route from the page map → snapshot → retry.
4. Only after all three: mark the task failed with a note saying exactly what you saw.

## MUST DO
- Snapshot before EVERY interaction; refs go stale after navigation.
- Verify content before capturing evidence.
- One showcase per meaningful state — after a posting, after a report renders.

## MUST NOT DO
- Never type into the login form — navigation handles authentication.
- Never showcase an error page or half-loaded screen as if it were success.
- Don't spam showcases of the same screen.
