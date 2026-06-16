# Requirements Document

## Introduction

This specification covers bringing Zavora ERP to full production readiness across all priority tiers (P0 through P3). Zavora ERP is a multi-tenant accounting system targeting Kenya SMEs, built with a Rust backend (Axum framework, `zavora-erp-core` library + `zavora-erp-api` server) and a React/TypeScript frontend (`zavora-erp-ui`). The system uses PostgreSQL 17 for persistence and Redis 7 for caching and async task queues.

The current state has functional business logic for invoicing, bills, payments, payroll, and general ledger — but lacks production-grade authentication, transaction safety, tenant isolation, testing, and operational infrastructure. This spec defines all requirements to reach a deployable, secure, and reliable state.

## Glossary

- **API_Server**: The `zavora-erp-api` Axum application serving JSON REST endpoints
- **Core_Engine**: The `zavora-erp-core` Rust library containing business logic
- **Auth_Module**: The authentication and session management subsystem
- **Tenant_Scope**: The entity_id-based data isolation layer ensuring each organization sees only its own data
- **Ledger_Service**: The journal entry creation, posting, and balance computation subsystem
- **Payment_Service**: The module handling payment recording, application, and M-Pesa integration
- **Document_Sequencer**: The subsystem generating sequential document numbers (invoice, bill, receipt prefixes)
- **Posting_Resolver**: The module determining GL accounts for transactions based on posting group matrices
- **CI_Pipeline**: The GitHub Actions continuous integration workflow
- **Frontend**: The `zavora-erp-ui` React/TypeScript single-page application
- **Daraja_API**: Safaricom's M-Pesa API gateway for STK Push and callback notifications
- **Rounding_Policy**: The defined rule for truncating/rounding monetary values to 2 decimal places

---

## Requirements

### Requirement 1: JWT-Based Authentication

**User Story:** As a system administrator, I want verified authentication with password hashing and JWT tokens, so that user identity is cryptographically proven rather than trusted from client-supplied headers.

#### Acceptance Criteria

1. WHEN a user submits valid credentials to the login endpoint, THE Auth_Module SHALL return a signed JWT access token and a refresh token
2. WHEN an API request includes a valid JWT in the Authorization header, THE Auth_Module SHALL extract user_id, entity_id, and role from the token claims
3. WHEN an API request includes an expired or invalid JWT, THE Auth_Module SHALL reject the request with HTTP 401
4. WHEN a user submits a password during registration or login, THE Auth_Module SHALL hash the password using Argon2id before storage or comparison
5. IF the X-User-Id, X-Entity-Id, or X-User-Role headers are present without a valid JWT, THEN THE Auth_Module SHALL ignore those headers and reject the request
6. WHEN a refresh token is submitted to the token refresh endpoint, THE Auth_Module SHALL issue a new access token if the refresh token is valid and not revoked
7. THE Auth_Module SHALL enforce a configurable access token expiry (default 15 minutes) and refresh token expiry (default 7 days)

---

### Requirement 2: Transaction Atomicity for Ledger-Coupled Flows

**User Story:** As an accountant, I want payment recording, invoice posting, credit notes, and payment application to execute atomically, so that a failure mid-operation cannot leave the ledger in an inconsistent state.

#### Acceptance Criteria

1. WHEN the Payment_Service records a payment, THE Ledger_Service SHALL execute the balance update, journal entry creation, and payment record insertion within a single database transaction
2. WHEN the Ledger_Service posts an invoice, THE Ledger_Service SHALL execute the status update, journal entry creation, and receivables balance adjustment within a single database transaction
3. WHEN the Ledger_Service creates a credit note, THE Ledger_Service SHALL execute the credit note record, journal reversal, and balance adjustment within a single database transaction
4. WHEN the Payment_Service applies an unapplied payment, THE Ledger_Service SHALL execute the allocation record, balance transfer, and journal entry within a single database transaction
5. IF any step within an atomic operation fails, THEN THE Ledger_Service SHALL roll back all changes from that operation and return a descriptive error
6. FOR ALL journal entries created by atomic operations, THE Ledger_Service SHALL verify that total debits equal total credits before committing the transaction

---

### Requirement 3: Per-Request Tenant Scoping

