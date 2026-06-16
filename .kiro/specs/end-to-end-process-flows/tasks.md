# Implementation Plan: End-to-End Process Flows

## Overview

This plan implements the 26 end-to-end business process flow requirements by hardening the existing Zavora ERP three-layer architecture. The work focuses on adding missing validations, RBAC enforcement, period close logic, payment handling edge cases, inventory integration, and wiring the frontend to live APIs. Implementation uses Rust for backend (zavora-erp-core + zavora-erp-api) and TypeScript/React for frontend (zavora-erp-ui).

## Tasks

- [x] 1. RBAC Middleware and Enforcement
  - [x] 1.1 Create RBAC middleware in `zavora-erp-api/src/middleware/auth.rs`
    - Implement `AuthContext` struct extracting user_id, entity_id, and role from JWT/session
    - Implement `require_role()` function checking allowed roles against AuthContext
    - Return 403 with descriptive message identifying required permission on failure
    - Register middleware as an Axum layer in the router
    - _Requirements: 26.1, 26.2, 26.3, 26.4, 26.5, 26.6, 26.7_

  - [x] 1.2 Add `require_role()` guards to all mutating route handlers
    - Invoice/bill/payment creation: Owner, Admin, Accountant, Editor
    - Bill/pay run approval: Owner, Admin, Approver
    - Journal posting and period close: Owner, Admin, Accountant
    - Invoice/statement sending: Owner, Admin, Accountant, Editor
    - Role assignment: Owner, Admin only
    - Viewer gets read-only access across all modules
    - Apply guards to: `routes/invoices.rs`, `routes/bills.rs`, `routes/payments.rs`, `routes/payroll.rs`, `routes/journal.rs`, `routes/periods.rs`, `routes/settings.rs`
    - _Requirements: 26.1, 26.2, 26.3, 26.4, 26.5, 26.6, 26.7, 1.9, 2.8, 10.4, 13.6, 18.6_

  - [ ]* 1.3 Write integration tests for RBAC enforcement
    - Test each role against the permission matrix (create, approve, post, send, manage)
    - Verify 403 response with correct error message for unauthorized attempts
    - Verify all roles can perform read operations
    - _Requirements: 26.7_

- [x] 2. Checkpoint - Ensure all tests pass
  - Ensure all tests pass, ask the user if questions arise.

- [x] 3. Period Close and Journal Hardening
  - [x] 3.1 Add period status check to `services/journal.rs`
    - Before inserting any journal entry, query the target period's status
    - If SoftClosed: reject entries where source ≠ Manual (allow only prior-period adjustments)
    - If HardClosed: reject all entries (defence-in-depth alongside DB trigger)
    - Return `PeriodClosed` error with period name and status
    - _Requirements: 18.2, 18.3, 10.5, 10.6, 13.5_

  - [x] 3.2 Create `services/period_close.rs` with year-end closing logic
    - Implement `execute_year_end_close()` function
    - Verify all 12 periods of the fiscal year are HardClosed; reject if any are not
    - Compute total Revenue and Expense balances across all periods
    - Generate closing Journal_Entry: DR all Revenue accounts, CR all Expense accounts, net to Retained Earnings (4600)
    - Generate opening balance JE in first period of next fiscal year carrying forward all Balance Sheet accounts
    - Register the new service in `services/mod.rs`
    - _Requirements: 19.4, 19.5, 19.6_

  - [x] 3.3 Add soft-close and hard-close enforcement in `services/periods.rs`
    - Implement `close_period()` handler for Soft and Hard close types
    - Soft close: transition Open → SoftClosed
    - Hard close: transition SoftClosed → HardClosed; reject if period is still Open
    - Implement `reopen_period()` for SoftClosed periods (requires reason)
    - Send `PeriodCloseWarning` notification to Accountant and Admin users on soft close
    - Record audit events for PeriodClosed and PeriodReopened
    - _Requirements: 18.1, 18.4, 18.5, 18.7, 19.1, 19.3, 19.7_

  - [ ]* 3.4 Write integration tests for period close flows
    - Test: post entries → soft close → verify non-manual rejected → manual allowed
    - Test: hard close → verify all posts rejected
    - Test: year-end close with open periods → verify rejection
    - Test: successful year-end close → verify closing JE and opening balances
    - _Requirements: 18.2, 18.3, 19.4, 19.6_

