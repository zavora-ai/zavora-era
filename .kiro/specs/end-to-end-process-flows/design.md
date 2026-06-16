# Design Document

## Overview

This document details the technical design for implementing the 26 end-to-end business process flow requirements in Zavora ERP. The design maps each requirement to specific components in the existing three-layer architecture (zavora-erp-core services → zavora-erp-api routes → zavora-erp-ui pages), identifies gaps between current implementation and requirements, and specifies the changes needed to achieve full compliance.

The system is already substantially built. This design focuses on **hardening the process flows** — adding missing validations, wiring notifications, enforcing RBAC at every step, and connecting integration points that are currently stubbed.

## Architecture Overview

```
┌────────────────────────────────────────────────────────────────────┐
│                        Frontend (React)                              │
│  Pages → API Client (axios) → /api/v1/* → Axum Router             │
└────────────────────────────────────────────────────────────────────┘
                                  │
┌────────────────────────────────────────────────────────────────────┐
│                        API Layer (Axum)                              │
│  Route Handlers → RBAC Middleware → Service Calls                  │
└────────────────────────────────────────────────────────────────────┘
                                  │
┌────────────────────────────────────────────────────────────────────┐
│                     Core Engine (Library)                            │
│  Services │ Journal Engine │ Scheduler │ Notifications │ Audit     │
└────────────────────────────────────────────────────────────────────┘
                                  │
┌──────────────────┐  ┌──────────────────┐  ┌──────────────────────┐
│  PostgreSQL 17   │  │  Redis 7         │  │  External Services   │
│  (data + triggers)│  │  (queues + audit)│  │  (M-Pesa, Email, AI)│
└──────────────────┘  └──────────────────┘  └──────────────────────┘
```

## Component Design

### 1. RBAC Middleware (NEW — Requirements 26, 9.4, 10.4, 13.6)

**Current state:** RBAC roles defined in `rbac/mod.rs` with permission methods, but NOT enforced in API routes.

**Design:** Add an Axum middleware layer that extracts user identity from JWT/session and checks permissions before the route handler executes.

```rust
// New file: zavora-erp-api/src/middleware/auth.rs
pub struct AuthContext {
    pub user_id: Uuid,
    pub entity_id: Uuid,
    pub role: UserRole,
}

pub async fn require_role(
    required: &[UserRole],
    ctx: &AuthContext,
) -> Result<(), ErpError> {
    if !required.contains(&ctx.role) {
        return Err(ErpError::PermissionDenied {
            action: "...",
            required_role: "...",
        });
    }
    Ok(())
}
```

**Permission matrix:**
| Action | Roles Allowed |
|--------|--------------|
| Create invoice/bill/payment | Owner, Admin, Accountant, Editor |
| Send invoice | Owner, Admin, Accountant, Editor |
| Approve bill/pay run | Owner, Admin, Approver |
| Post journal entry | Owner, Admin, Accountant |
| Close/reopen period | Owner, Admin |
| Manage users | Owner, Admin |
| View (read-only) | All roles |

---

### 2. Invoice Lifecycle State Machine (Requirements 1-7)

**State transitions:**
```
Draft → Sent → Viewed → [PartiallyPaid] → Paid
  │                                          ↑
  └─── (post) ────────────────────────────────┘
  
Draft → Voided (cancel before send)
Sent/Viewed/PartiallyPaid → Overdue (scheduler detects past due)
Any (except Voided) → CreditNote issued (reduces balance)
```

**GL Impact per transition:**
| Transition | Journal Entry |
|-----------|--------------|
| Draft → Sent (post) | DR 1200 AR / CR 5xxx Revenue / CR 3100 VAT Output |
| Payment received | DR 1020 Bank / CR 1200 AR |
| Credit note posted | DR 5xxx Revenue + DR 3100 VAT / CR 1200 AR |
| Overdue | No GL impact (status only) |

**Notification triggers:**
| Event | Recipients | Channels |
|-------|-----------|----------|
| Invoice sent | Customer | Email, WhatsApp |
| Payment received | Owner, Invoice creator | In-App, Email |
| Invoice overdue | Customer (per policy) | Email, WhatsApp, SMS |
| Credit note issued | Customer, Owner | Email, In-App |

---

### 3. Bill Lifecycle State Machine (Requirements 9-11)