**User Story:** As a platform operator, I want every database query scoped to the authenticated user's entity_id, so that tenants cannot access each other's data.

#### Acceptance Criteria

1. THE API_Server SHALL include a WHERE entity_id = $authenticated_entity_id clause on every SELECT, UPDATE, and DELETE query
2. WHEN a new record is created, THE Core_Engine SHALL set the entity_id field to the authenticated user's entity_id from the JWT claims
3. IF a request targets a record belonging to a different entity_id, THEN THE API_Server SHALL return HTTP 404 (not 403) to prevent information leakage
4. THE API_Server SHALL remove the startup ENTITY_ID environment variable as the sole query-scoping mechanism and replace it with per-request scoping from the AuthContext

---

### Requirement 4: Automated Test Suite

**User Story:** As a developer, I want comprehensive automated tests covering accounting logic, so that regressions in journal balancing, posting, payroll calculations, and FX operations are caught before deployment.

#### Acceptance Criteria

1. THE Core_Engine SHALL include unit tests verifying that every journal entry created by the posting engine has total debits equal to total credits
2. THE Core_Engine SHALL include integration tests for the payment recording flow covering: single payment, partial payment, overpayment, and multi-currency payment
3. THE Core_Engine SHALL include unit tests for payroll tax calculations covering PAYE brackets, NSSF contributions, NHIF deductions, and housing levy
4. THE Core_Engine SHALL include integration tests for the period close flow verifying that posting to a closed period is rejected
5. THE Core_Engine SHALL include tests for FX revaluation verifying correct gain/loss journal entries against known exchange rates
6. WHEN the test suite executes, THE Core_Engine SHALL achieve a minimum of 80% line coverage on the ledger, payments, and payroll modules

---

### Requirement 5: Monetary Rounding Policy

**User Story:** As an accountant, I want all monetary calculations to apply consistent 2-decimal-place rounding, so that VAT-derived fractional amounts do not block journal posting or create audit discrepancies.

#### Acceptance Criteria

1. THE Core_Engine SHALL round all monetary values to 2 decimal places using banker's rounding (round half to even) before storing or comparing
2. WHEN computing line-level VAT, THE Core_Engine SHALL apply rounding to each line independently before summing
3. WHEN a journal entry has a rounding imbalance of 0.01 KES or less due to VAT line accumulation, THE Ledger_Service SHALL insert a rounding adjustment line to the configured rounding expense/income account
4. THE Core_Engine SHALL store all monetary values in the database as NUMERIC(18,2) or equivalent fixed-precision type

---

### Requirement 6: Document Numbering

**User Story:** As a business owner, I want gapless, year-scoped document numbering for invoices, bills, and receipts, so that sequences comply with KRA requirements and reset correctly at fiscal year boundaries.

#### Acceptance Criteria

1. WHEN a document is created successfully, THE Document_Sequencer SHALL allocate the next sequential number without gaps
2. IF a document creation fails after number allocation, THEN THE Document_Sequencer SHALL release the allocated number back to the sequence (or allocate within the same transaction to prevent gaps)
3. WHEN the fiscal year changes and year_reset is enabled for a sequence, THE Document_Sequencer SHALL reset the counter to the configured start number
4. THE Document_Sequencer SHALL format numbers using the configured prefix, year placeholder, and zero-padded counter (e.g., INV-2026-00042)
5. WHEN two concurrent requests attempt to allocate a number, THE Document_Sequencer SHALL serialize access to prevent duplicate numbers

---

### Requirement 7: CORS Lockdown

**User Story:** As a security engineer, I want CORS restricted to known origins in production, so that unauthorized domains cannot make cross-origin API requests.

#### Acceptance Criteria

1. WHILE the application is running in production mode, THE API_Server SHALL restrict CORS allowed origins to a configurable list of domains
2. WHILE the application is running in development mode, THE API_Server SHALL permit all origins for local development convenience
3. WHEN a request arrives from a non-allowed origin, THE API_Server SHALL omit CORS headers from the response, causing the browser to block the request

---

### Requirement 8: M-Pesa Callback Authenticity

**User Story:** As a payments engineer, I want M-Pesa callbacks validated for authenticity, so that forged payment notifications cannot credit customer accounts.

#### Acceptance Criteria