- [x] 4. Invoice Lifecycle Hardening
  - [x] 4.1 Add credit limit check in `services/invoicing.rs` `post_invoice()`
    - Query customer's outstanding AR balance (sum of balance_due where status not Paid/Voided)
    - If outstanding + invoice.gross_total > customer.credit_limit: reject with `CreditLimitExceeded`
    - Send notification to Admin users on rejection via In-App and Email channels
    - _Requirements: 20.4, 20.5_

  - [x] 4.2 Add inventory issue logic in `services/invoicing.rs` on invoice post
    - For each line item where product.track_inventory is true:
      - Call `inventory::issue(item_id, quantity)` using configured costing method (FIFO/WAC)
      - Create COGS journal lines: DR 6000 COGS / CR 1500 Inventory at computed cost
    - If insufficient stock for any item: reject posting with validation error identifying item and shortfall
    - _Requirements: 23.1, 23.2, 23.3_

  - [x] 4.3 Add inventory return logic in `services/invoicing.rs` on credit note post
    - For each line item in credit note where original invoice issued inventory:
      - Call `inventory::receive(item_id, quantity, original_cost)`
      - Reverse COGS journal lines: DR 1500 Inventory / CR 6000 COGS
    - _Requirements: 23.4, 23.5_

  - [x] 4.4 Wire `create_credit_note` endpoint in `routes/invoices.rs` to service
    - Validate credit note amount does not exceed original invoice balance_due
    - Post reversing GL entry (DR Revenue, CR Accounts Receivable) proportionally
    - Reduce original invoice balance_due by credit note gross_total
    - Record audit event linking credit note to original invoice
    - _Requirements: 6.1, 6.2, 6.3, 6.4, 6.5_

  - [ ]* 4.5 Write integration tests for invoice lifecycle
    - Test: create invoice → post with credit limit exceeded → verify rejection
    - Test: post invoice with stock items → verify inventory decreased and COGS posted
    - Test: post credit note → verify inventory returned and COGS reversed
    - Test: credit note exceeding balance_due → verify rejection
    - _Requirements: 20.4, 23.1, 23.4, 6.4_

- [x] 5. Checkpoint - Ensure all tests pass
  - Ensure all tests pass, ask the user if questions arise.

- [x] 6. Payment Engine Enhancements
  - [x] 6.1 Implement overpayment handling in `services/payments.rs`
    - When payment amount exceeds invoice/bill balance_due:
      - Apply only up to balance_due against the document
      - Create unapplied credit for the remainder on the customer/vendor account
    - Create Journal_Entry: DR Bank / CR AR (applied) + CR Unapplied Payments (excess)
    - When payment has no applications: hold full amount as Unapplied_Payment
    - _Requirements: 3.4, 3.5, 3.6, 24.1_

  - [x] 6.2 Implement unapplied payment allocation in `services/payments.rs`
    - Add `apply_unapplied_payment()` function
    - Reduce payment's unapplied_balance and target document's balance_due
    - Create JE: DR Unapplied Payments / CR AR or AP
    - Reject if apply amount exceeds unapplied balance
    - Record audit event with before/after amounts
    - _Requirements: 24.2, 24.3, 24.4, 24.5_

  - [x] 6.3 Implement FX gain/loss on cross-currency payments in `services/payments.rs`
    - When payment currency differs from invoice currency:
      - Compute exchange difference between invoice rate and payment rate
      - Post gain/loss JE to account 8120 (Realised FX Gain) or 8130 (Realised FX Loss)
    - Record the exchange rate used in audit trail
    - _Requirements: 22.2, 22.3, 22.5_

  - [ ]* 6.4 Write integration tests for payment edge cases
    - Test: overpayment → verify split into applied + unapplied
    - Test: allocate unapplied funds → verify balance reduction on target doc
    - Test: cross-currency payment → verify FX gain/loss journal posted
    - Test: apply more than unapplied balance → verify rejection
    - _Requirements: 3.4, 24.2, 22.2_

- [x] 7. Scheduler: Overdue Detection and Reminders
  - [x] 7.1 Add overdue status transition logic in `services/scheduler.rs`
    - Periodic job: query invoices where due_date < now() AND balance_due > 0 AND status IN (Sent, Viewed, PartiallyPaid)
    - Transition matching invoices to Overdue status
    - Trigger reminder delivery per customer's Reminder_Policy (offset_days, channels)
    - Skip channels with no valid delivery address; log warning
    - Record audit event for each reminder delivery attempt
    - _Requirements: 5.1, 5.2, 5.3, 5.5, 5.6_

  - [x] 7.2 Cancel pending reminders when payment received in `services/scheduler.rs`
    - When a payment clears an overdue invoice's balance:
      - Cancel all pending scheduled reminders for that invoice
      - Transition invoice status from Overdue back to Paid/PartiallyPaid as appropriate
    - _Requirements: 5.4_

  - [ ]* 7.3 Write integration tests for overdue detection
    - Test: invoice past due → verify Overdue transition and reminder queued
    - Test: payment received on overdue invoice → verify reminders cancelled
    - _Requirements: 5.1, 5.4_