**State transitions:**
```
[OCR Capture] → Draft → PendingApproval → Approved → Posted → [PartiallyPaid] → Paid
                  │                                                                  
                  └── Cancelled                    Posted → Disputed
```

**GL Impact:**
| Transition | Journal Entry |
|-----------|--------------|
| Approved → Posted | DR 7xxx Expense / CR 3010 AP / DR 1300 VAT Input / CR 3210 WHT Payable |
| Payment made | DR 3010 AP / CR 1020 Bank + DR 3210 WHT / CR 1020 Bank |

**Validation rules:**
- Cannot approve if period is closed
- Cannot pay if not Approved/Posted
- WHT auto-calculated from vendor.wht_category + vendor.resident

---

### 4. Payroll State Machine (Requirements 12-14)

**State transitions:**
```
RunPayroll → Draft → Approved → Posted → Paid
```

**Computation pipeline (per employee):**
1. Gross = basic_salary + Σ(allowances)
2. NSSF_employee = 6% × min(gross, 36000)
3. Housing_Levy_employee = 1.5% × gross
4. Taxable = gross - NSSF_employee - Housing_Levy_employee
5. If disability: taxable = max(0, taxable - 150000)
6. PAYE = progressive_bands(taxable) - personal_relief(2400)
7. SHA = 2.75% × gross
8. Net = gross - PAYE - NSSF_employee - SHA - Housing_Levy_employee - HELB

**GL Posting (single consolidated entry):**
```
DR 7010 Salaries        (total_gross)
DR 7020 Employer NSSF   (employer_nssf)
DR 7030 Employer HL     (employer_housing_levy)
  CR 3310 PAYE Payable     (total_paye)
  CR 3320 NSSF Payable     (total_nssf = emp + employer)
  CR 3330 SHA Payable      (total_sha)
  CR 3340 HELB Payable     (total_helb)
  CR 3350 HL Payable       (total_housing_levy = emp + employer)
  CR 3400 Net Pay Payable  (total_net)
```

---

### 5. Bank Reconciliation Pipeline (Requirements 15-17)

**Three-pass algorithm detail:**

```
Pass 1 — Exact Match:
  WHERE stmt.amount = je.amount 
    AND stmt.date = je.date 
    AND stmt.reference = je.reference
  → Auto-match, confidence 1.0

Pass 2 — Near Match:
  WHERE stmt.amount = je.amount
    AND ABS(stmt.date - je.date) <= 3 days
    AND fuzzy_match(stmt.reference, je.reference) > 0.8
  → Suggest, confidence 0.8-0.99

Pass 3 — AI Suggestion:
  WHERE unmatched after Pass 1+2
  → Embedding similarity to historical categorisations
  → Suggest account_code, confidence 0.5-0.9
```

**Data flow:**
```
Bank Statement (CSV/MT940/OFX)
  → Parse → ImportedTransaction rows (status: uncategorised)
  → Three_Pass_Matcher runs
  → Matched lines → link to journal_entry_id
  → Unmatched → Categorisation Queue (UI)
  → User categorises/splits/merges → Post → Journal Entry
  → All matched → Reconciliation complete
```

---

### 6. Period Close Sequence (Requirements 18-19)

**Soft close enforcement points:**
- `services/journal.rs` → check period status before inserting
- `services/invoicing.rs` → check before post_invoice()
- `services/bills.rs` → check before bill GL posting
- `services/payroll.rs` → check before post_pay_run()

**Hard close enforcement:**
- PostgreSQL trigger `trg_prevent_hardclosed_insert` on `journal_lines`
- Cannot be bypassed even via direct SQL access

**Year-end close procedure:**
1. Verify all 12 periods are HardClosed
2. Compute P&L totals (all Revenue - all Expense accounts)
3. Create closing JE: DR Revenue accounts / CR Expense accounts / DR/CR Retained Earnings (4600)
4. Create opening balance JE in period 1 of next year: carry forward all BS account balances

---

### 7. Notification Delivery Architecture (Requirements 2, 5, 10, 13, 15)

**Current state:** Redis XADD to `erp:notifications:{entity_id}` stream.

**Design for delivery workers (not yet implemented):**

```
Redis Stream (erp:notifications:{entity_id})
  │
  ├── Email Worker → SendGrid/SMTP
  ├── WhatsApp Worker → WhatsApp Business API
  ├── SMS Worker → Africa's Talking API
  └── In-App Worker → WebSocket push to frontend
```