1. WHEN the API_Server receives an M-Pesa callback, THE Payment_Service SHALL validate the callback source against Safaricom's published IP ranges
2. WHEN the callback source IP is not in the allowed range, THE Payment_Service SHALL reject the callback with HTTP 403 and log the attempt
3. WHEN the API_Server receives a valid M-Pesa callback, THE Payment_Service SHALL correlate the payment using the CheckoutRequestID or AccountReference rather than a client-supplied invoice_id
4. IF a callback arrives for a CheckoutRequestID that has already been processed, THEN THE Payment_Service SHALL return HTTP 200 without creating a duplicate payment record

---

### Requirement 9: Secrets Management and TLS

**User Story:** As a DevOps engineer, I want credentials stored in a secret manager and TLS termination configured, so that secrets are not in plain-text environment variables and all traffic is encrypted.

#### Acceptance Criteria

1. THE API_Server SHALL load database credentials, Redis passwords, M-Pesa API keys, and JWT signing keys from a secret store (environment variables sourced from Docker secrets or a vault) rather than hardcoded values
2. THE API_Server SHALL accept connections only via TLS in production (either directly or via a reverse proxy performing TLS termination)
3. THE API_Server SHALL never log secret values, connection strings containing passwords, or JWT signing keys
4. IF a required secret is missing at startup, THEN THE API_Server SHALL fail fast with a descriptive error message identifying the missing secret

---

### Requirement 10: Void and Delete Flows

**User Story:** As an accountant, I want to void posted invoices/bills and delete drafts, so that erroneous documents can be corrected without corrupting the ledger.

#### Acceptance Criteria

1. WHEN an authorized user voids a posted invoice, THE Ledger_Service SHALL create a reversing journal entry and set the invoice status to Voided
2. WHEN an authorized user voids a posted bill, THE Ledger_Service SHALL create a reversing journal entry and set the bill status to Voided
3. WHEN an authorized user deletes a draft invoice or bill, THE API_Server SHALL remove the record and its line items from the database
4. IF a user attempts to void an invoice that has payments applied, THEN THE Ledger_Service SHALL reject the void operation with a message indicating payments must be reversed first
5. IF a user attempts to delete a non-draft document, THEN THE API_Server SHALL reject the request with HTTP 409 indicating only drafts can be deleted

---

### Requirement 11: Pagination

**User Story:** As a user with large datasets, I want list endpoints to support pagination, so that the UI remains responsive and the API does not return unbounded result sets.

#### Acceptance Criteria

1. THE API_Server SHALL accept optional limit and offset query parameters on all list endpoints
2. WHEN limit is not specified, THE API_Server SHALL apply a default limit of 50 records
3. THE API_Server SHALL return pagination metadata (total_count, limit, offset, has_more) in the response body alongside the results array
4. THE API_Server SHALL enforce a maximum limit of 500 records per request

---

### Requirement 12: User Management UI

**User Story:** As a business owner, I want a frontend screen to invite users and assign roles, so that I can manage team access without backend intervention.

#### Acceptance Criteria

1. WHEN an Owner or Admin navigates to Settings > Users, THE Frontend SHALL display a list of current users with their roles and status
2. WHEN an Owner or Admin clicks "Invite User", THE Frontend SHALL present a form to enter email and select a role
3. WHEN the invite form is submitted, THE API_Server SHALL create a pending user record and trigger an invitation email via the notification queue
4. WHEN an Owner or Admin changes a user's role, THE API_Server SHALL update the role and the change SHALL take effect on the user's next token refresh
5. WHEN an Owner or Admin deactivates a user, THE API_Server SHALL revoke all active sessions for that user

---

### Requirement 13: Settings Persistence

**User Story:** As a business owner, I want all Settings tabs (Company, Tax, Payments, Document Numbers) to save changes, so that configuration edits persist across sessions.

#### Acceptance Criteria

1. WHEN the user edits Company settings and clicks Save, THE API_Server SHALL persist the branding and company details to the entity_settings table
2. WHEN the user edits Tax settings and clicks Save, THE API_Server SHALL persist VAT registration, rates, and WHT configuration
3. WHEN the user edits Payment settings and clicks Save, THE API_Server SHALL persist M-Pesa paybill, Flutterwave keys, and bank transfer preferences
4. WHEN the user edits Document Number settings and clicks Save, THE API_Server SHALL persist sequence prefixes, start numbers, and year_reset flags
5. WHEN settings are saved successfully, THE API_Server SHALL trigger a configuration reload so changes take effect without server restart

