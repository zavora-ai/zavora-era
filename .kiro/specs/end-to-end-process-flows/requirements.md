# Requirements Document

## Introduction

This document specifies the end-to-end business process flows in the Zavora ERP system. It defines each critical business process as a complete lifecycle — from trigger through intermediate steps to completion — including GL impact, status transitions, notifications, edge cases, exception handling, integration points, and authorization requirements. The focus is on the six core processes a typical SME user executes daily: Invoice Lifecycle, Bill Lifecycle, Payroll Cycle, Bank Reconciliation, Period Close, and Customer Onboarding-to-Payment.

## Glossary

- **System**: The Zavora ERP application (backend engine + API + frontend)
- **Invoice_Engine**: The subsystem responsible for creating, validating, sending, and managing invoices
- **Bill_Engine**: The subsystem responsible for creating, approving, posting, and paying vendor bills
- **Payment_Engine**: The subsystem responsible for recording, applying, and reconciling payments
- **Payroll_Engine**: The subsystem responsible for computing payslips, statutory deductions, and posting payroll journals
- **Bank_Engine**: The subsystem responsible for importing statements, categorising transactions, and reconciling bank accounts
- **Period_Engine**: The subsystem responsible for managing fiscal periods, soft/hard close, and year-end procedures
- **Notification_Service**: The subsystem responsible for queuing and delivering notifications via Email, WhatsApp, SMS, and In-App channels
- **Audit_Service**: The subsystem responsible for recording before/after state for every state change
- **GL**: The General Ledger — the double-entry accounting core
- **Journal_Entry**: A balanced debit/credit entry posted to the GL
- **M-Pesa_Gateway**: The Safaricom Daraja API integration for STK Push and payment callbacks
- **OCR_Service**: The Azure AI Content Understanding integration for receipt/bill scanning
- **Categorisation_Queue**: The list of imported bank transactions awaiting GL account assignment
- **Three_Pass_Matcher**: The bank reconciliation algorithm (exact match → near match → AI suggestion)
- **RBAC**: Role-Based Access Control with roles: Owner, Admin, Accountant, Editor, Approver, Viewer
- **WHT**: Withholding Tax as per Kenya Revenue Authority regulations
- **PAYE**: Pay As You Earn income tax
- **NSSF**: National Social Security Fund contributions
- **SHA**: Social Health Authority contributions (successor to NHIF)
- **Housing_Levy**: Affordable Housing Levy (1.5% employee + 1.5% employer)
- **HELB**: Higher Education Loans Board deduction
- **Functional_Currency**: KES (Kenya Shillings) — the base reporting currency
- **Unapplied_Payment**: A payment received but not yet allocated to a specific invoice or bill
- **Credit_Limit**: The maximum outstanding AR balance allowed for a customer
- **Reminder_Policy**: A set of rules defining when and how overdue reminders are sent to a customer

## Requirements

### Requirement 1: Invoice Creation

**User Story:** As a business owner, I want to create invoices for my customers, so that I can bill them for goods and services provided.

#### Acceptance Criteria

1. WHEN a user submits a CreateInvoiceRequest with valid customer_id and at least one line item, THE Invoice_Engine SHALL create an invoice in Draft status with a sequentially generated invoice number.
2. WHEN an invoice is created, THE Invoice_Engine SHALL compute subtotal, tax_total (per VAT treatment), discount_total, gross_total, and balance_due from line items.
3. WHEN no issue_date is provided, THE Invoice_Engine SHALL default to today's date in the entity's timezone.
4. WHEN no due_date is provided, THE Invoice_Engine SHALL calculate due_date from the customer's configured payment_terms applied to the issue_date.
5. WHEN no currency is provided, THE Invoice_Engine SHALL default to the customer's configured currency.
6. WHEN the invoice currency differs from Functional_Currency, THE Invoice_Engine SHALL require an fx_rate and compute functional amounts for all line items.
7. WHEN an invoice contains inventory items, THE Invoice_Engine SHALL validate sufficient stock is available before allowing the invoice to be posted.
8. THE Audit_Service SHALL record a Created event with the full invoice state when an invoice is created.
9. WHEN a user with role Viewer attempts to create an invoice, THE System SHALL reject the request with an authorization error.