**Retry policy:**
- Max 3 attempts
- Exponential backoff: 30s, 5min, 30min
- Dead letter after 3 failures → notify Admin

---

### 8. M-Pesa Integration Flow (Requirement 4)

**Outbound (STK Push):**
```
User clicks "Pay with M-Pesa" on invoice
  → API: POST /payments/mpesa-stk-push { invoice_id, phone }
  → Backend calls Daraja /mpesa/stkpush/v1/processrequest
  → Returns checkout_request_id
  → Customer sees STK popup on phone
```

**Inbound (Callback):**
```
Safaricom calls: POST /payments/mpesa-callback
  → Validate: result_code == 0
  → Match invoice via account_reference (invoice number)
  → record_mpesa_payment(invoice_id, callback)
  → Creates Payment, applies to invoice, posts JE
  → Notifies owner: "Payment received from +2547XX"
```

**Edge cases:**
- Duplicate callback → idempotency check on mpesa_receipt_number
- Unmatched reference → create Unapplied_Payment
- Timeout (no callback within 60s) → mark as pending, poll status

---

### 9. OCR Receipt Pipeline (Requirement 9)

**Flow:**
```
User uploads photo/PDF
  → POST /receipts/capture { image_url }
  → Store in receipt_captures (status: pending)
  → Async: call Azure AI Content Understanding
  → On result: update ocr_result, status: reviewed
  → If confidence < 0.7: flag for mandatory review
  → User reviews, adjusts, confirms
  → POST /receipts/confirm { capture_id, vendor_id, adjustments }
  → Creates Bill from OCR data
  → Capture status: posted, linked to bill
```

---

### 10. Credit Limit Enforcement (Requirement 20)

**Check point:** `post_invoice()` in `services/invoicing.rs`

```rust
// Before creating the GL journal entry:
if let Some(credit_limit) = customer.credit_limit {
    let outstanding = sum(invoices.balance_due WHERE customer_id AND status NOT IN (paid, voided));
    if outstanding + invoice.gross_total > credit_limit {
        // Notify admin
        notify(CreditLimitExceeded, admin_users);
        return Err(ErpError::CreditLimitExceeded { ... });
    }
}
```

---

### 11. Multi-Currency FX Handling (Requirement 22)

**On transaction creation:**
- Validate fx_rate provided when currency ≠ KES
- Store both transaction amounts and functional amounts
- `functional_debit = debit × fx_rate`

**On payment in different currency:**
- Compute realised FX gain/loss
- Post to accounts 8120 (Realised FX Gain) / 8130 (Realised FX Loss)

**Period-end revaluation (already implemented in `services/fx.rs`):**
- Recompute all FCY balances at period-end rate
- Post unrealised gain/loss to 8100/8110
- Auto-reverse on first day of next period

---

### 12. Inventory Integration (Requirement 23)

**On invoice post (stock items):**
```rust
for line in invoice.lines {
    if line.product.track_inventory {
        issue_inventory(item_id, quantity)?;  // updates on_hand, available
        // JE: DR 6000 COGS / CR 1500 Inventory (at WAC or FIFO cost)
    }
}
```

**On credit note post:**
```rust
for line in credit_note.lines {
    if line.product.track_inventory {
        receive_inventory(item_id, quantity, original_cost)?;  // reverses
        // JE: DR 1500 Inventory / CR 6000 COGS
    }
}
```

---

## Changes Required

### Backend (zavora-erp-core)

| File | Change | Requirement |
|------|--------|-------------|
| `services/invoicing.rs` | Add credit limit check in `post_invoice()` | R20 |
| `services/invoicing.rs` | Add inventory issue on post (stock items) | R23 |
| `services/invoicing.rs` | Add inventory return on credit note post | R23 |
| `services/journal.rs` | Check period status (soft-close blocks non-manual) | R18 |
| `services/payments.rs` | Handle overpayment → unapplied split | R3, R24 |
| `services/payments.rs` | Compute FX gain/loss on cross-currency payment | R22 |
| `services/scheduler.rs` | Add overdue status transition logic | R5 |
| `services/scheduler.rs` | Cancel reminders on payment received | R5 |
| NEW: `services/period_close.rs` | Year-end closing entry logic | R19 |

### API Layer (zavora-erp-api)