---

### Requirement 14: CI Pipeline

**User Story:** As a developer, I want a GitHub Actions pipeline that builds, tests, and lints on every push, so that code quality issues are caught before merge.

#### Acceptance Criteria

1. WHEN code is pushed to any branch, THE CI_Pipeline SHALL execute cargo build for the workspace
2. WHEN code is pushed to any branch, THE CI_Pipeline SHALL execute the full test suite and report failures
3. WHEN code is pushed to any branch, THE CI_Pipeline SHALL run cargo clippy with warnings treated as errors
4. WHEN code is pushed to any branch, THE CI_Pipeline SHALL verify that pending migrations apply cleanly against a fresh PostgreSQL instance
5. WHEN code is pushed to any branch, THE CI_Pipeline SHALL build the Frontend and run ESLint and TypeScript type checking
6. THE CI_Pipeline SHALL complete all checks within 10 minutes for a typical commit

---

### Requirement 15: Containerization and Deployment

**User Story:** As a DevOps engineer, I want Docker containers for the API and UI with production-ready compose configuration, so that the system can be deployed reliably to any hosting environment.

#### Acceptance Criteria

1. THE API_Server SHALL have a multi-stage Dockerfile producing a minimal runtime image without build tooling
2. THE Frontend SHALL have a Dockerfile producing an Nginx-based image serving static assets
3. THE CI_Pipeline SHALL produce a docker-compose.yml for production that includes API, Frontend, PostgreSQL, Redis, and a reverse proxy with TLS
4. WHEN the API container starts, THE API_Server SHALL expose a /health endpoint returning HTTP 200 when the database and Redis connections are healthy
5. WHEN the /health endpoint detects an unhealthy dependency, THE API_Server SHALL return HTTP 503 with details identifying the failing component
6. THE API_Server container SHALL support graceful shutdown, completing in-flight requests before terminating within a 30-second timeout

---

### Requirement 16: Backups and Migration Safety

**User Story:** As a platform operator, I want automated database backups and safe migration practices, so that data loss is recoverable and schema changes do not corrupt production.

#### Acceptance Criteria

1. THE CI_Pipeline SHALL include a documented runbook for database backup and restore using pg_dump/pg_restore
2. THE API_Server SHALL apply migrations only on startup and log each migration applied with its version number
3. WHEN a migration fails, THE API_Server SHALL halt startup and log the failed migration name and error
4. THE CI_Pipeline SHALL test migrations by applying them to a fresh database and then running the full test suite against it

---

### Requirement 17: Posting Group Matrices

**User Story:** As an accountant, I want VAT and General posting group matrices to determine accounts automatically, so that invoice and bill posting uses the correct GL accounts based on customer/vendor and product classifications.

#### Acceptance Criteria

1. WHEN a VAT Business Group and VAT Product Group combination is configured, THE Posting_Resolver SHALL use the matrix entry to determine the VAT rate and output/input GL accounts
2. WHEN a General Business Group and General Product Group combination is configured, THE Posting_Resolver SHALL use the matrix entry to determine sales, purchase, and COGS GL accounts
3. WHEN posting an invoice line, THE Posting_Resolver SHALL look up the customer's VAT Business Group and the product's VAT Product Group to determine the VAT treatment
4. IF a required posting group combination is not configured, THEN THE Posting_Resolver SHALL fall back to the default accounts defined in entity settings and log a warning
5. THE Frontend SHALL provide a matrix editor UI under Settings > Posting Accounts for configuring group combinations

---

### Requirement 18: M-Pesa STK Push Integration

**User Story:** As a business owner, I want to send M-Pesa STK Push prompts to customers, so that they can pay invoices directly from their phone without manual USSD entry.

#### Acceptance Criteria

