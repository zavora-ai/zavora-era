



ZAVORA TECHNOLOGIES LTD
Zavora ERA — Core ERP Engine

Technical Specification v0.2 — Full Product Scope
June 2026 · Nairobi, Kenya


Author	James Karanja, Zavora Technologies Ltd
Version	0.2 — revised from 0.1 after Wave Apps parity audit
Status	Draft — Internal Review
Classification	Confidential
Related specs	Zavora ERA Agentic Layer; AWP Protocol v0.4
 
1. Purpose and scope
This document specifies the complete Zavora ERA core ERP engine and UI-facing product. Version 0.2 supersedes v0.1 following a Wave Apps parity audit that identified 20 missing feature areas. The engine must now serve two equal consumers: (1) the standalone user interface — a non-accountant using it like Wave Apps — and (2) the ADK-Rust agentic layer that sits above it. Neither consumer is more important than the other.
The governing product principle: every feature available in Wave Apps Starter/Pro must exist in Zavora ERA, plus Kenya-specific additions (KRA iTax, M-Pesa, NSSF/NHIF, PAYE) that Wave does not provide.

v0.2 additions vs v0.1:  Customer & Vendor entities · Products & Services catalog · Estimates & Quotes · Recurring invoices · Invoice branding & templates · Payment links (M-Pesa, card) · Auto payment reminders · Customer statements · Credit notes · Partial payments & deposits · Receipt scanning / OCR · Transaction categorisation queue · Split & merge transactions · Payroll engine (Kenya) · User roles & RBAC · Notification system · Dashboard summary API · Settings persistence · Document sequences · Report export (PDF/CSV)

 
2. Wave Apps feature parity matrix
Every row marked 'In scope' is a delivery commitment for v1. 'v2' items are explicitly deferred.

Feature area	Wave Apps	Zavora ERA v1	Zavora advantage
Dashboard — financial overview	Yes	Yes	Agent-narrated insights panel
Invoicing — create & send	Yes	Yes	WhatsApp + M-Pesa payment link
Invoicing — customise (logo, colour, template)	Yes	Yes	Per-entity branding, PDF preview
Invoicing — numbering sequences	Yes	Yes	Configurable prefix + start number
Estimates / Quotes	Yes	Yes	Convert estimate → invoice one-click
Recurring invoices & auto-reminders	Yes	Yes	Cron-based, configurable schedule
Invoice status tracking (sent/viewed/paid)	Yes	Yes	Webhook delivery receipts
Payment links — card	Yes (Stripe/card)	Yes	Card via Flutterwave
Payment links — bank	Yes (ACH)	Yes	M-Pesa Daraja, bank transfer
Online payment recording (auto)	Yes	Yes	M-Pesa webhook auto-reconcile
Customer statements	Yes	Yes	PDF + email + WhatsApp
Credit notes & refunds (AR)	Yes	Yes	Full reversal linkage in GL
Partial payments & deposits	Yes	Yes	Unapplied payments ledger
Products & services catalog	Yes	Yes	With default VAT, account, price
Customer management	Yes	Yes	KRA PIN, credit limit, WHT flag
Vendor / supplier management	Yes	Yes	WHT category, PIN, payment terms
Bills (AP) — create & approve	Yes	Yes	With delegation-of-authority policy
Credit notes (AP)	Yes	Yes	Supplier credit note, DR creditor
Receipt scanning — OCR	Yes (add-on)	Yes	Azure AI Content Understanding
Expense tracking	Yes	Yes	Linked to GL and supplier
Bank account feeds — auto-import	Yes (Pro)	Yes	KCB, Equity, NCBA, M-Pesa
Transaction categorisation queue	Yes	Yes	With AI suggestion engine
Split transactions	Yes	Yes	Split one line → multiple GL codes
Merge transactions	Yes	Yes	Combine duplicates
Bank reconciliation	Yes	Yes	Three-pass auto-match
Chart of accounts — custom	Yes	Yes	Kenya standard COA template
Manual journal entries	Yes	Yes	Full debit/credit form
Payroll — employees	Yes (US/CA only)	Yes	Kenya PAYE, NSSF, NHIF, HELB
Payroll — contractors / 1099	Yes	Yes	WHT certificate (P10/P10A)
Payroll — leave tracking	Yes	Yes	Annual, sick, maternity
Profit & loss report	Yes	Yes	Comparative, by period
Balance sheet	Yes	Yes	Comparative, multi-currency
Cash flow statement	Yes	Yes	Indirect method
Trial balance	Yes	Yes	Opening + movement + closing
General ledger detail	Yes	Yes	Paginated, filterable
AR ageing	Yes	Yes	5 buckets, per customer
AP ageing	Yes	Yes	5 buckets, per vendor
Sales tax / VAT report	Yes	Yes	iTax-ready export
Customer payment history report	Yes	Yes	—
Report export — PDF	Yes	Yes	Branded, printable
Report export — CSV/Excel	Yes	Yes	—
Multi-user access (roles)	Yes	Yes	Owner/Admin/Editor/Viewer/Accountant
Accountant access (external)	Yes	Yes	Read-only + journal-entry role
Audit trail (UI-visible)	Partial	Yes	Full per-record event log
Multi-currency	Yes	Yes	CBK rates auto-loaded
Mobile app	Yes	v2	Planned post-launch
Inventory tracking	No	Yes	FIFO/WAC, warehouse
Fixed assets & depreciation	No	Yes	KRA tax classes
Kenya PAYE / NSSF / NHIF compliance	No	Yes	Zavora-only feature
KRA iTax VAT filing prep	No	Yes	Zavora-only feature
M-Pesa / Daraja payment integration	No	Yes	Zavora-only feature
Agentic posting (natural language)	No	Yes	Zavora-only feature

 
3. Engine architecture
3.1  Crate layout
The core engine is a Rust library crate zavora-erp-core. It has no async runtime dependency — all public methods are async and the caller provides the runtime context. The boundary between engine and UI is a JSON REST API (zavora-erp-api) that the web frontend and mobile app consume. The agentic layer also calls this API via typed MCP tools.