---

### Requirement 2: Invoice Sending and Delivery

**User Story:** As a business owner, I want to send invoices to customers via their preferred channel, so that they receive timely billing notifications.

#### Acceptance Criteria

1. WHEN a user submits a SendInvoiceRequest for a Draft invoice, THE Invoice_Engine SHALL transition the invoice status from Draft to Sent and record sent_at timestamp.
2. WHEN channels include Email, THE Invoice_Engine SHALL generate a branded PDF and deliver it to the customer's primary email address with a payment link.
3. WHEN channels include WhatsApp, THE Invoice_Engine SHALL deliver the invoice summary and payment link to the customer's WhatsApp-enabled phone number.
4. WHEN the invoice includes send_immediately flag on creation, THE Invoice_Engine SHALL send the invoice immediately after creation without requiring a separate send action.
5. WHEN a customer views the invoice via payment link, THE Invoice_Engine SHALL update status to Viewed and record viewed_at timestamp.
6. IF delivery fails on any channel, THEN THE Notification_Service SHALL retry delivery up to 3 times with exponential backoff and log the failure.
7. THE Audit_Service SHALL record a Sent event with delivery channels and recipient details.
8. WHEN a user with role Viewer or Approver attempts to send an invoice, THE System SHALL reject the request with an authorization error.

---

### Requirement 3: Invoice Payment Receipt and Application

**User Story:** As a business owner, I want payments to be recorded against invoices automatically or manually, so that my accounts receivable stays accurate.

#### Acceptance Criteria

1. WHEN a customer payment is recorded with applications referencing an invoice, THE Payment_Engine SHALL reduce the invoice's balance_due by the applied amount.
2. WHEN an applied payment brings balance_due to zero, THE Invoice_Engine SHALL transition the invoice status to Paid and record paid_at timestamp.
3. WHEN an applied payment reduces balance_due but does not clear it, THE Invoice_Engine SHALL transition the invoice status to PartiallyPaid.
4. WHEN a payment amount exceeds the invoice balance_due, THE Payment_Engine SHALL apply only up to balance_due and hold the remainder as unapplied credit on the customer's account.
5. WHEN a payment is recorded without applications, THE Payment_Engine SHALL hold the full amount as Unapplied_Payment on the customer's account.
6. WHEN a payment is recorded, THE Payment_Engine SHALL create a Journal_Entry debiting the bank account and crediting Accounts Receivable (for the applied portion) and Unapplied Payments (for any excess).
7. THE Notification_Service SHALL send a PaymentReceived notification to the business owner via In-App channel when a customer payment is recorded.
8. THE Audit_Service SHALL record a Paid event on the invoice and a Created event on the payment.

---

### Requirement 4: M-Pesa Payment Integration

**User Story:** As a business owner, I want customers to pay invoices via M-Pesa, so that I can offer a convenient mobile payment method popular in Kenya.

#### Acceptance Criteria

1. WHEN an invoice is sent, THE Invoice_Engine SHALL include an M-Pesa payment link containing the paybill number and the invoice number as the account reference.
2. WHEN a user initiates an STK Push for an invoice, THE M-Pesa_Gateway SHALL send the push request to Safaricom Daraja API with the invoice balance_due as the amount.
3. WHEN the M-Pesa callback reports result_code 0 (success), THE Payment_Engine SHALL automatically create a payment record with method Mpesa, apply it to the referenced invoice, and post the corresponding Journal_Entry.
4. WHEN the M-Pesa callback reports a non-zero result_code, THE Payment_Engine SHALL log the failure and notify the business owner via In-App channel.
5. WHEN an M-Pesa payment is received via paybill but cannot be matched to an invoice (unrecognised account reference), THE Payment_Engine SHALL create an Unapplied_Payment and notify the business owner to manually allocate it.
6. THE Audit_Service SHALL record the M-Pesa receipt_number, phone_number, and transaction timestamp on the payment record.

---

### Requirement 5: Invoice Overdue Detection and Reminders

