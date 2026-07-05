---
name: manage-procurement
description: Manage, monitor and post procurement in Zavora ERA — purchase requisitions, purchase orders (direct or from tender), goods receipts + 3-way match, debit notes (supplier returns), expense claims, and procurement/budget analytics. Use when the user asks to raise or approve a requisition or PO, receive goods, check a 3-way match, issue a debit note, file or approve an expense claim, email an LPO to a vendor, or review procurement spend, commitments or budget.
license: Proprietary
compatibility: Requires mcp-erp (zavora backend) and the Playwright browser tools.
allowed-tools: [procurement_list_requisitions, procurement_create_requisition, procurement_approve_requisition, procurement_convert_requisition, procurement_create_purchase_order, procurement_send_purchase_order, procurement_receive_goods, procurement_three_way_match, procurement_create_debit_note, procurement_list_expense_claims, procurement_create_expense_claim, procurement_approve_expense_claim, procurement_analytics, procurement_budget_control, list_vendors, get_vendor, list_bills, list_purchase_orders, browser_navigate, browser_snapshot, browser_click, showcase_step, plan_tasks, update_task]
metadata:
  author: Zavora AI
  category: procurement
  success-criteria:
    control-discipline: "Never approve or post above the approver's spend limit; never approve a bill for goods not received"
    confirmation: "100% of approvals, awards, and money-committing actions confirmed by the user first"
    evidence: "Every posting/award/receipt showcased with a meaningful caption"
---

# Manage Procurement (P2P)

You run the buyer's procure-to-pay process end to end and show the user the result. The flow is: **requisition → approval → (tender or direct PO) → goods receipt → 3-way match → invoice → bill approval → payment**, with debit notes and expense claims alongside.

## Decision tree
```
User mentions procurement
├── "what are we committed to / spend / budget?" → MONITOR (analytics, budget control)
├── "raise a request to buy X"                     → WORKFLOW: Requisition
├── "buy X from vendor Y now"                      → WORKFLOW: Direct PO
├── "approve requisition/claim …"                  → WORKFLOW: Approve (check spend limit)
├── "we received the goods for LPO-…"              → WORKFLOW: Receive + match
├── "can we pay bill … / is it matched?"           → 3-way match, then hand to record-payment
├── "we returned goods / were overcharged"         → WORKFLOW: Debit note
├── "claim my expenses / approve a claim"          → WORKFLOW: Expense claim
└── "email the LPO to the vendor"                  → procurement_send_purchase_order
```

## MONITOR (read-only — no confirmation needed)
- `procurement_analytics` → spend by vendor (ordered vs billed vs uninvoiced), the **open-commitment register** (POs issued but not yet invoiced), and pipeline counts. Lead with the committed total and the biggest open commitments.
- `procurement_budget_control` → budget vs **committed** (open POs) vs actual by account, with `over_budget` flags. Call out any over-budget account explicitly before recommending a new PO on it.
- `procurement_three_way_match(po_id)` → per-line ordered/received/billed with status; use before advising on a payment.

## WORKFLOW: Requisition (self-service front door)
1. `procurement_create_requisition` with `{title, department?, needed_by?, justification?, lines:[{description, quantity, uom, estimated_unit_price, account_code?}]}`. It starts as a **draft**.
2. Before recommending approval, run `procurement_budget_control` — if the account is over budget, tell the user.
3. Approval is a separate authority (see Approve). Once approved, convert with `procurement_convert_requisition(id, {target})`:
   - `target:"purchase_order"` + `vendor_id` (+ `delivery_date?`) for a single-source buy, **or**
   - `target:"tender"` (+ `closing_date?`) to invite competitive bids.
4. Showcase the resulting PR/LPO/RFQ.

## WORKFLOW: Direct PO
1. `list_vendors` → resolve the vendor id (do NOT create a vendor unasked).
2. CONFIRM the order with the user (vendor, lines, total).
3. `procurement_create_purchase_order` `{vendor_id, currency?, delivery_date?, notes?, lines:[{description, quantity, uom, unit_price, account_code?}]}`.
4. Offer to `procurement_send_purchase_order(id, {recipient_email?})` to email the LPO PDF to the vendor.
5. Showcase the LPO (Procurement → Purchase Orders).

## WORKFLOW: Receive + match
1. `procurement_receive_goods(po_id, {lines:[{po_line_id?, description, quantity_received}]})` — record what actually arrived (partial receipts are fine).
2. `procurement_three_way_match(po_id)` → confirm **matched**. If any line is `over_billed`, the goods aren't fully received — do not advise paying yet.
3. Showcase the PO detail showing the match panel.

## WORKFLOW: Approve (spend limits apply)
- Requisitions: `procurement_approve_requisition(id)`. Expense claims: `procurement_approve_expense_claim(id)`.
- These enforce **Delegation of Authority** — an amount above the approver's role limit is rejected. If you hit that error, tell the user it needs higher authority; never try to work around it.
- Bill approval also requires a passing 3-way match (goods received). If approval is blocked "record a goods receipt first", run Receive + match.
- Always CONFIRM before approving — state the number and amount.

## WORKFLOW: Debit note (supplier return / overcharge)
1. `list_vendors` → vendor id. CONFIRM the return with the user.
2. `procurement_create_debit_note` `{vendor_id, reason?, applies_to_bill?, po_id?, lines:[{description, quantity, unit_price, account_code?}]}` — this **posts** a journal that reduces the payable (DR AP / CR expense). Treat it like any posting: confirm first.
3. Showcase Debit Notes.

## WORKFLOW: Expense claim
1. `procurement_create_expense_claim` `{title, lines:[{expense_date?, description, account_code?, amount}]}` (draft).
2. Approval (`procurement_approve_expense_claim`) posts DR expense / CR payable and enforces the approver's spend limit. Confirm before approving.
3. Showcase Expense Claims.

## MUST DO
- Confirm every award, approval, PO, debit note, and claim approval before acting; drafts/requisition creation don't need confirmation.
- Respect spend limits and the 3-way-match gate — surface the block, don't circumvent it.
- Check budget (`procurement_budget_control`) before recommending new commitments.
- Showcase every posting/award/receipt with a specific caption (e.g. "LPO-2026-0007 — Anthropic, US$1,800,000, issued").

## MUST NOT DO
- Never approve above a role's spend limit, or approve a bill for goods not received.
- Never create a vendor as a side effect — ask first.
- Never merge unrelated line items to dodge a budget or limit check.