zavora-erp-core/
  src/
    lib.rs             — public API surface (re-exports only)
    engine.rs          — ErpEngine coordinator
    ledger/            — CoA, Journal, GL
    parties/           — Customer, Vendor, Employee entities
    catalog/           — Products & Services
    invoicing/         — Invoices, Estimates, Recurring, Credit Notes
    ap/                — Bills, Supplier Credit Notes
    payments/          — Online payments, M-Pesa, receipts, partial pay
    transactions/      — Categorisation queue, split, merge
    bank/              — Bank feeds, reconciliation
    payroll/           — Employees, pay runs, Kenya statutory
    period/            — Fiscal periods
    tax/               — VAT, WHT, PAYE, NSSF, NHIF
    fx/                — Exchange rates, revaluation
    assets/            — Fixed assets, depreciation
    inventory/         — Stock, FIFO/WAC
    reporting/         — All report types, export
    notifications/     — Reminders, webhooks, push
    documents/         — Attachments, templates, branding
    rbac/              — Users, roles, permissions
    settings/          — Entity config, sequences, branding
    audit/             — AuditEvent, Redis stream
    error.rs           — ErpError enum

3.2  ErpEngine
pub struct ErpEngine {
    pool:   PgPool,
    redis:  MultiplexedConnection,
    config: ErpConfig,          // loaded from settings table
    storage: ObjectStorageClient, // for attachments, PDFs
}

pub struct ErpConfig {
    pub entity_id:       Uuid,
    pub base_currency:   CurrencyCode,
    pub fiscal_year_end: MonthDay,
    pub coa_template:    CoaTemplate,
    pub branding:        BrandingConfig,
    pub sequences:       DocumentSequences,
    pub tax_config:      TaxConfig,
    pub payment_config:  PaymentConfig,
}

 
4. Chart of accounts
4.1  Account model
pub struct Account {
    pub id:           Uuid,
    pub entity_id:    Uuid,
    pub code:         AccountCode,
    pub name:         String,
    pub account_type: AccountType,
    pub parent_code:  Option<AccountCode>,
    pub currency:     Option<CurrencyCode>,
    pub is_control:   bool,
    pub is_active:    bool,
    pub tags:         Vec<String>,
    pub created_at:   DateTime<Utc>,
}
pub enum AccountType {
    Asset, Liability, Equity, Revenue, Expense,
    ContraAsset, ContraLiability, ContraRevenue, ContraExpense,
}

4.2  Kenya standard CoA — top-level segments
Code range	Classification	Normal balance	Statement
1000–1999	Current assets (cash, AR, VAT input, inventory)	Debit	Balance Sheet
2000–2499	Non-current assets	Debit	Balance Sheet
2500–2999	Fixed assets & accumulated depreciation	Debit / Credit	Balance Sheet
3000–3999	Current liabilities (AP, VAT output, WHT, PAYE)	Credit	Balance Sheet
4000–4499	Non-current liabilities	Credit	Balance Sheet
4500–4999	Equity & retained earnings	Credit	Balance Sheet
5000–5999	Revenue	Credit	P&L
6000–6999	Cost of goods sold	Debit	P&L
7000–7999	Operating expenses (rent, salaries, utilities)	Debit	P&L
8000–8499	Finance income / expense, FX gain/loss	Debit / Credit	P&L
8500–8999	Tax expense (CIT, deferred)	Debit	P&L
9000–9999	Control, clearing, suspense accounts	Varies	Off-statement

 
5. Journal engine
5.1  JournalEntry
pub struct JournalEntry {
    pub id:          Uuid,
    pub entity_id:   Uuid,
    pub number:      JournalNumber,
    pub date:        NaiveDate,
    pub period_id:   Uuid,
    pub source:      JournalSource,
    pub reference:   String,
    pub description: String,
    pub lines:       Vec<JournalLine>,
    pub status:      EntryStatus,   // Draft | Posted | Reversed
    pub created_by:  AgentOrUserId,
    pub created_at:  DateTime<Utc>,
    pub posted_at:   Option<DateTime<Utc>>,
}
pub struct JournalLine {
    pub id:               Uuid,
    pub account_code:     AccountCode,
    pub debit:            Option<Decimal>,
    pub credit:           Option<Decimal>,
    pub currency:         CurrencyCode,
    pub fx_rate:          Decimal,
    pub functional_debit: Option<Decimal>,
    pub functional_credit:Option<Decimal>,
    pub description:      Option<String>,
    pub dimensions:       HashMap<String, String>,
}

5.2  Posting rules
Rule	Error if violated
Sum of functional debits = sum of functional credits	ErpError::Unbalanced
Entry date falls in an open fiscal period	ErpError::PeriodClosed
All account codes exist and are active	ErpError::AccountNotFound
Reference is unique per entity	ErpError::DuplicateReference
Control accounts may not be directly posted	ErpError::ValidationFailed
FX rate must exist for non-base-currency lines	ErpError::FxRateNotFound

 
6. Parties — customers and vendors
6.1  Customer
A customer is a party to whom the entity issues invoices. Customer data drives AR control, invoice defaults, reminder schedules, and the customer payment portal link. Every customer record stores a KRA PIN for WHT certificate generation and iTax reporting.
pub struct Customer {
    pub id:               Uuid,
    pub entity_id:        Uuid,
    pub name:             String,
    pub kra_pin:          Option<String>,
    pub vat_number:       Option<String>,
    pub email:            Vec<ContactEmail>,
    pub phone:            Vec<ContactPhone>,   // for WhatsApp & SMS
    pub address:          Option<Address>,
    pub currency:         CurrencyCode,        // default invoice currency
    pub payment_terms:    PaymentTerms,        // Net30, Net14, DueOnReceipt
    pub credit_limit:     Option<Decimal>,
    pub ar_account:       AccountCode,         // default: 1200
    pub reminder_policy:  ReminderPolicy,
    pub portal_enabled:   bool,
    pub notes:            Option<String>,
    pub is_active:        bool,
    pub created_at:       DateTime<Utc>,
}

