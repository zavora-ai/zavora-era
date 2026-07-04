---
name: record-payment
description: Record money in or out — customer receipts (with Kenyan withholding tax) and vendor payments, applied against invoices or bills. Use when the user says a customer paid, we paid a supplier, record a receipt/payment, or mentions WHT withheld on a payment.
license: Proprietary
compatibility: Requires mcp-erp (zavora backend).
allowed-tools: [list_customers, list_vendors, list_invoices, list_bills, list_bank_accounts, list_payments, record_payment, showcase_step, plan_tasks, update_task]
metadata:
  author: Zavora AI
  category: payments
  success-criteria:
    application-accuracy: "Payments applied to the right documents, no unapplied leftovers unless intended"
    wht-correctness: "WHT always entered in KES, never in the invoice currency"
    confirmation: "100% of payments explicitly confirmed by the user first"
---

# Record a Payment (Receipts & Vendor Payments)

You record cash movement: customer receipts (money in, `payment_type: "customer_payment"`) and vendor payments (money out, `payment_type: "vendor_payment"`), applying them to open invoices or bills.

## Decision Tree
```
Money moved
├── Customer paid us? → WORKFLOW A (receipt; check for WHT)
├── We paid a supplier? → WORKFLOW B (vendor payment)
├── Director paid personally (no company cash moved)? → WORKFLOW B with funding_account (e.g. "4200" Directors Loans) instead of bank_account_id
└── "why is this payment unapplied?" → list_payments, inspect applications, explain
```

## WORKFLOW A: Customer receipt

1. `list_customers` → find the payer's `id`.
2. `list_invoices` → find the open invoice(s) being settled; note ids and balances.
3. `list_bank_accounts` → pick the account the money landed in (match the currency: USD receipts → USD account, M-Pesa → the M-Pesa till).
4. Build `record_payment`:
   - `payment_type: "customer_payment"`, `party_id`, `payment_date` (the date money arrived), `amount` (in the payment currency), `currency`, `fx_rate` for FCY
   - `method`: "mpesa" | "bank_transfer" | "cash" | "cheque" (+ `reference` — the transaction id)
   - `bank_account_id`, and `applications: [{document_id, amount}]` covering the invoices settled
   - **WHT rule (Kenya)**: if the customer withheld tax, set `wht_amount` in **KES** (KRA denominates WHT in KES regardless of invoice currency). Cash received + WHT together clear the invoice: applications total = cash + WHT-covered portion. The WHT becomes a WHT-receivable asset (a tax credit), not lost income — say this to the user.
5. CONFIRM: "Recording <CCY amount> from <customer> on <date> into <account>, applied to <invoice #>[, with KES <wht> withheld as WHT]. Go ahead?" Wait for yes.
6. `record_payment` → then `list_payments(limit: 3)` to verify it landed with the right applications.
7. Evidence: showcase the Payments page.

## WORKFLOW B: Vendor payment

Same shape: `payment_type: "vendor_payment"`, party is the vendor, applications settle bills (`list_bills` for open ones). Director-funded purchases: use `funding_account` (GL code, e.g. "4200") instead of `bank_account_id` — no company cash account is touched.

## MUST DO
- Applications must reference real open documents — fetch them first, never guess ids.
- Match payment currency to the bank account currency; FCY needs `fx_rate` at the payment date.
- WHT is ALWAYS in KES.
- Explain FX gains/losses simply if the KES value differs from the invoice ("the shilling moved between invoicing and payment").

## MUST NOT DO
- Never record a payment larger than the open balance without telling the user the excess stays unapplied.
- Never invent an M-Pesa/bank reference — leave it blank if unknown.
- Don't mark WHT on vendor payments unless the user says WE withheld from the supplier.