1. WHEN an authorized user triggers STK Push for an invoice, THE Payment_Service SHALL submit a request to the Daraja_API with the customer's phone number, amount, and account reference
2. WHEN the Daraja_API returns a successful initiation response, THE Payment_Service SHALL store the CheckoutRequestID and mark the payment as pending
3. WHEN the Daraja_API callback confirms payment, THE Payment_Service SHALL record the payment and apply it to the referenced invoice atomically
4. IF the Daraja_API returns an error (insufficient funds, timeout, wrong PIN), THEN THE Payment_Service SHALL update the payment status and notify the user
5. IF M-Pesa credentials are not configured, THEN THE Payment_Service SHALL return a descriptive error indicating M-Pesa is not configured for this entity

---

### Requirement 19: Notification Workers

**User Story:** As a business owner, I want email, WhatsApp, and SMS notifications delivered reliably, so that invoice reminders, payment confirmations, and user invitations reach recipients.

#### Acceptance Criteria

1. WHEN a notification event is produced (invoice sent, payment received, user invited), THE Core_Engine SHALL enqueue a message to the Redis notification queue
2. WHEN a notification worker picks up a message, THE Core_Engine SHALL deliver it via the configured channel (email/SMTP, WhatsApp API, or SMS gateway)
3. IF delivery fails, THEN THE Core_Engine SHALL retry with exponential backoff up to 3 attempts before marking the notification as failed
4. THE Core_Engine SHALL log delivery status (queued, sent, delivered, failed) for each notification for audit purposes

---

### Requirement 20: Supplier Credit Note Line Items

**User Story:** As an accountant, I want supplier credit notes to store individual line items, so that the credit can be traced to specific products/services and posted to the correct GL accounts.

#### Acceptance Criteria

1. WHEN a supplier credit note is created, THE Core_Engine SHALL accept and store line items with product, quantity, unit price, VAT treatment, and GL account
2. WHEN the supplier credit note is posted, THE Ledger_Service SHALL create journal entries at the line-item level using each line's GL account
3. THE API_Server SHALL return line items when retrieving a supplier credit note

---

### Requirement 21: Statutory Payroll Accuracy

**User Story:** As a payroll administrator, I want PAYE, NSSF, NHIF, and housing levy calculations to match KRA/statutory formulas precisely, so that payroll filings are accurate.

#### Acceptance Criteria

1. WHEN computing PAYE, THE Core_Engine SHALL apply the current KRA tax brackets, personal relief (2,400 KES/month), and insurance relief for NHIF contributions
2. WHEN computing NSSF, THE Core_Engine SHALL apply the current Tier I and Tier II rates and caps
3. WHEN computing NHIF, THE Core_Engine SHALL apply the current graduated scale based on gross pay
4. WHEN computing housing levy, THE Core_Engine SHALL apply the 1.5% employer and 1.5% employee rate on gross pay
5. THE Core_Engine SHALL round PAYE to the nearest shilling (0 decimal places) as required by KRA
6. THE Core_Engine SHALL include SHA/insurance relief deduction from PAYE where the employee has qualifying NHIF contributions

---

### Requirement 22: Rate Limiting

**User Story:** As a security engineer, I want rate limits and request size caps on public endpoints, so that the API is protected against brute-force attacks and denial-of-service via oversized payloads.

#### Acceptance Criteria

1. THE API_Server SHALL enforce a rate limit of 10 requests per minute on the login endpoint per IP address
2. THE API_Server SHALL enforce a rate limit of 60 requests per minute on authenticated endpoints per user
3. THE API_Server SHALL reject request bodies larger than 10 MB with HTTP 413
4. WHEN a rate limit is exceeded, THE API_Server SHALL return HTTP 429 with a Retry-After header indicating seconds until the limit resets

---

### Requirement 23: Observability

**User Story:** As a DevOps engineer, I want structured logging, Prometheus metrics, and distributed tracing, so that production issues can be diagnosed quickly.

#### Acceptance Criteria

1. THE API_Server SHALL emit structured JSON logs including request_id, user_id, entity_id, method, path, status_code, and latency_ms for every request
2. THE API_Server SHALL expose a /metrics endpoint in Prometheus exposition format with request count, latency histograms, and error rates
3. THE API_Server SHALL propagate OpenTelemetry trace context (traceparent header) and export spans to a configured collector
4. THE API_Server SHALL include the request_id in all error responses so users can reference it in support requests

---

### Requirement 24: Performance Optimization

**User Story:** As a user, I want API responses to return within acceptable latency, so that the UI feels responsive even with large datasets.

#### Acceptance Criteria