**User Story:** As a business owner, I want the system to automatically detect overdue invoices and send reminders, so that I can improve cash collection without manual follow-up.

#### Acceptance Criteria

1. WHEN an invoice's due_date passes and balance_due remains greater than zero, THE Invoice_Engine SHALL transition the invoice status to Overdue.
2. WHILE an invoice is in Overdue status, THE Notification_Service SHALL send reminders according to the customer's configured Reminder_Policy (offset_days and channels).
3. WHEN a reminder is triggered at offset_days before or after due_date, THE Notification_Service SHALL deliver via all channels specified in the corresponding ReminderRule.
4. WHEN a payment is received that clears the overdue balance, THE Invoice_Engine SHALL cancel any pending scheduled reminders for that invoice.
5. IF the customer has no valid delivery address for a configured channel, THEN THE Notification_Service SHALL skip that channel and log a warning.
6. THE Audit_Service SHALL record each reminder delivery attempt with channel, recipient, and outcome.

---

### Requirement 6: Credit Note Issuance

**User Story:** As a business owner, I want to issue credit notes against invoices, so that I can formally record refunds, discounts, or corrections.

#### Acceptance Criteria

1. WHEN a credit note is created referencing an original invoice, THE Invoice_Engine SHALL create a document with invoice_type CreditNote and link it via credit_note_for field.
2. WHEN a credit note is posted, THE Invoice_Engine SHALL create a Journal_Entry that reverses the original invoice's GL impact (debit Revenue, credit Accounts Receivable) proportionally.
3. WHEN a credit note is posted, THE Invoice_Engine SHALL reduce the original invoice's balance_due by the credit note gross_total.
4. IF the credit note amount exceeds the original invoice's balance_due, THEN THE Invoice_Engine SHALL reject the credit note with a validation error.
5. THE Audit_Service SHALL record the credit note creation with a link to the original invoice.

---

### Requirement 7: Recurring Invoice Generation

**User Story:** As a business owner, I want invoices to be generated automatically on a schedule, so that I do not have to manually create them for repeat customers.

#### Acceptance Criteria

1. WHEN the current date reaches or passes a RecurringInvoice's next_run date, THE Invoice_Engine SHALL create a new invoice from the stored template with dates adjusted to the current period.
2. WHEN auto_send is true on the recurring configuration, THE Invoice_Engine SHALL send the generated invoice immediately via the customer's preferred channels.
3. WHEN auto_charge is true and the customer has M-Pesa on file, THE Invoice_Engine SHALL initiate an STK Push for the generated invoice amount.
4. WHEN a recurring invoice is generated, THE Invoice_Engine SHALL advance next_run to the next date based on the configured frequency and increment run_count.
5. WHEN a recurring invoice's end_date has passed, THE Invoice_Engine SHALL deactivate the recurring configuration and stop generating new invoices.
6. THE Audit_Service SHALL record each auto-generated invoice with source reference to the recurring configuration.

---

### Requirement 8: Estimate Creation and Conversion

**User Story:** As a business owner, I want to create estimates and convert accepted ones to invoices, so that I can quote work before committing to billing.

#### Acceptance Criteria

1. WHEN a user creates an estimate, THE Invoice_Engine SHALL assign a sequential estimate number and set status to Draft.
2. WHEN an estimate is sent to a customer, THE Invoice_Engine SHALL transition status to Sent.
3. WHEN a customer accepts an estimate, THE Invoice_Engine SHALL transition status to Accepted.
4. WHEN a user converts an accepted estimate to an invoice, THE Invoice_Engine SHALL create a new invoice copying all line items, mark the estimate status as Converted, and link via source_estimate field.
5. WHEN an estimate's expiry_date passes without acceptance, THE Invoice_Engine SHALL transition status to Expired.
6. IF a user attempts to convert a Declined or Expired estimate, THEN THE Invoice_Engine SHALL reject the conversion with a validation error.

---

### Requirement 9: Bill Creation and OCR Capture

**User Story:** As a business owner, I want to create bills from vendor invoices or capture them via OCR, so that I can track what I owe my suppliers.

#### Acceptance Criteria

