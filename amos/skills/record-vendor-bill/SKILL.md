---
name: record-vendor-bill
description: Record a supplier invoice as a vendor bill in Zavora ERA — vendor lookup, duplicate check, draft with correct currency and FX rate, user confirmation, posting, and browser evidence. Use when the user asks to record, enter, capture, or post a supplier/vendor invoice or bill (Google, Anthropic, NameCheap, etc.).
license: Proprietary
compatibility: Requires mcp-erp (zavora backend) and the Playwright browser tools.
allowed-tools: [list_vendors, get_vendor, list_bills, get_bill, create_bill_draft, post_bill, browser_navigate, browser_snapshot, browser_click, showcase_step, plan_tasks, update_task]
metadata:
  author: Zavora AI
  category: accounts-payable
  success-criteria:
    duplicate-rate: "0 duplicate bills posted"
    fx-compliance: "Every FCY bill carries currency + fx_rate"
    confirmation: "100% of postings explicitly confirmed by the user first"
---

# Record a Vendor Bill (Accounts Payable)

You record a supplier's invoice as a bill, post it to the ledger, and show the user the result. The supplier's own invoice number is the legal document reference — always capture it.

## Decision Tree
```
User mentions a supplier invoice
├── Have the details (vendor, date, amount, invoice #)? → WORKFLOW: Record
├── Details missing? → Ask ONLY for what's missing (vendor, issue date, gross amount, currency, supplier invoice number)
├── "is it already recorded?" → duplicate check only (steps 1–2), report back
└── Many bills for one vendor (e.g. 12 monthly invoices)? → process CHRONOLOGICALLY, oldest first, one confirmation for the batch
```

## WORKFLOW: Record

**Tool sequence:**
1. `list_vendors` → find the vendor by name; note its `id` and default currency. If missing, tell the user — do NOT create vendors without being asked.
2. `list_bills(limit: 200)` → duplicate check: scan ALL returned bills for the same vendor + same `vendor_invoice_number` (or same issue date + amount). The books hold 100+ bills — a small page WILL miss duplicates. If found, STOP and tell the user it already exists (give the bill number). If unsure, say so and check the Bills page in the browser before drafting.
3. `create_bill_draft` with:
   - `vendor_id`, `vendor_invoice_number` (the supplier's own number)
   - `issue_date` (the invoice date, ISO), `due_date` if stated on the invoice
   - `line_items`: one line per charge, plain descriptions, quantity × unit_price
   - **FCY rule**: if the invoice is not in KES, set `currency` AND `fx_rate` (the KES spot rate on the issue date). Never leave fx_rate at 1 for USD/EUR bills.
4. `get_bill(id)` on the fresh draft → VERIFY `gross_total` matches the supplier invoice EXACTLY. VAT may be auto-added by the product's tax setting — if the gross differs from the source invoice, tell the user the drafted total and why, and fix the draft (VAT treatment) before going further.
5. CONFIRM with the user: "I'm about to post a bill from <vendor> for <CCY gross> (≈ KES equivalent), dated <date>, invoice number <#>. Shall I post it?" Wait for a clear yes.
6. `post_bill(id)` → posts the AP journal to the ledger. Then `get_bill(id)` → verify status is posted.
7. Evidence: `browser_navigate` to the ERP → click **Bills** in the sidebar → `showcase_step` with a caption like "BILL-2025-0013 — Google Cloud EUR 4.58, posted".

## MUST DO
- Duplicate-check BEFORE drafting, every time.
- Capture the supplier's invoice number — it is the legal reference.
- FCY bills: currency + fx_rate, and quote both the original amount and the KES equivalent to the user.
- Post only after explicit confirmation; drafts don't need confirmation.
- Chronological order when recording several bills (oldest issue date first) so document numbers flow with dates.

## MUST NOT DO
- Never post without confirmation.
- Never guess an FX rate silently — if you don't know it, ask or use the rate the user gives you.
- Never merge multiple supplier invoices into one bill.
- Don't create a new vendor as a side effect — ask first.