1. THE Core_Engine SHALL include database indexes on all foreign key columns and frequently-filtered columns (entity_id, status, date, customer_id, vendor_id)
2. THE Core_Engine SHALL eliminate N+1 query patterns on detail endpoints by using JOINs or batch loading for related records (invoice lines, journal lines, payment allocations)
3. WHEN a list endpoint is called with default pagination, THE API_Server SHALL return results within 200ms for datasets up to 100,000 records (measured at the database layer)

---

### Requirement 25: Customer Statements

**User Story:** As a business owner, I want to generate and send customer statements showing outstanding balances, so that customers receive a summary of their account activity.

#### Acceptance Criteria

1. WHEN a user requests a customer statement, THE API_Server SHALL generate a statement showing all invoices, payments, and credit notes for the specified date range
2. THE API_Server SHALL calculate and display the running balance and total outstanding amount
3. WHEN a user sends a statement, THE Core_Engine SHALL deliver it via the customer's preferred channel (email or WhatsApp) using the notification queue
4. THE Frontend SHALL provide a UI to select customers, date range, and trigger statement generation/sending

---

### Requirement 26: Invoice Template Editor

**User Story:** As a business owner, I want to customize invoice templates with my branding, so that sent invoices reflect my company's visual identity.

#### Acceptance Criteria

1. THE Frontend SHALL provide a template editor allowing the user to configure logo, colors, footer text, and field visibility
2. WHEN the user saves a template, THE API_Server SHALL persist the template configuration per entity
3. WHEN an invoice PDF is generated, THE Core_Engine SHALL apply the entity's saved template configuration
4. THE Frontend SHALL show a live preview of the template as the user edits it

---

### Requirement 27: Dashboard Polish

**User Story:** As a user, I want the dashboard to handle empty data, loading states, and errors gracefully, so that the UI remains informative and functional regardless of data availability.

#### Acceptance Criteria

1. WHEN dashboard data is loading, THE Frontend SHALL display skeleton loaders in place of data widgets
2. WHEN no data exists for a metric (e.g., zero invoices), THE Frontend SHALL display "0" or "No data" rather than "NaN%" or blank space
3. WHEN a dashboard API call fails, THE Frontend SHALL display an error message with a retry button within the affected widget without crashing the entire page
4. THE Frontend SHALL wrap each dashboard widget in an error boundary to isolate failures

---

### Requirement 28: Build Warnings Cleanup

**User Story:** As a developer, I want zero compiler and linter warnings in the codebase, so that real issues are not hidden by noise and CI can enforce warning-free builds.

#### Acceptance Criteria

1. THE Core_Engine and API_Server SHALL compile with zero warnings under cargo clippy with default lints
2. THE Frontend SHALL pass ESLint with zero warnings under the project's configured ruleset
3. WHEN the CI_Pipeline runs, THE CI_Pipeline SHALL fail the build if any new warnings are introduced (using -D warnings for Rust and --max-warnings 0 for ESLint)

---

### Requirement 29: Individual Report Pages

**User Story:** As a user, I want dedicated pages for each report type (P&L, Balance Sheet, Trial Balance, etc.), so that I can navigate directly to the report I need.

#### Acceptance Criteria

1. THE Frontend SHALL provide a dedicated route and page for each report type: Profit & Loss, Balance Sheet, Cash Flow, Trial Balance, General Ledger, AR Ageing, AP Ageing, VAT Report
2. WHEN a user navigates to a report page, THE Frontend SHALL display the report with appropriate filters (date range, comparison period) and export options
3. THE Frontend SHALL maintain a Reports menu/index page linking to all individual report pages

---

### Requirement 30: Document Sequences UI

**User Story:** As a business owner, I want to configure document number prefixes and start numbers from the Settings UI, so that numbering matches my business conventions.

#### Acceptance Criteria

1. WHEN the user navigates to Settings > Document Numbers, THE Frontend SHALL display the current prefix, start number, and year_reset flag for each document type (Invoice, Bill, Receipt, Credit Note, Estimate)
2. WHEN the user edits a sequence configuration and clicks Save, THE API_Server SHALL validate and persist the new settings
3. IF a user sets a start number lower than the current counter, THEN THE API_Server SHALL reject the change with an error explaining that the start number cannot be lower than the last issued number