1. WHEN a user submits a CreateBillRequest with valid vendor_id and line items, THE Bill_Engine SHALL create a bill in Draft status with a sequentially generated bill number.
2. WHEN a bill is created for a vendor with a configured wht_category, THE Bill_Engine SHALL automatically calculate the WHT amount based on the vendor's residency status and WHT rates.
3. WHEN a user submits a receipt image via CaptureReceiptRequest, THE OCR_Service SHALL extract vendor_name, date, total, vat_amount, and line_items with a confidence score.
4. WHEN OCR extraction completes, THE OCR_Service SHALL attempt to match the extracted vendor_name to an existing vendor record and set suggested_vendor_id.
5. WHEN a user confirms a captured receipt via ConfirmReceiptRequest, THE Bill_Engine SHALL create a bill from the OCR data (with any manual adjustments applied) and transition the capture status to Posted.
6. IF the OCR confidence score is below 0.7, THEN THE System SHALL flag the capture for mandatory human review before posting.
7. THE Audit_Service SHALL record the OCR result, adjustments, and the resulting bill creation as linked events.

---

### Requirement 10: Bill Approval Workflow

**User Story:** As a business owner, I want bills to go through an approval process before being paid, so that I can control expenditure.

#### Acceptance Criteria

1. WHEN a bill in Draft status is submitted for approval, THE Bill_Engine SHALL transition the status to PendingApproval.
2. WHEN a bill is in PendingApproval status, THE Notification_Service SHALL send a BillApprovalNeeded notification to all users with the Approver or Admin role.
3. WHEN an authorized user approves a bill, THE Bill_Engine SHALL transition status to Approved, record approved_by and approved_at, and post the Journal_Entry (debit Expense/Asset account, credit Accounts Payable, credit WHT Payable if applicable).
4. IF a user with insufficient role (Viewer or Editor without Approver role) attempts to approve a bill, THEN THE System SHALL reject the request with an authorization error.
5. WHEN a bill is posted to the GL, THE Bill_Engine SHALL validate that the target fiscal period is Open before creating the Journal_Entry.
6. IF the target fiscal period is SoftClosed or HardClosed, THEN THE Bill_Engine SHALL reject the posting with an error identifying the closed period.
7. THE Audit_Service SHALL record the approval event with the approver identity and timestamp.

---

### Requirement 11: Bill Payment

**User Story:** As a business owner, I want to record payments against bills, so that I can track what I have paid and what remains outstanding.

#### Acceptance Criteria

1. WHEN a vendor payment is recorded with applications referencing a bill, THE Payment_Engine SHALL reduce the bill's balance_due by the applied amount.
2. WHEN a payment clears the bill's balance_due, THE Bill_Engine SHALL transition the bill status to Paid.
3. WHEN a payment partially reduces balance_due, THE Bill_Engine SHALL transition the bill status to PartiallyPaid.
4. WHEN a vendor payment is recorded, THE Payment_Engine SHALL create a Journal_Entry debiting Accounts Payable and crediting the bank account.
5. WHEN a vendor has WHT applied, THE Payment_Engine SHALL create separate Journal_Entry lines for the WHT amount (debit WHT Payable, reducing the cash payment).
6. IF a user attempts to pay a bill that is still in Draft or PendingApproval status, THEN THE Payment_Engine SHALL reject the payment with a validation error indicating the bill must be approved first.
7. THE Audit_Service SHALL record the payment event with method, reference, and the resulting bill status.

---

### Requirement 12: Payroll Run Computation

**User Story:** As a business owner, I want to run payroll for a period computing all Kenya statutory deductions, so that my employees are paid correctly and compliantly.

#### Acceptance Criteria