6.2  Vendor
A vendor is a party from whom the entity receives bills. The WHT category on the vendor record is used to automatically compute withholding tax at bill posting time. No manual WHT calculation is required from the user.
pub struct Vendor {
    pub id:               Uuid,
    pub entity_id:        Uuid,
    pub name:             String,
    pub kra_pin:          Option<String>,
    pub vat_number:       Option<String>,
    pub email:            Vec<ContactEmail>,
    pub phone:            Vec<ContactPhone>,
    pub address:          Option<Address>,
    pub currency:         CurrencyCode,
    pub payment_terms:    PaymentTerms,
    pub wht_category:     Option<WhtCategory>,  // auto-applies on bill post
    pub resident:         bool,                 // determines WHT rate
    pub ap_account:       AccountCode,          // default: 3010
    pub default_expense_account: Option<AccountCode>,
    pub bank_details:     Option<BankDetails>,  // for payment runs
    pub notes:            Option<String>,
    pub is_active:        bool,
}

 
7. Products and services catalog
The catalog stores reusable line-item definitions for both invoicing and billing. When a product is selected on an invoice line, the system auto-fills the description, unit price, default account, and VAT treatment — the user only needs to adjust quantity. This is essential for non-accountant users who should never need to know which GL account maps to consulting services.
pub struct Product {
    pub id:               Uuid,
    pub entity_id:        Uuid,
    pub name:             String,
    pub description:      Option<String>,
    pub product_type:     ProductType,   // Service | Goods | Expense
    pub unit_price:       Option<Decimal>,
    pub currency:         CurrencyCode,
    pub uom:              UnitOfMeasure, // Each, Hour, Day, Kg, Litre
    pub sales_account:    AccountCode,   // for invoice lines
    pub purchase_account: AccountCode,   // for bill lines
    pub vat_treatment:    VatTreatment,  // Standard16 | ZeroRated | Exempt
    pub track_inventory:  bool,
    pub inventory_item_id:Option<Uuid>,  // links to inventory module
    pub is_active:        bool,
}

7.1  Account auto-resolution
Scenario	account_code source priority
Invoice line — product selected	Product.sales_account
Invoice line — no product, account typed	User input
Invoice line — neither	Entity default sales account (settings)
Bill line — product selected	Product.purchase_account
Bill line — no product, account typed	User input
Bill line — neither	Entity default expense account (settings)

 
8. Invoicing — AR documents
8.1  Document types
Type	Description	GL impact
Estimate / Quote	Pre-invoice offer to customer. No GL impact.	None
Invoice	Demand for payment. Posts to AR + Revenue + VAT Output.	DR AR / CR Revenue / CR VAT Output
Recurring invoice	Template that auto-generates invoices on a schedule.	On each generated invoice
Credit note	Reduces or cancels a prior invoice. Reverses GL.	DR Revenue / DR VAT Output / CR AR
Payment receipt	Acknowledgement sent when payment is received.	None (informational)
Customer statement	Aggregated activity for a customer over a period.	None (informational)

8.2  Invoice data model
pub struct Invoice {
    pub id:               Uuid,
    pub entity_id:        Uuid,
    pub number:           String,          // e.g. INV-2026-042
    pub invoice_type:     InvoiceType,     // Invoice | CreditNote
    pub customer_id:      Uuid,
    pub issue_date:       NaiveDate,
    pub due_date:         NaiveDate,
    pub currency:         CurrencyCode,
    pub fx_rate:          Decimal,
    pub lines:            Vec<InvoiceLine>,
    pub tax_lines:        Vec<TaxLine>,
    pub subtotal:         Decimal,
    pub discount_total:   Decimal,
    pub tax_total:        Decimal,
    pub gross_total:      Decimal,
    pub amount_paid:      Decimal,
    pub balance_due:      Decimal,
    pub status:           InvoiceStatus,
    pub source_estimate:  Option<Uuid>,    // if converted from estimate
    pub credit_note_for:  Option<Uuid>,    // if credit note for invoice
    pub journal_entry_id: Option<Uuid>,
    pub sent_at:          Option<DateTime<Utc>>,
    pub viewed_at:        Option<DateTime<Utc>>,  // payment portal
    pub paid_at:          Option<DateTime<Utc>>,
    pub template_id:      Uuid,
    pub notes:            Option<String>,
    pub attachments:      Vec<AttachmentRef>,
}
pub enum InvoiceStatus {
    Draft, Sent, Viewed, PartiallyPaid, Paid, Overdue, Voided
}

8.3  Estimates
pub struct Estimate {
    pub id:           Uuid,
    pub entity_id:    Uuid,
    pub number:       String,         // e.g. EST-2026-018
    pub customer_id:  Uuid,
    pub issue_date:   NaiveDate,
    pub expiry_date:  NaiveDate,
    pub lines:        Vec<InvoiceLine>,
    pub tax_lines:    Vec<TaxLine>,
    pub gross_total:  Decimal,
    pub status:       EstimateStatus, // Draft | Sent | Accepted | Declined | Expired | Converted
    pub converted_to: Option<Uuid>,   // invoice_id if converted
}

8.4  Recurring invoices
pub struct RecurringInvoice {
    pub id:            Uuid,
    pub entity_id:     Uuid,
    pub template:      Invoice,         // base invoice (draft)
    pub frequency:     RecurrenceFreq,  // Weekly | Monthly | Quarterly | Annual
    pub start_date:    NaiveDate,
    pub end_date:      Option<NaiveDate>,
    pub next_run:      NaiveDate,
    pub auto_send:     bool,
    pub auto_charge:   bool,            // auto-charge saved payment method
    pub last_run:      Option<NaiveDate>,
    pub run_count:     u32,
    pub is_active:     bool,
}

8.5  Invoice sending and delivery
Channel	Trigger	Tracking
Email	send_invoice(id, Email) or auto on status → Sent	viewed_at set when payment portal opened
WhatsApp	send_invoice(id, WhatsApp) — WhatsApp Business API MCP	Delivery receipt via webhook
SMS	send_invoice(id, Sms) for minimal-data recipients	Delivery receipt via Africa's Talking MCP
M-Pesa payment link	Embedded in email/WhatsApp — Daraja STK Push link	paid_at set on Daraja callback
Card payment link	Embedded via Flutterwave payment page link	paid_at set on Flutterwave webhook
PDF download	Customer downloads from payment portal	viewed_at set