| File | Change | Requirement |
|------|--------|-------------|
| NEW: `middleware/auth.rs` | RBAC enforcement middleware | R26 |
| `routes/*.rs` | Add `require_role()` checks to all mutating handlers | R26 |
| `routes/invoices.rs` | Wire `create_credit_note` to actual service | R6 |
| `routes/assets.rs` | Wire `run_depreciation` to actual service | R19 |
| NEW: `routes/receipts.rs` | Receipt capture/confirm endpoints | R9 |

### Frontend (zavora-erp-ui)

| File | Change | Requirement |
|------|--------|-------------|
| `PaymentsPage.tsx` | Show unapplied payments ledger | R24 |
| `InvoiceDetailPage.tsx` | Show payment link / M-Pesa button | R4 |
| `BankingPage.tsx` | Real bank account CRUD (not static demo) | R15 |
| `TransactionsPage.tsx` | Wire to live API (not demo data) | R16 |
| NEW: `ReceiptCapturePage.tsx` | Upload + review + confirm flow | R9 |

---

## Data Flow Diagrams

### Invoice-to-Cash Flow
```
Customer ─── Estimate ──→ Invoice ──→ Send ──→ Payment ──→ Reconcile
                │              │         │         │            │
                │              │         │         │            │
             (no GL)     (DR AR/CR Rev) (notify) (DR Bank/CR AR) (match)
                │              │         │         │            │
                ▼              ▼         ▼         ▼            ▼
           estimates      invoices   notifications  payments   bank_recon
             table          table      stream       table      matched
```

### Procure-to-Pay Flow
```
Vendor ─── Receipt/OCR ──→ Bill ──→ Approve ──→ Post ──→ Pay ──→ Reconcile
              │               │        │          │        │         │
              │               │        │          │        │         │
          (extract)     (auto WHT) (notify)  (DR Exp/  (DR AP/   (match)
              │               │        │     CR AP)   CR Bank)      │
              ▼               ▼        ▼       ▼        ▼          ▼
         receipt_captures   bills   notify  journal   payments  bank_recon
```

### Payroll Cycle
```
Employees ──→ Run Payroll ──→ Review ──→ Approve ──→ Post ──→ Disburse
    │              │             │           │          │         │
    │              │             │           │          │         │
 (master)    (compute all   (payslips)  (notify)  (DR Salary  (DR Net Pay
  data        deductions)    displayed   admins   CR Payables) CR Bank)
    │              │             │           │          │         │
    ▼              ▼             ▼           ▼          ▼         ▼
 employees     pay_runs      payslips    notify    journal    payments
```

---

## Error Handling Strategy

| Error Class | HTTP Code | User Message | System Action |
|------------|-----------|--------------|---------------|
| ValidationFailed | 400 | "Entry is unbalanced" | Reject, no state change |
| PeriodClosed | 409 | "Period Jun 2026 is closed" | Reject posting |
| InsufficientStock | 409 | "Only 5 available, 10 requested" | Reject invoice post |
| CreditLimitExceeded | 409 | "Would exceed KES 500,000 limit" | Reject, notify admin |
| Overpayment | 400 | "Payment exceeds balance" | Split: apply balance, unapply rest |
| PermissionDenied | 403 | "Requires Approver role" | Reject, log attempt |
| DuplicateReference | 409 | "Reference already exists" | Reject |
| FxRateNotFound | 400 | "No USD/KES rate for 2026-06-10" | Reject, suggest adding rate |
| Database | 500 | "Internal error" | Log, alert, retry if transient |
| Redis | 500 | "Notification queuing failed" | Log, continue (non-blocking) |

---

## Testing Strategy

Each requirement maps to integration tests that verify the complete flow:

1. **Invoice lifecycle test:** Create → Post → Send → Record payment → Verify Paid status + GL balanced
2. **Bill lifecycle test:** Create (vendor with WHT) → Submit → Approve → Post → Verify WHT journal lines
3. **Payroll test:** Add employees → Run → Verify PAYE computation → Approve → Post → Verify GL
4. **Recon test:** Import CSV → Run matcher → Confirm matches → Post unmatched → Verify reconciled
5. **Period close test:** Post entries → Soft close → Verify rejection → Hard close → Verify DB trigger
6. **Credit limit test:** Set limit → Post invoice at limit → Post another → Verify rejection
7. **Multi-currency test:** Create USD invoice → Receive KES payment → Verify FX gain/loss posted
8. **RBAC test:** Attempt operations as each role → Verify allowed/denied per matrix