1. WHEN a user initiates a RunPayrollRequest for a fiscal period, THE Payroll_Engine SHALL generate a payslip for each active employee (or specified subset) computing gross salary, PAYE, NSSF (employee + employer), SHA, Housing Levy (employee + employer), HELB, and net salary.
2. THE Payroll_Engine SHALL compute PAYE using the progressive tax bands (10%/25%/30%/32.5%/35%) with personal relief of KES 2,400 deducted from computed tax.
3. THE Payroll_Engine SHALL compute NSSF at 6% of pensionable income capped at KES 36,000 for both employee and employer portions.
4. THE Payroll_Engine SHALL compute SHA at 2.75% of gross salary.
5. THE Payroll_Engine SHALL compute Housing Levy at 1.5% of gross salary for both employee and employer portions.
6. WHEN payroll is computed, THE Payroll_Engine SHALL set the pay run status to Draft and compute totals (total_gross, total_paye, total_nssf, total_sha, total_housing_levy, total_helb, total_net).
7. IF no active employees exist for the period, THEN THE Payroll_Engine SHALL reject the run with a validation error.
8. WHEN an employee has a disability exemption flag, THE Payroll_Engine SHALL apply the KES 150,000 monthly disability exemption before computing PAYE.

---

### Requirement 13: Payroll Approval and Posting

**User Story:** As a business owner, I want to review and approve the payroll before it posts to the GL, so that I can verify computations before committing.

#### Acceptance Criteria

1. WHEN an authorized user approves a pay run in Draft status, THE Payroll_Engine SHALL transition the status to Approved and record approved_by and approved_at.
2. WHEN an approved pay run is posted, THE Payroll_Engine SHALL create a Journal_Entry with: debit Salary Expense (total_gross), credit PAYE Payable (total_paye), credit NSSF Payable (total_nssf employee + employer), credit SHA Payable (total_sha), credit Housing Levy Payable (total_housing_levy employee + employer), credit HELB Payable (total_helb), credit Net Salary Payable (total_net).
3. WHEN the pay run is posted, THE Payroll_Engine SHALL transition status to Posted and link the journal_entry_id.
4. THE Notification_Service SHALL send a PayRunApprovalNeeded notification to users with Approver or Admin role when a pay run is submitted for approval.
5. IF the fiscal period for the pay_date is not Open, THEN THE Payroll_Engine SHALL reject the posting with a validation error.
6. WHEN a user with role Viewer, Editor, or Accountant (without Approver) attempts to approve, THE System SHALL reject the request with an authorization error.
7. THE Audit_Service SHALL record the approval and posting events with actor identity.

---

### Requirement 14: Payroll Payment Disbursement

**User Story:** As a business owner, I want to record salary disbursements after payroll is posted, so that my bank balance and payables reflect actual payments.

#### Acceptance Criteria

1. WHEN salary payments are disbursed for a posted pay run, THE Payment_Engine SHALL create vendor payment records for each employee (or a bulk payment) debiting Net Salary Payable and crediting the bank account.
2. WHEN statutory remittances are made (PAYE, NSSF, SHA, Housing Levy, HELB), THE Payment_Engine SHALL create payment records debiting the respective payable accounts and crediting the bank account.
3. WHEN all net salaries are disbursed, THE Payroll_Engine SHALL transition the pay run status to Paid.
4. THE Audit_Service SHALL record each disbursement with the payment method and bank reference.

---

### Requirement 15: Bank Statement Import

**User Story:** As a business owner, I want to import bank statements so that I can reconcile my books against actual bank transactions.

#### Acceptance Criteria

1. WHEN a user uploads a bank statement file (MT940, OFX, or CSV format), THE Bank_Engine SHALL parse the file, create a StatementImport record, and add each transaction line to the Categorisation_Queue with status Uncategorised.
2. WHEN a bank account has feed_enabled with a configured provider (KCB, Equity, NCBA, M-Pesa), THE Bank_Engine SHALL automatically import new transactions via the provider's API at scheduled intervals.
3. WHEN statement lines are imported, THE Bank_Engine SHALL record line_count, and set matched_count and unmatched_count to 0 pending reconciliation.
4. IF the statement file format is invalid or unparseable, THEN THE Bank_Engine SHALL reject the import with a descriptive error and not create partial records.
5. IF an automatic feed import fails (API error, authentication failure), THEN THE Notification_Service SHALL send a BankFeedError notification to Admin users.
6. THE Audit_Service SHALL record the import event with format, filename, and line_count.

---

### Requirement 16: Transaction Categorisation

