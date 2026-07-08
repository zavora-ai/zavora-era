---
name: record-customer-invoice
description: Raise a customer (sales) invoice in Zavora ERA — customer lookup, duplicate check, draft with correct lines and VAT, user confirmation, posting, KRA eTIMS transmission check, and browser evidence. Use when the user asks to invoice a customer, bill a client, raise/create/send a sales invoice, or re-invoice recurring work.
license: Proprietary
compatibility: Requires mcp-erp (zavora backend) and the Playwright browser tools.
allowed-tools: [list_customers, get_customer, create_customer, list_products, get_product, list_invoices, get_invoice, create_invoice_draft, submit_invoice, post_invoice, etims_status, etims_transmit_invoice, browser_navigate, browser_snapshot, browser_click, showcase_step, plan_tasks, update_task]
metadata:
  author: Zavora AI
  category: accounts-receivable
  success-criteria:
    duplicate-rate: "0 duplicate invoices posted"
    vat-compliance: "Line VAT treatment matches the product's tax setting; gross confirmed before posting"
    etims: "Every posted invoice transmits to KRA (or the failure is reported)"
    confirmation: "100% of postings explicitly confirmed by the user first"
---

# Raise a Customer Invoice (Accounts Receivable)

You create a sales invoice, post it to the ledger, and confirm it reached KRA eTIMS. The invoice becomes a legal tax document the moment it posts — accuracy before speed.

## Decision Tree
```
User wants to invoice someone
├── Have the details (customer, lines/amounts, date)? → WORKFLOW: Invoice
├── Details missing? → Ask ONLY for what's missing (customer, what was sold, quantities/prices, invoice date)
├── New customer? → confirm name/details, create_customer first (ask before creating)
├── "same as last month" / recurring? → get_invoice on the previous one, copy its lines, confirm the new period wording
└── Quote/estimate accepted? → build the invoice from the quoted lines
```

## WORKFLOW: Invoice

**Tool sequence:**
1. `list_customers` → find the customer; note `id`, currency, and payment terms. Missing? Ask, then `create_customer` (with KRA PIN if given — eTIMS uses it).
2. `list_invoices(limit: 200)` → duplicate check: same customer + same period/description or same amount on the same date. If found, STOP and show the user the existing invoice number.
3. `list_products` → match each line to a product/service so the correct income account and VAT treatment apply. No matching product? Ask whether to add one (`create_product`) or use a described free-text line.
4. `create_invoice_draft` with:
   - `customer_id`, `issue_date` (default: the work-as-of date), `due_date` per the customer's terms
   - `line_items`: quantity × unit_price per line, plain descriptions the customer will understand
   - **FCY rule**: non-KES invoices set `currency` AND `fx_rate` — never 1.0 for USD/EUR.
5. `get_invoice(id)` on the draft → VERIFY the gross total (VAT is added from the product's tax setting). Quote the net, VAT and gross to the user.
6. CONFIRM: "I'm about to post invoice to <customer> for <gross> (<net> + <VAT> VAT), dated <date>, due <due>. Shall I post it?" Wait for a clear yes.
7. `post_invoice(id)` → posts the AR journal AND auto-transmits to KRA eTIMS. Then `get_invoice(id)` → verify status and check the eTIMS fields (SCU receipt / invoice number).
8. If eTIMS did not transmit: `etims_status` to check the device, then `etims_transmit_invoice(id)` to retry — after telling the user.
9. Evidence: `browser_navigate` to the ERP → **Invoices** → `showcase_step` with a caption like "INV-2026-0042 — Acme Ltd KES 58,000, posted + eTIMS ✓".

## MUST DO
- Duplicate-check BEFORE drafting, every time.
- Verify the drafted gross against what the user expects BEFORE posting — VAT surprises erode trust.
- Confirm eTIMS transmission after posting; a fiscal invoice that never reached KRA is a compliance problem.
- Use the customer's payment terms for the due date unless told otherwise.

## MUST NOT DO
- Never post without explicit confirmation (drafts are safe; posting is legal).
- Never guess prices, quantities, or FX rates — ask.
- Never create a customer or product as a silent side effect — ask first.
- Never delete or alter a POSTED invoice — corrections go through a credit note (tell the user; the credit-note flow runs in the ERP UI).