- [x] 8. Checkpoint - Ensure all tests pass
  - Ensure all tests pass, ask the user if questions arise.

- [x] 9. Receipt Capture and OCR Pipeline
  - [x] 9.1 Create `routes/receipts.rs` with capture and confirm endpoints
    - `POST /receipts/capture`: accept image upload, store in receipt_captures (status: pending), trigger async OCR
    - `POST /receipts/confirm`: accept capture_id, vendor_id, manual adjustments; create bill from OCR data; set capture status to Posted
    - Register routes in API router
    - _Requirements: 9.3, 9.4, 9.5_

  - [x] 9.2 Implement OCR result handling in `services/ocr.rs`
    - On OCR completion: extract vendor_name, date, total, vat_amount, line_items with confidence
    - Attempt vendor matching: fuzzy match extracted vendor_name to existing vendor records
    - If confidence < 0.7: flag for mandatory human review
    - Update receipt_capture record with ocr_result and status: reviewed
    - Record audit event linking OCR result to capture
    - _Requirements: 9.3, 9.4, 9.6, 9.7_

  - [ ]* 9.3 Write integration tests for receipt pipeline
    - Test: upload receipt → verify OCR triggered and result stored
    - Test: low confidence → verify flagged for review
    - Test: confirm receipt → verify bill created with correct amounts
    - _Requirements: 9.3, 9.5, 9.6_

- [x] 10. Bill Lifecycle and Payroll Flow Completion
  - [x] 10.1 Ensure bill approval posts GL with period validation in `services/bills.rs`
    - On approve: validate target fiscal period is Open
    - If SoftClosed or HardClosed: reject with error identifying the closed period
    - On post: create JE (DR Expense/Asset, CR Accounts Payable, CR WHT Payable if applicable)
    - Reject payment on bills in Draft or PendingApproval status
    - _Requirements: 10.3, 10.5, 10.6, 11.6_

  - [x] 10.2 Wire bill payment with WHT handling in `services/payments.rs`
    - When vendor has WHT applied: separate JE lines for WHT (DR WHT Payable / CR Bank)
    - Main payment: DR AP / CR Bank (net of WHT)
    - _Requirements: 11.4, 11.5_

  - [x] 10.3 Ensure payroll posting validates period and computes correctly in `services/payroll.rs`
    - Validate fiscal period for pay_date is Open before posting
    - Create consolidated JE per design (DR Salary Expense, CR all payable accounts)
    - Transition pay run: Draft → Approved → Posted → Paid
    - Reject if no active employees exist for the period
    - _Requirements: 12.1, 12.6, 12.7, 13.1, 13.2, 13.3, 13.5_

  - [ ]* 10.4 Write integration tests for bill and payroll flows
    - Test: approve bill in closed period → verify rejection
    - Test: pay bill with WHT vendor → verify separate WHT journal lines
    - Test: run payroll → verify PAYE, NSSF, SHA, Housing Levy computations
    - Test: post payroll in closed period → verify rejection
    - _Requirements: 10.5, 11.5, 12.2, 13.5_

- [x] 11. Bank Reconciliation and Statement Import
  - [x] 11.1 Ensure bank statement import handles all formats in `services/bank.rs`
    - Parse MT940, OFX, and CSV formats
    - Create StatementImport record with line_count, matched_count=0, unmatched_count=0
    - Add each transaction line to Categorisation_Queue with status Uncategorised
    - Reject invalid/unparseable files with descriptive error (no partial records)
    - _Requirements: 15.1, 15.3, 15.4_

  - [x] 11.2 Wire three-pass reconciliation matcher in `services/bank.rs`
    - Pass 1: exact match (amount + date + reference)
    - Pass 2: near match (amount match, date within 3 days, reference similarity > 0.8)
    - Pass 3: AI suggestion for remaining unmatched lines
    - On confirm match: link statement line to journal entry, mark both reconciled
    - On post unmatched: create new JE and link to statement line
    - Verify statement_balance == gl_balance on completion; report difference if not
    - _Requirements: 17.1, 17.2, 17.3, 17.4, 17.5, 17.6, 17.7_

  - [ ]* 11.3 Write integration tests for bank reconciliation
    - Test: import valid CSV → verify transactions in queue
    - Test: import invalid file → verify rejection with no partial records
    - Test: run three-pass matcher → verify exact matches auto-linked
    - Test: reconciliation with balance mismatch → verify prevented
    - _Requirements: 15.1, 15.4, 17.1, 17.7_

- [x] 12. Checkpoint - Ensure all tests pass
  - Ensure all tests pass, ask the user if questions arise.