**User Story:** As a business owner, I want imported bank transactions to be categorised to GL accounts, so that they are properly reflected in my financial reports.

#### Acceptance Criteria

1. WHEN a transaction is imported, THE Bank_Engine SHALL generate an AI-powered AccountSuggestion with account_code, confidence score, and explanation.
2. WHEN a user accepts a categorisation suggestion or manually assigns an account, THE Bank_Engine SHALL transition the transaction status to Categorised and record the assigned_account.
3. WHEN a user splits a transaction into multiple GL parts via SplitRequest, THE Bank_Engine SHALL create TransactionSplit records and validate that split amounts sum to the original transaction amount.
4. WHEN a user merges duplicate transactions via MergeRequest, THE Bank_Engine SHALL mark duplicate records with merged_into pointing to the primary transaction.
5. WHEN a user excludes a transaction (personal or duplicate), THE Bank_Engine SHALL set status to Excluded with the provided reason.
6. WHEN a categorised transaction is posted, THE Bank_Engine SHALL create a Journal_Entry (debit/credit the assigned account, contra the bank GL account) and set status to Posted.
7. IF the split amounts do not sum to the original transaction amount, THEN THE Bank_Engine SHALL reject the split with a validation error.
8. THE Audit_Service SHALL record each categorisation action with the actor (user or agent) and the before/after state.

---

### Requirement 17: Bank Reconciliation Three-Pass Matching

**User Story:** As a business owner, I want the system to automatically match bank statement lines to GL entries, so that reconciliation requires minimal manual effort.

#### Acceptance Criteria

1. WHEN a user initiates reconciliation for a statement, THE Three_Pass_Matcher SHALL execute Pass 1 (exact match) comparing statement lines to unreconciled journal entries by amount and date.
2. WHEN Pass 1 completes, THE Three_Pass_Matcher SHALL execute Pass 2 (near match) finding entries within a configurable date tolerance (default 3 days) and reference similarity above 0.8.
3. WHEN Pass 2 completes, THE Three_Pass_Matcher SHALL execute Pass 3 (AI suggestion) for remaining unmatched lines, generating account suggestions with confidence scores.
4. WHEN a user confirms a match (exact or near), THE Bank_Engine SHALL link the statement line to the journal entry and mark both as reconciled.
5. WHEN a user posts an unmatched line via PostUnmatchedRequest, THE Bank_Engine SHALL create a new Journal_Entry for the transaction and link it to the statement line.
6. WHEN all statement lines are matched or posted, THE Bank_Engine SHALL mark the reconciliation as complete (is_reconciled = true) and verify that statement_balance equals gl_balance.
7. IF the final statement_balance does not equal gl_balance after reconciliation, THEN THE Bank_Engine SHALL report the difference and prevent marking as fully reconciled.
8. THE Audit_Service SHALL record each match confirmation, rejection, and posting action.

---

### Requirement 18: Fiscal Period Soft Close

**User Story:** As an accountant, I want to soft-close a period to prevent routine posting while still allowing adjusting entries, so that I can prepare period-end financials with confidence.

#### Acceptance Criteria

1. WHEN an authorized user submits a ClosePeriodRequest with close_type Soft, THE Period_Engine SHALL transition the period status from Open to SoftClosed.
2. WHILE a period is in SoftClosed status, THE GL SHALL reject any Journal_Entry with source other than Manual (i.e., only prior-period adjustments are allowed).
3. WHILE a period is in SoftClosed status, THE Invoice_Engine, Bill_Engine, and Payroll_Engine SHALL reject posting any document dated within that period.
4. WHEN a period is soft-closed, THE Notification_Service SHALL send a PeriodCloseWarning notification to all Accountant and Admin users.
5. WHEN an authorized user submits a ReopenPeriodRequest for a SoftClosed period, THE Period_Engine SHALL transition the period back to Open status with the reason recorded.
6. IF a user with role below Admin or Accountant attempts to close or reopen a period, THEN THE System SHALL reject the request with an authorization error.
7. THE Audit_Service SHALL record PeriodClosed and PeriodReopened events with actor and reason.

---