8.6  Automatic payment reminders
pub struct ReminderPolicy {
    pub reminders: Vec<ReminderRule>,
}
pub struct ReminderRule {
    pub offset_days:   i32,    // negative = before due, positive = after
    pub channels:      Vec<Channel>,
    pub template_id:   Uuid,
}
Default policy: reminder at -3 days (before due), +1 day, +7 days, +14 days. The notification scheduler is a background task that runs hourly and emits reminder jobs to a Redis queue consumed by the notification worker.

8.7  Customer statements
pub async fn customer_statement(
    &self,
    customer_id: Uuid,
    period_from: NaiveDate,
    period_to:   NaiveDate,
    format:      OutputFormat,  // Pdf | Json
) -> Result<StatementOutput, ErpError>;
A customer statement lists all invoices, credit notes, and payments in the period with opening balance, closing balance, and amount due. It is sent on demand or on a scheduled basis (e.g. first of each month).

8.8  Invoice branding and templates
pub struct InvoiceTemplate {
    pub id:            Uuid,
    pub entity_id:     Uuid,
    pub name:          String,
    pub logo_url:      Option<String>,
    pub primary_color: String,       // hex
    pub font:          String,
    pub footer_text:   Option<String>,
    pub show_bank_details: bool,
    pub show_mpesa_paybill: bool,
    pub layout:        TemplateLayout, // Classic | Modern | Minimal
    pub is_default:    bool,
}

 
9. Accounts payable — bills and supplier credit notes
9.1  Bill lifecycle
State	Description	GL impact
Draft	Captured from OCR/manual. Editable.	None
Pending approval	Submitted. Locked.	None
Approved	Authorised per DoA policy.	None
Posted	GL journal created. AP balance updated.	DR Expense / CR AP / DR VAT Input / CR WHT Payable
Partially paid	One or more payments applied.	DR AP / CR Bank
Paid	Balance = zero.	DR AP / CR Bank (final payment)
Disputed	Flagged — payment blocked.	No change
Cancelled / Credit note	Reversed via supplier credit note.	Reverses original post lines

9.2  Supplier credit note
pub struct SupplierCreditNote {
    pub id:                 Uuid,
    pub entity_id:          Uuid,
    pub vendor_id:          Uuid,
    pub credit_note_number: String,
    pub credit_note_date:   NaiveDate,
    pub applies_to_bill:    Option<Uuid>,
    pub lines:              Vec<InvoiceLine>,
    pub tax_lines:          Vec<TaxLine>,
    pub gross_total:        Decimal,
    pub status:             ApDocStatus,
    pub journal_entry_id:   Option<Uuid>,
}

 
10. Payments, receipts, and online payment processing
10.1  Payment model
A Payment record represents money received (AR payment) or money sent (AP payment). Payments are applied to invoices/bills to reduce their balance. Unapplied payments are held in a clearing account (9100 Unapplied Customer Payments / 9110 Unapplied Vendor Credits) until matched.
pub struct Payment {
    pub id:              Uuid,
    pub entity_id:       Uuid,
    pub payment_type:    PaymentType,  // CustomerPayment | VendorPayment
    pub party_id:        Uuid,         // customer_id or vendor_id
    pub payment_date:    NaiveDate,
    pub amount:          Decimal,
    pub currency:        CurrencyCode,
    pub fx_rate:         Decimal,
    pub method:          PaymentMethod,
    pub reference:       String,
    pub bank_account_id: Uuid,
    pub applications:    Vec<PaymentApplication>,
    pub unapplied:       Decimal,
    pub journal_entry_id:Uuid,
    pub status:          PaymentStatus,
}
pub struct PaymentApplication {
    pub document_id:    Uuid,   // invoice_id or bill_id
    pub amount_applied: Decimal,
}
pub enum PaymentMethod {
    Mpesa { transaction_id: String, phone: String },
    BankTransfer { reference: String },
    Card { processor: String, authorization: String },
    Cash,
    Cheque { number: String },
}

10.2  M-Pesa / Daraja integration
When a customer clicks a payment link on an invoice, the Daraja MCP server initiates an STK Push to the customer's phone. On successful callback, the engine calls record_mpesa_payment() which creates a Payment record and applies it to the invoice automatically. No manual reconciliation is needed for M-Pesa payments.
pub async fn record_mpesa_payment(
    &self,
    invoice_id: Uuid,
    mpesa_callback: MpesaCallback,
) -> Result<Payment, ErpError>;

10.3  Receipt capture — OCR expense entry
A receipt capture flow allows non-accountants to photograph a supplier receipt. The OCR MCP (Azure AI Content Understanding) extracts vendor name, date, amount, and VAT. The engine creates a Draft Bill pre-filled with extracted data. The user reviews and approves; no accounting knowledge is needed.
pub struct ReceiptCapture {
    pub id:              Uuid,
    pub entity_id:       Uuid,
    pub image_url:       String,
    pub ocr_result:      OcrResult,
    pub suggested_bill:  Option<BillDraft>,  // pre-filled from OCR
    pub status:          CaptureStatus,  // Pending | Reviewed | Posted | Rejected
    pub captured_by:     AgentOrUserId,
    pub captured_at:     DateTime<Utc>,
}
pub struct OcrResult {
    pub vendor_name:   Option<String>,
    pub vendor_pin:    Option<String>,
    pub date:          Option<NaiveDate>,
    pub total:         Option<Decimal>,
    pub vat_amount:    Option<Decimal>,
    pub line_items:    Vec<OcrLineItem>,
    pub confidence:    f32,
}

 
11. Transactions and categorisation queue
When bank statement lines are imported (auto-feed or manual CSV), they enter a Categorisation Queue before becoming journal entries. This is distinct from formal bank reconciliation. Categorisation is the act of assigning a GL account to a raw bank line. Reconciliation is the act of matching a GL entry against a bank line.
11.1  ImportedTransaction
pub struct ImportedTransaction {
    pub id:            Uuid,
    pub entity_id:     Uuid,
    pub bank_account:  Uuid,
    pub value_date:    NaiveDate,
    pub description:   String,
    pub reference:     String,
    pub debit:         Option<Decimal>,
    pub credit:        Option<Decimal>,
    pub running_bal:   Decimal,
    pub category_status: CategoryStatus,
    pub assigned_account: Option<AccountCode>,
    pub split_parts:   Vec<TransactionSplit>,   // for split transactions
    pub merged_into:   Option<Uuid>,            // if merged with another
    pub journal_entry_id: Option<Uuid>,         // once posted
    pub suggestion:    Option<AccountSuggestion>,  // AI-suggested
}
pub enum CategoryStatus {
    Uncategorised,
    Suggested,     // AI has a suggestion, awaiting confirmation
    Categorised,   // account assigned, not yet posted
    Posted,        // journal entry created
    Excluded,      // marked as non-business (personal, duplicate)
}