- [x] 13. Frontend: Payments and Invoice Updates
  - [x] 13.1 Update `PaymentsPage.tsx` to show unapplied payments ledger
    - Add unapplied payments table showing customer/vendor, amount, date, and allocation status
    - Add "Allocate" action to apply unapplied funds to a selected document
    - Filter: visible to Accountant, Admin, and Owner roles
    - Wire to live `GET /payments?status=unapplied` and `POST /payments/apply` endpoints
    - _Requirements: 24.1, 24.2_

  - [x] 13.2 Add M-Pesa payment button to `InvoiceDetailPage.tsx`
    - Show "Pay with M-Pesa" button when invoice is Sent/Viewed/PartiallyPaid/Overdue
    - On click: prompt for phone number, call `POST /payments/mpesa-stk-push`
    - Show loading state while awaiting callback
    - Display success/failure notification based on result
    - _Requirements: 4.1, 4.2_

  - [ ]* 13.3 Write component tests for payment UI
    - Test: unapplied payments render correctly with mock data
    - Test: M-Pesa button triggers STK push API call
    - _Requirements: 24.1, 4.2_

- [x] 14. Frontend: Banking and Transactions Live Wiring
  - [x] 14.1 Update `BankingPage.tsx` with real bank account CRUD
    - Replace static demo data with live API calls to `GET/POST/PUT/DELETE /bank/accounts`
    - Add bank account creation form (name, institution, account_number, currency)
    - Show connected feed status and last sync time
    - _Requirements: 15.2_

  - [x] 14.2 Wire `TransactionsPage.tsx` to live API
    - Replace demo data with `GET /transactions` endpoint
    - Implement categorisation UI: accept suggestion, manual assign, split, merge, exclude
    - Wire split/merge/exclude actions to respective API endpoints
    - Show AI-suggested account with confidence score
    - _Requirements: 16.1, 16.2, 16.3, 16.4, 16.5_

  - [ ]* 14.3 Write component tests for banking UI
    - Test: bank accounts render from API data
    - Test: categorisation actions call correct endpoints
    - _Requirements: 15.2, 16.2_

- [x] 15. Frontend: Receipt Capture Page
  - [x] 15.1 Create `ReceiptCapturePage.tsx` with upload, review, and confirm flow
    - Upload zone: accept image/PDF, call `POST /receipts/capture`
    - Review panel: display OCR-extracted fields (vendor, date, total, VAT, lines) with confidence indicators
    - Low confidence fields highlighted for mandatory review
    - Vendor matching: show suggested vendor with option to override
    - Confirm action: submit adjustments via `POST /receipts/confirm`
    - Show created bill link on success
    - _Requirements: 9.3, 9.4, 9.5, 9.6_

  - [x] 15.2 Add receipt capture navigation and routing
    - Add route `/receipts/capture` in App.tsx router
    - Add navigation link in sidebar under Bills section
    - _Requirements: 9.3_

  - [ ]* 15.3 Write component tests for receipt capture
    - Test: upload triggers capture API call
    - Test: low confidence fields display review warning
    - Test: confirm submits correct payload
    - _Requirements: 9.5, 9.6_

- [x] 16. Final Checkpoint - Ensure all tests pass
  - Ensure all tests pass, ask the user if questions arise.

## Notes

- Tasks marked with `*` are optional and can be skipped for faster MVP
- Each task references specific requirements for traceability
- Backend implementation uses Rust (Axum framework for API, library crate for core services)
- Frontend implementation uses TypeScript with React, Redux Toolkit, and Tailwind CSS
- The design does not include Correctness Properties, so property-based tests are not included — standard integration tests cover the flows
- Checkpoints ensure incremental validation between major phases
- Notification delivery workers (Email, WhatsApp, SMS) are referenced but assumed to be a separate infrastructure concern beyond the scope of these coding tasks

## Task Dependency Graph

```json
{
  "waves": [
    { "id": 0, "tasks": ["1.1"] },
    { "id": 1, "tasks": ["1.2", "3.1", "3.2"] },
    { "id": 2, "tasks": ["1.3", "3.3"] },
    { "id": 3, "tasks": ["3.4", "4.1", "4.2", "6.1"] },
    { "id": 4, "tasks": ["4.3", "4.4", "6.2", "6.3", "7.1"] },
    { "id": 5, "tasks": ["4.5", "6.4", "7.2", "9.1"] },
    { "id": 6, "tasks": ["7.3", "9.2", "10.1", "10.2"] },
    { "id": 7, "tasks": ["9.3", "10.3", "11.1"] },
    { "id": 8, "tasks": ["10.4", "11.2"] },
    { "id": 9, "tasks": ["11.3", "13.1", "13.2"] },
    { "id": 10, "tasks": ["13.3", "14.1", "14.2"] },
    { "id": 11, "tasks": ["14.3", "15.1"] },
    { "id": 12, "tasks": ["15.2", "15.3"] }
  ]
}
```