### Requirement 19: Fiscal Period Hard Close and Year-End

**User Story:** As an accountant, I want to hard-close a period making it immutable, and perform year-end closing, so that historical data cannot be altered after finalisation.

#### Acceptance Criteria

1. WHEN an authorized user submits a ClosePeriodRequest with close_type Hard, THE Period_Engine SHALL transition the period status from SoftClosed to HardClosed.
2. WHILE a period is in HardClosed status, THE database trigger SHALL reject any INSERT, UPDATE, or DELETE on journal entries within that period.
3. IF a user attempts to hard-close a period that is still in Open status (not yet soft-closed), THEN THE Period_Engine SHALL reject the request requiring soft close first.
4. WHEN the final period of a fiscal year is hard-closed, THE Period_Engine SHALL generate a year-end closing Journal_Entry that transfers all Revenue and Expense account balances to Retained Earnings.
5. WHEN a year-end close is executed, THE Period_Engine SHALL create opening balances in the first period of the next fiscal year carrying forward all Balance Sheet account balances.
6. IF any period within the fiscal year is not HardClosed when year-end close is attempted, THEN THE Period_Engine SHALL reject the year-end close identifying the open periods.
7. THE Audit_Service SHALL record the hard close and year-end close events as immutable records.

---

### Requirement 20: Customer Onboarding

**User Story:** As a business owner, I want to register new customers with their details and preferences, so that I can invoice them and track receivables efficiently.

#### Acceptance Criteria

1. WHEN a user submits a CreateCustomerRequest with a unique name, THE System SHALL create a customer record with a default AR account, the entity's base currency, and a default Reminder_Policy.
2. WHEN a customer is created with a kra_pin, THE System SHALL validate the KRA PIN format (P-prefixed, 9 digits + letter, or A-prefixed for companies).
3. WHEN no payment_terms are specified, THE System SHALL default to Net30.
4. WHEN a customer is created with a credit_limit, THE System SHALL enforce that no invoice can be posted if it would cause the customer's outstanding AR balance to exceed the credit_limit.
5. IF posting an invoice would exceed the customer's credit_limit, THEN THE Invoice_Engine SHALL reject the posting with a CreditLimitExceeded error and notify the Admin via In-App and Email channels.
6. THE Audit_Service SHALL record the customer creation event with all initial field values.

---

### Requirement 21: Customer Statement Generation

**User Story:** As a business owner, I want to generate and send account statements to customers, so that they have a clear record of invoices, payments, and outstanding balances.

#### Acceptance Criteria

1. WHEN a user requests a customer statement for a date range, THE System SHALL compile all invoices, credit notes, and payments for that customer within the range, showing running balance.
2. THE System SHALL generate the statement as a branded PDF matching the entity's invoice template style.
3. WHEN a user sends a statement, THE System SHALL deliver it via the customer's preferred channels (Email, WhatsApp) as configured in their contact record.
4. THE System SHALL include ageing buckets (Current, 1-30, 31-60, 61-90, 90+) in the statement summary.
5. THE Audit_Service SHALL record the statement generation and delivery event.

---

### Requirement 22: Multi-Currency Invoice and Payment Handling

**User Story:** As a business owner dealing with international clients, I want invoices and payments in foreign currencies to be properly recorded with exchange rate tracking, so that my KES-denominated reports are accurate.

#### Acceptance Criteria

1. WHEN an invoice is created in a currency other than KES, THE Invoice_Engine SHALL require an fx_rate and compute all functional (KES) amounts by multiplying transaction amounts by the fx_rate.
2. WHEN a payment is received in a currency different from the invoice currency, THE Payment_Engine SHALL compute the exchange difference and post a gain/loss Journal_Entry to the FX Gains/Losses account.
3. WHEN the GL is posted for a foreign-currency transaction, THE Journal_Entry SHALL record both transaction currency amounts (debit/credit) and functional currency amounts (functional_debit/functional_credit).
4. WHEN an FX revaluation is triggered for open AR/AP balances, THE System SHALL recompute functional balances at the new rate and post adjustment Journal_Entries for unrealised gains/losses.
5. THE Audit_Service SHALL record the exchange rate used for each transaction and any revaluation adjustments.