11.2  Split transactions
A single imported bank debit of KES 50,000 might cover KES 35,000 office rent and KES 15,000 insurance premium — two different GL accounts. The split transaction feature divides one imported line into multiple categorised parts, each with its own account and amount.
pub async fn split_transaction(
    &self,
    transaction_id: Uuid,
    parts: Vec<SplitPart>,
    split_by: AgentOrUserId,
) -> Result<Vec<ImportedTransaction>, ErpError>;

pub struct SplitPart {
    pub amount:       Decimal,
    pub account_code: AccountCode,
    pub description:  String,
}

11.3  Merge transactions
Duplicate imports (e.g. the same transaction appearing in both an auto-feed and a manually imported CSV) are resolved by merging. The merge designates a primary record and archives the duplicate, then adjusts the running balances.
pub async fn merge_transactions(
    &self,
    primary_id: Uuid,
    duplicate_ids: Vec<Uuid>,
    merged_by: AgentOrUserId,
) -> Result<ImportedTransaction, ErpError>;

 
12. Bank reconciliation
12.1  Statement import
The bank reconciliation module accepts MT940, OFX, or CSV statement imports, or receives live transactions via the Bank MCP server (KCB, Equity, NCBA, M-Pesa). Imported lines begin as ImportedTransactions in the Categorisation Queue before entering the reconciliation match process.
12.2  Matching algorithm (three-pass)
1.	Exact match — amount + date + reference string equality against posted GL entries for the same bank account.
2.	Near match — amount equality, two-day date window, reference fuzzy-match score > 0.85.
3.	AI suggestion — description embedding similarity to prior categorisations, surfaced as a Suggested status in the queue.

12.3  API
pub async fn match_bank_lines(&self, statement_id: Uuid) -> Result<MatchReport, ErpError>;
pub async fn confirm_match(&self, stmt_line_id: Uuid, journal_entry_id: Uuid, confirmed_by: AgentOrUserId) -> Result<(), ErpError>;
pub async fn post_unmatched_line(&self, stmt_line_id: Uuid, account_code: AccountCode, description: String, posted_by: AgentOrUserId) -> Result<JournalEntry, ErpError>;

 
13. Payroll — Kenya
Zavora ERA includes a full Kenya payroll engine covering PAYE, NSSF, NHIF (SHA), HELB deductions, and payslip generation. This is not available in Wave Apps and is a primary Zavora differentiator for SMEs with employees.
13.1  Employee
pub struct Employee {
    pub id:              Uuid,
    pub entity_id:       Uuid,
    pub staff_number:    String,
    pub full_name:       String,
    pub kra_pin:         String,
    pub nssf_number:     Option<String>,
    pub nhif_number:     Option<String>,
    pub helb_deduction:  Option<Decimal>,    // monthly HELB deduction
    pub employment_type: EmploymentType,     // Permanent | Contract | Casual
    pub basic_salary:    Decimal,
    pub allowances:      Vec<Allowance>,
    pub bank_account:    BankDetails,
    pub tax_relief:      Decimal,            // personal relief KES 2,400/month
    pub start_date:      NaiveDate,
    pub end_date:        Option<NaiveDate>,
}

13.2  Kenya statutory deductions (2026 rates)
Deduction	Rate / amount	Employer contribution	GL accounts
PAYE	Progressive bands per KRA Finance Act 2024	None	DR Salaries / CR PAYE Payable (3310)
NSSF (Tier I)	6% of gross up to KES 7,000	6% matching	DR Salaries+NSSF Cost / CR NSSF Payable (3320)
NSSF (Tier II)	6% of gross KES 7,001–36,000	6% matching	As above
SHA (NHIF replacement)	2.75% of gross	None — employee only	DR Salaries / CR SHA Payable (3330)
HELB	Per employee agreement (KES 1,500–10,000)	None	DR Salaries / CR HELB Payable (3340)
Housing Levy	1.5% of gross	1.5% matching	DR Salaries+HL Cost / CR Housing Levy Payable (3350)

13.3  Pay run
pub struct PayRun {
    pub id:           Uuid,
    pub entity_id:    Uuid,
    pub period_id:    Uuid,
    pub pay_date:     NaiveDate,
    pub payslips:     Vec<Payslip>,
    pub total_gross:  Decimal,
    pub total_paye:   Decimal,
    pub total_nssf:   Decimal,
    pub total_sha:    Decimal,
    pub total_net:    Decimal,
    pub status:       PayRunStatus,  // Draft | Approved | Posted | Paid
    pub journal_entry_id: Option<Uuid>,
}
pub async fn run_payroll(&self, period_id: Uuid, run_by: AgentOrUserId) -> Result<PayRun, ErpError>;
pub async fn approve_pay_run(&self, pay_run_id: Uuid, approved_by: AgentOrUserId) -> Result<PayRun, ErpError>;
pub async fn post_pay_run(&self, pay_run_id: Uuid, posted_by: AgentOrUserId) -> Result<JournalEntry, ErpError>;

 
14. User management and access control
14.1  Roles
Role	Description	Can post?	Can approve?	Can close periods?
Owner	Full access. Billing, user management, all data.	Yes	Yes	Yes
Admin	Full accounting access. Cannot manage billing.	Yes	Yes	Yes
Accountant (external)	All accounting read + manual journal entry. No settings.	Yes	No	No
Editor	Create invoices, bills, capture receipts. No period close.	No (draft only)	No	No
Approver	Approve bills and pay runs. Read-only otherwise.	No	Yes	No
Viewer	Read-only. All reports and records visible.	No	No	No

14.2  User model
pub struct EraUser {
    pub id:          Uuid,
    pub entity_id:   Uuid,
    pub email:       String,
    pub display_name:String,
    pub role:        UserRole,
    pub is_active:   bool,
    pub invited_by:  Uuid,
    pub last_login:  Option<DateTime<Utc>>,
}

 
15. Notification and reminder system
All time-sensitive actions — invoice reminders, payment received, bill approval requests, payroll approval, period close warnings — are handled by the notification engine. Channels: Email (SMTP/SendGrid), WhatsApp (WhatsApp Business API), SMS (Africa's Talking), and in-app.
15.1  NotificationEvent types
Event	Trigger	Recipients	Default channel
invoice_reminder	Cron: reminder_policy days before/after due	Customer + entity owner	Email + WhatsApp
invoice_paid	M-Pesa/card webhook received	Entity owner + editor who raised invoice	Email + in-app
bill_approval_needed	Bill submitted for approval	Approvers	Email + in-app
bill_overdue	Cron: bill past due date unpaid	Owner + admin	Email
payrun_approval_needed	Pay run drafted	Owner + approvers	Email + in-app
period_close_warning	Cron: 3 days before month end	Owner + accountants	Email
bank_feed_error	Bank MCP sync fails	Owner + admin	Email + in-app
receipt_processed	OCR capture complete	User who captured	In-app

 
16. Settings and configuration
All settings are persisted in the entity_settings table and are mutable at runtime through the settings API. The ErpConfig struct is refreshed from this table at each engine initialisation and can be hot-reloaded without restart.
16.1  Document sequences
pub struct DocumentSequences {
    pub invoice_prefix:    String,   // e.g. "INV"
    pub invoice_next:      u64,      // next number, e.g. 43
    pub estimate_prefix:   String,
    pub estimate_next:     u64,
    pub credit_note_prefix:String,
    pub credit_note_next:  u64,
    pub bill_prefix:       String,
    pub bill_next:         u64,
    pub journal_prefix:    String,
    pub journal_next:      u64,
    pub year_reset:        bool,     // reset counter on new fiscal year
}

16.2  Tax configuration
pub struct TaxConfig {
    pub vat_registered:     bool,
    pub vat_number:         Option<String>,
    pub vat_period:         VatPeriod,   // Monthly | Quarterly
    pub standard_vat_rate:  Decimal,     // 0.16
    pub default_vat_treatment: VatTreatment,
    pub wht_enabled:        bool,
    pub paye_enabled:       bool,
}

16.3  Settings API
pub async fn get_settings(&self) -> Result<ErpConfig, ErpError>;
pub async fn update_settings(&self, patch: SettingsPatch, updated_by: AgentOrUserId) -> Result<ErpConfig, ErpError>;
pub async fn get_sequences(&self) -> Result<DocumentSequences, ErpError>;
pub async fn update_sequence(&self, seq_type: SeqType, patch: SeqPatch) -> Result<DocumentSequences, ErpError>;

 
17. Dashboard summary API
The dashboard API returns all data needed for the home screen in a single call, avoiding N+1 queries from the UI. The summary is computed over the current open period and the last 6 months.
pub struct DashboardSummary {
    pub as_at:              DateTime<Utc>,
    pub total_receivable:   Decimal,
    pub overdue_receivable: Decimal,
    pub overdue_invoice_count: u32,
    pub total_payable:      Decimal,
    pub overdue_payable:    Decimal,
    pub overdue_bill_count: u32,
    pub cash_and_bank:      Decimal,
    pub net_income_mtd:     Decimal,
    pub net_income_prior:   Decimal,
    pub revenue_6m:         Vec<MonthlyAmount>,
    pub expenses_6m:        Vec<MonthlyAmount>,
    pub recent_transactions:Vec<TransactionSummary>,
    pub outstanding_invoices:Vec<InvoiceSummary>,
    pub pending_approvals:  u32,
    pub uncategorised_txns: u32,
}
pub async fn dashboard_summary(&self, entity_id: Uuid) -> Result<DashboardSummary, ErpError>;

 
18. Financial reporting
18.1  Report types
Report	Method	Key parameters
Trial balance	trial_balance()	entity_id, as_at, compare_to
Balance sheet	balance_sheet()	entity_id, as_at, comparative
Profit & loss	profit_and_loss()	entity_id, period_from, period_to, comparative
Cash flow (indirect)	cash_flow_statement()	entity_id, period_from, period_to
AR ageing	ar_ageing_report()	entity_id, as_at, customer filter
AP ageing	ap_ageing_report()	entity_id, as_at, vendor filter
VAT return data	vat_return_data()	entity_id, period_id
GL detail	gl_detail()	account_code, date_from, date_to
Customer statement	customer_statement()	customer_id, period_from, period_to
Customer payment history	customer_payment_history()	customer_id, date_from, date_to
Bank recon summary	bank_recon_summary()	bank_account_id, statement_id
Payroll summary	payroll_summary()	period_id
PAYE schedule (P10)	paye_p10()	period_id
WHT certificate (P10A)	wht_certificate()	period_id, vendor_id
Sales tax summary	sales_tax_summary()	entity_id, period_from, period_to

18.2  Export
pub async fn export_report(
    &self,
    report: ReportData,
    format: ExportFormat,  // Pdf | Csv | Xlsx | Json
    template_id: Option<Uuid>,
) -> Result<ExportOutput, ErpError>;

 
19. Fiscal periods
State	Description	Who may trigger	Reversible?
Future	Not yet open.	System (auto)	No
Open	Transactions may be posted.	System (auto on period start)	N/A
Soft-closed	Prior-period adjustments allowed.	Finance lead / Compliance Agent	Yes → re-open
Hard-closed	Immutable. DB trigger enforces.	Finance lead (dual approval)	Never
DB-level guarantee:  The hard-closed period is enforced by a Postgres trigger on journal_lines INSERT — not just application code. No bypass is possible via migration scripts or direct DB access.

 
20. Multi-currency and FX revaluation
20.1  Exchange rates
pub struct ExchangeRate {
    pub from_ccy:   CurrencyCode,
    pub to_ccy:     CurrencyCode,
    pub rate_date:  NaiveDate,
    pub rate_type:  RateType,   // Spot | Revaluation | Budget
    pub rate:       Decimal,
    pub source:     String,     // CBK | manual | provider
}

20.2  FX revaluation
Period-end revaluation posts unrealised FX gain/loss to accounts 8100/8110 with an automatic reversal dated the first day of the next period.
pub async fn run_fx_revaluation(&self, period_id: Uuid, rate_date: NaiveDate, triggered_by: AgentOrUserId) -> Result<FxRevaluationReport, ErpError>;

 
21. Kenya tax compliance
21.1  VAT rates
Rate	Description	Examples
16%	Standard rate	Most goods and services
8%	Petroleum products	Petrol, diesel, kerosene
0%	Zero-rated	Exports, basic foodstuffs
Exempt	Outside scope	Financial services, land

21.2  Withholding tax — key rates
Category	Resident	Non-resident	GL debit / credit
Consultancy / management fees	5%	20%	DR AP / CR WHT Payable 3210
Rent (land/building)	10%	30%	DR AP / CR WHT Payable 3210
Royalties	5%	20%	DR AP / CR WHT Payable 3210
Interest (non-bank)	15%	15%	DR AP / CR WHT Payable 3210
Contractual (construction)	3%	20%	DR AP / CR WHT Payable 3210

 
22. Fixed assets and depreciation
22.1  Asset model
pub struct FixedAsset {
    pub id:                   Uuid,
    pub asset_number:         String,
    pub description:          String,
    pub category:             AssetCategory,
    pub acquisition_date:     NaiveDate,
    pub cost:                 Decimal,
    pub residual_value:       Decimal,
    pub useful_life_months:   u32,
    pub depreciation_method:  DepreciationMethod,
    pub gl_asset_account:     AccountCode,
    pub gl_accum_depr_account:AccountCode,
    pub gl_depr_expense:      AccountCode,
    pub status:               AssetStatus,
}
pub enum DepreciationMethod {
    StraightLine,
    DecliningBalance { rate_percent: Decimal },
    KraTax { class: KraAssetClass },
}

22.2  KRA asset classes
Class	Rate	Method	Examples
Class 1	37.5%	Declining balance	Computers, software
Class 2	30%	Declining balance	Motor vehicles, lorries
Class 3	25%	Declining balance	Machinery, plant
Class 4	12.5%	Declining balance	Buildings (industrial)

 
23. Inventory
pub struct InventoryItem {
    pub id:              Uuid,
    pub sku:             String,
    pub description:     String,
    pub uom:             UnitOfMeasure,
    pub costing_method:  CostingMethod,  // FIFO | WeightedAvgCost
    pub gl_inventory:    AccountCode,
    pub gl_cogs:         AccountCode,
    pub reorder_point:   Option<Decimal>,
}
The engine enforces non-negative stock: an issue that would take on-hand below zero returns ErpError::InsufficientStock. The Posting Agent resolves discrepancies before retrying.

 
24. Document management and attachments
pub struct Attachment {
    pub id:           Uuid,
    pub entity_id:    Uuid,
    pub linked_type:  LinkedType,   // Invoice|Bill|Payment|JournalEntry|Receipt
    pub linked_id:    Uuid,
    pub filename:     String,
    pub mime_type:    String,
    pub storage_key:  String,       // object storage path
    pub size_bytes:   u64,
    pub uploaded_by:  AgentOrUserId,
    pub uploaded_at:  DateTime<Utc>,
}
Attachments are stored in object storage (S3-compatible). Access is controlled by the RBAC layer — Viewers can read attachments on records they can see; Editors can upload; Admins can delete.

 
25. Audit and immutability
25.1  AuditEvent
pub struct AuditEvent {
    pub id:          Uuid,
    pub entity_id:   Uuid,
    pub event_type:  AuditEventType,
    pub object_type: String,
    pub object_id:   Uuid,
    pub actor:       AgentOrUserId,
    pub before:      Option<serde_json::Value>,
    pub after:       Option<serde_json::Value>,
    pub timestamp:   DateTime<Utc>,
}
Emitted to Redis stream erp:audit:{entity_id} inside the same DB transaction. Consumed by audit sink worker, persisted to audit_events table (Postgres) for KDPA 7-year retention. Visible in the UI per-record as an activity timeline.

25.2  Immutability guarantees
Guarantee	Mechanism
Posted journal entries never mutated	Postgres UPDATE trigger on journal_entries where status='posted'
Hard-closed periods reject all new lines	Postgres INSERT trigger on journal_lines checks period status
Audit stream = DB state	Redis XADD inside DB transaction; rollback on Redis failure
Every action attributed to actor	AgentOrUserId non-nullable on all write APIs
Reversals do not mutate originals	New entry created; original status updated to 'reversed' only

 
26. Postgres schema — key tables
26.1  Parties
CREATE TABLE customers ( id UUID PK, entity_id UUID, name TEXT, kra_pin TEXT,
  currency CHAR(3), payment_terms TEXT, credit_limit NUMERIC, ar_account TEXT,
  portal_enabled BOOL, is_active BOOL, created_at TIMESTAMPTZ );

CREATE TABLE vendors ( id UUID PK, entity_id UUID, name TEXT, kra_pin TEXT,
  currency CHAR(3), payment_terms TEXT, wht_category TEXT, resident BOOL,
  ap_account TEXT, is_active BOOL );

26.2  Products
CREATE TABLE products ( id UUID PK, entity_id UUID, name TEXT,
  product_type TEXT, unit_price NUMERIC, uom TEXT,
  sales_account TEXT, purchase_account TEXT, vat_treatment TEXT,
  track_inventory BOOL, inventory_item_id UUID, is_active BOOL );

26.3  Invoicing
CREATE TABLE invoices ( id UUID PK, entity_id UUID, number TEXT UNIQUE,
  invoice_type TEXT, customer_id UUID, issue_date DATE, due_date DATE,
  currency CHAR(3), fx_rate NUMERIC, subtotal NUMERIC, tax_total NUMERIC,
  gross_total NUMERIC, amount_paid NUMERIC, balance_due NUMERIC,
  status TEXT, journal_entry_id UUID, sent_at TIMESTAMPTZ, viewed_at TIMESTAMPTZ,
  paid_at TIMESTAMPTZ, template_id UUID );

CREATE TABLE invoice_lines ( id UUID PK, invoice_id UUID, product_id UUID,
  description TEXT, quantity NUMERIC, unit_price NUMERIC, discount_pct NUMERIC,
  account_code TEXT, vat_treatment TEXT, line_total NUMERIC, vat_amount NUMERIC );

26.4  Transactions
CREATE TABLE imported_transactions (
  id UUID PK, entity_id UUID, bank_account UUID,
  value_date DATE, description TEXT, reference TEXT,
  debit NUMERIC, credit NUMERIC, running_bal NUMERIC,
  category_status TEXT DEFAULT 'uncategorised',
  assigned_account TEXT, journal_entry_id UUID,
  merged_into UUID, import_batch_id UUID );

26.5  Payroll
CREATE TABLE employees ( id UUID PK, entity_id UUID, staff_number TEXT,
  full_name TEXT, kra_pin TEXT, nssf_number TEXT, nhif_number TEXT,
  basic_salary NUMERIC, employment_type TEXT, bank_account JSONB,
  tax_relief NUMERIC, start_date DATE, end_date DATE );

CREATE TABLE pay_runs ( id UUID PK, entity_id UUID, period_id UUID,
  pay_date DATE, total_gross NUMERIC, total_paye NUMERIC, total_nssf NUMERIC,
  total_sha NUMERIC, total_net NUMERIC, status TEXT, journal_entry_id UUID );

26.6  Users & RBAC
CREATE TABLE era_users ( id UUID PK, entity_id UUID, email TEXT,
  display_name TEXT, role TEXT, is_active BOOL,
  invited_by UUID, last_login TIMESTAMPTZ,
  UNIQUE(entity_id, email) );

26.7  Settings
CREATE TABLE entity_settings ( entity_id UUID PK,
  base_currency CHAR(3), fiscal_year_end TEXT, coa_template TEXT,
  branding JSONB, sequences JSONB, tax_config JSONB,
  payment_config JSONB, updated_at TIMESTAMPTZ, updated_by UUID );

 
27. Agentic layer API surface
Agents call post_from_agent() and run_report() exclusively. All other module methods are pub(crate). This boundary is enforced by Rust visibility and cannot be bypassed without modifying the crate source.
pub async fn post_from_agent(&self, req: PostingRequest) -> Result<AgentPostingResult, ErpError>;
pub async fn run_report(&self, req: ReportRequest) -> Result<ReportData, ErpError>;
pub async fn dashboard_summary(&self, entity_id: Uuid) -> Result<DashboardSummary, ErpError>;
pub async fn validate_entry(&self, req: &JournalEntryRequest) -> Result<ValidationReport, ErpError>;

pub struct AgentPostingResult {
    pub entry:                   JournalEntry,
    pub validation_report:       ValidationReport,
    pub natural_language_summary:String,
}

 
28. Implementation milestones — revised
Milestone	Deliverable	Target
M1 — Ledger core	CoA, Journal, Periods, Audit stream, Settings persistence, Document sequences	Week 2
M2 — Parties & Catalog	Customer, Vendor, Employee, Products & Services, RBAC	Week 4
M3 — Invoicing	Invoice, Estimate, Credit Note, Recurring, Reminders, Branding, Payment links	Week 6
M4 — AP & Payments	Bills, Supplier credit notes, Partial payments, Receipt OCR, M-Pesa webhook	Week 8
M5 — Transactions & Banking	Import queue, Categorisation, Split, Merge, Bank feeds, Reconciliation	Week 10
M6 — Payroll	Employee CRUD, Pay run, PAYE/NSSF/SHA/Housing Levy, Payslips, P9/P10	Week 12
M7 — Tax & FX	VAT/WHT compute, FX rates, Period-end revaluation, iTax data export	Week 14
M8 — Assets & Inventory	Fixed assets, KRA depreciation, FIFO/WAC, Warehouse stock	Week 16
M9 — Reporting	All 15 report types, PDF/CSV export, Dashboard summary API	Week 18
M10 — Hardening	DB triggers, integration test suite, adk-bench baseline, security audit	Week 20

29. Open questions
•	Mobile app: Wave has iOS and Android. Zavora ERA v1 is web-only. Mobile targets v2. Receipt scanning on mobile is critical UX — consider PWA as interim.
•	Multi-entity / consolidation: entity_id column is present everywhere, making multi-entity non-breaking. Inter-company elimination and consolidated reporting targeted for v2.
•	POS integration: Wave supports Square/Shopify via Zapier. Zavora ERA v1 has no POS. v2 consideration.
•	Offline posting: field staff using M-Pesa receipts need offline queue. AWP message queue is the candidate protocol.
•	Project costing: dimension tags enable cost-centre reporting. Full WIP/project P&L is a separate module for v2, aligned with Mitchell Cotts Group requirements.
•	KDPA retention: Redis audit stream requires periodic Postgres/S3 sink. 7-year minimum retention for financial records under KDPA.
•	Accountant portal: Wave Advisors offers bookkeeper matching. Zavora could offer a managed bookkeeping tier — deferred until post-launch.