---

### Requirement 23: Inventory Impact on Invoice Posting

**User Story:** As a business owner selling physical goods, I want inventory to be updated when invoices are posted, so that my stock levels stay accurate.

#### Acceptance Criteria

1. WHEN an invoice containing stock items is posted, THE System SHALL issue the specified quantities from inventory using the configured costing method (FIFO or WAC).
2. WHEN inventory is issued, THE System SHALL create a Journal_Entry debiting Cost of Goods Sold and crediting the Inventory asset account for the computed cost.
3. IF insufficient stock exists for any line item at the time of posting, THEN THE Invoice_Engine SHALL reject the posting with a validation error identifying the item and shortfall.
4. WHEN a credit note is posted for an invoice that issued inventory, THE System SHALL reverse the inventory issuance (return stock) and reverse the COGS journal entry.
5. THE Audit_Service SHALL record inventory movements linked to the invoice or credit note.

---

### Requirement 24: Unapplied Payment Management

**User Story:** As a business owner, I want to see and allocate unapplied payments, so that excess or unmatched funds are eventually applied to the correct invoices or bills.

#### Acceptance Criteria

1. WHEN a payment has unapplied balance greater than zero, THE Payment_Engine SHALL display it in the unapplied payments ledger visible to Accountant, Admin, and Owner roles.
2. WHEN a user submits an ApplyPaymentRequest allocating unapplied funds to a document, THE Payment_Engine SHALL reduce the payment's unapplied balance and reduce the document's balance_due accordingly.
3. WHEN unapplied funds are applied, THE Payment_Engine SHALL create an additional Journal_Entry crediting the Unapplied Payments account and debiting the appropriate receivable/payable.
4. IF the apply amount exceeds the payment's unapplied balance, THEN THE Payment_Engine SHALL reject the request with a validation error.
5. THE Audit_Service SHALL record each application event with before/after unapplied amounts.

---

### Requirement 25: End-to-End Audit Trail Integrity

**User Story:** As an accountant, I want every state change in the system to be recorded with before/after state, so that I have a complete audit trail for compliance and troubleshooting.

#### Acceptance Criteria

1. THE Audit_Service SHALL record an AuditEvent for every state transition in Invoice_Engine, Bill_Engine, Payment_Engine, Payroll_Engine, Bank_Engine, and Period_Engine.
2. THE Audit_Service SHALL capture before_state and after_state as JSON snapshots for every update operation.
3. THE Audit_Service SHALL record the actor (User or Agent) performing each action via the AgentOrUserId type.
4. THE Audit_Service SHALL record the exact timestamp of each event using UTC.
5. WHILE a period is HardClosed, THE Audit_Service SHALL ensure that audit records for that period are immutable (no deletion or modification).
6. THE System SHALL provide paginated audit queries filterable by object_type, object_id, actor, event_type, and date range.

---

### Requirement 26: Role-Based Access Control Enforcement

**User Story:** As a business owner, I want each role to have specific permissions at each step of the business process, so that sensitive operations are restricted to authorized personnel.

#### Acceptance Criteria

1. THE System SHALL enforce the following create permissions: Owner, Admin, Accountant, and Editor roles MAY create invoices, bills, and payments; Approver and Viewer roles SHALL NOT create these documents.
2. THE System SHALL enforce the following approval permissions: only Owner, Admin, and Approver roles MAY approve bills and pay runs.
3. THE System SHALL enforce the following posting permissions: only Owner, Admin, and Accountant roles MAY post journal entries and close fiscal periods.
4. THE System SHALL enforce the following send permissions: Owner, Admin, Accountant, and Editor roles MAY send invoices and statements; Viewer and Approver roles SHALL NOT send documents.
5. THE System SHALL enforce that only Owner and Admin roles MAY modify RBAC role assignments.
6. THE System SHALL enforce that Viewer role has read-only access to all modules with no create, update, delete, approve, or post capabilities.
7. IF any unauthorized action is attempted, THEN THE System SHALL return a 403 Forbidden response with a message identifying the required permission.
