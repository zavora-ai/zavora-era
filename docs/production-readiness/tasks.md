# Implementation Plan: Production Readiness

## Overview

This plan brings Zavora ERP from functional prototype to production-grade deployment across four priority tiers. Tasks are ordered by dependency: P0 blockers first (auth, atomicity, tenant scoping, tests), then P1 operational requirements, P2 feature completions, and P3 polish. Each task builds incrementally on prior work, with property-based tests validating correctness properties from the design.

## Tasks

- [ ] 1. P0 — Database Migration and Schema Foundation
  - [ ] 1.1 Create migration 006 with all new tables and schema changes
    - Add `password_hash`, `status`, `invited_at`, `last_login_at` columns to `users` table
    - Create `refresh_tokens` table with indexes
    - Create posting group tables (`vat_business_groups`, `vat_product_groups`, `vat_posting_matrix`, `general_business_groups`, `general_product_groups`, `general_posting_matrix`)
    - Create `supplier_credit_note_lines` table
    - Add posting group FK columns to `customers`, `vendors`, `products`
    - Add `invoice_template` JSONB to `entity_settings`
    - Add `last_fiscal_year_allocated` to `entity_settings`
    - Add performance indexes (entity_id + customer_id, status, date, vendor_id, party_id, payment_date, account_code)
    - _Requirements: 1.4, 3.4, 6.1, 17.1, 20.1, 24.1, 26.2_

- [ ] 2. P0 — JWT Authentication Module
  - [ ] 2.1 Implement JWT auth core in `zavora-erp-core`
    - Add `jsonwebtoken = "9"` and `argon2 = "0.5"` dependencies
    - Create `src/auth/mod.rs` with `JwtConfig`, `Claims`, `TokenPair` structs
    - Implement `encode_token()`, `decode_token()`, `hash_password()`, `verify_password()` functions
    - Implement refresh token generation and storage (Redis-backed with TTL)
    - _Requirements: 1.1, 1.4, 1.6, 1.7_

  - [ ]* 2.2 Write property tests for JWT round-trip
    - **Property 1: JWT Round-Trip (encode → decode preserves claims)**
    - **Validates: Requirements 1.1, 1.2**

  - [ ]* 2.3 Write property tests for invalid JWT rejection
    - **Property 2: Invalid JWT Rejection**
    - **Validates: Requirements 1.3, 1.5**

  - [ ]* 2.4 Write property tests for password hash security
    - **Property 3: Password Hash Security**
    - **Validates: Requirements 1.4**

  - [ ] 2.5 Replace header-based `AuthContext` extractor with JWT middleware in `zavora-erp-api`
    - Modify `src/middleware/auth.rs` to extract and verify JWT from `Authorization: Bearer` header
    - Reject requests with X-User-* headers but no valid JWT
    - Extract `user_id`, `entity_id`, `role` from JWT claims into `AuthContext`
    - _Requirements: 1.2, 1.3, 1.5_

  - [ ] 2.6 Implement login and token refresh routes
    - Create `POST /api/v1/auth/login` endpoint (credential validation, JWT issuance)
    - Create `POST /api/v1/auth/refresh` endpoint (refresh token → new access token)
    - Create `POST /api/v1/auth/register` endpoint (Argon2id password hashing + user creation)
    - _Requirements: 1.1, 1.6, 1.7_

- [ ] 3. P0 — Transaction Atomicity for Ledger-Coupled Flows
  - [ ] 3.1 Refactor `create_and_post()` to accept `&mut sqlx::Transaction`
    - Create `create_and_post_in_tx()` variant in `zavora-erp-core/src/engine.rs`
    - Add debit == credit validation before transaction commit
    - _Requirements: 2.6_

  - [ ] 3.2 Wrap `record_payment` in a single database transaction
    - Thread `sqlx::Transaction` through payment insert, balance update, and JE creation
    - Rollback all on any step failure
    - _Requirements: 2.1, 2.5_

  - [ ] 3.3 Wrap `post_invoice` in a single database transaction
    - Thread transaction through status update, JE creation, and receivables balance adjustment
    - _Requirements: 2.2, 2.5_

  - [ ] 3.4 Wrap `create_credit_note` in a single database transaction
    - Thread transaction through CN record creation, reversing JE, and balance adjustment
    - _Requirements: 2.3, 2.5_

  - [ ] 3.5 Wrap `apply_unapplied_payment` in a single database transaction
    - Thread transaction through allocation record, balance transfer, and JE creation
    - _Requirements: 2.4, 2.5_

  - [ ]* 3.6 Write property tests for journal entry balance invariant
    - **Property 4: Journal Entry Balance Invariant**
    - **Validates: Requirements 2.6, 4.1**

  - [ ]* 3.7 Write property tests for payment recording atomicity
    - **Property 5: Transaction Atomicity (payment recording)**
    - **Validates: Requirements 2.1, 2.5**

  - [ ]* 3.8 Write property tests for invoice posting atomicity
    - **Property 6: Transaction Atomicity (invoice posting)**
    - **Validates: Requirements 2.2, 2.5**

- [ ] 4. P0 — Per-Request Tenant Scoping
  - [ ] 4.1 Remove startup `ENTITY_ID` env var as query-scoping mechanism
    - Replace `engine.entity_id()` with per-request `ctx.entity_id` in all service functions
    - Add `entity_id: Uuid` parameter to all core service functions
    - Ensure all queries include `WHERE entity_id = $entity_id` from the AuthContext parameter
    - _Requirements: 3.1, 3.2, 3.4_

  - [ ] 4.2 Implement cross-tenant 404 response behavior
    - When a record belongs to a different entity_id, return HTTP 404 (not 403)
    - Ensure INSERT operations set `entity_id` from AuthContext JWT claims
    - _Requirements: 3.3_

  - [ ]* 4.3 Write property tests for tenant isolation
    - **Property 7: Tenant Isolation**
    - **Validates: Requirements 3.1, 3.2, 3.3**

- [ ] 5. P0 — Monetary Rounding Policy
  - [ ] 5.1 Implement rounding utility functions in `zavora-erp-core`
    - Create `round_money()` — banker's rounding to 2dp
    - Create `round_paye()` — round to nearest shilling (0dp)
    - Apply `round_money()` to all monetary computations in invoicing, payments, and journal posting
    - _Requirements: 5.1, 5.4_

  - [ ] 5.2 Implement line-level VAT rounding and rounding adjustment
    - Round each invoice line's VAT independently before summing
    - When JE imbalance ≤ 0.01 due to VAT accumulation, insert rounding adjustment line to configured rounding expense account
    - Reject as truly unbalanced if imbalance > 0.01
    - _Requirements: 5.2, 5.3_

  - [ ]* 5.3 Write property tests for monetary rounding consistency
    - **Property 8: Monetary Rounding Consistency**
    - **Validates: Requirements 5.1**

  - [ ]* 5.4 Write property tests for VAT line-level rounding order
    - **Property 9: VAT Line-Level Rounding Order**
    - **Validates: Requirements 5.2**

  - [ ]* 5.5 Write property tests for rounding adjustment balancing
    - **Property 10: Rounding Adjustment Balances Entry**
    - **Validates: Requirements 5.3**

- [ ] 6. P0 — Automated Test Suite Foundation
  - [ ] 6.1 Set up test infrastructure with `proptest` and integration test harness
    - Add `proptest = "1"` to dev-dependencies
    - Create `tests/property_tests/` directory structure in `zavora-erp-core`
    - Create `tests/integration_tests/` directory structure
    - Set up test database provisioning utilities
    - _Requirements: 4.1, 4.2, 4.3, 4.4, 4.5_

  - [ ] 6.2 Write integration tests for payment recording flows
    - Test single payment, partial payment, overpayment, and multi-currency payment
    - Verify journal balancing on each path
    - _Requirements: 4.2_

  - [ ] 6.3 Write unit tests for payroll statutory calculations
    - Test PAYE brackets with boundary values
    - Test NSSF Tier I/II contributions and caps
    - Test NHIF/SHA graduated scale
    - Test housing levy computation
    - _Requirements: 4.3_

  - [ ] 6.4 Write integration tests for period close and FX revaluation
    - Verify posting to closed period is rejected
    - Verify FX gain/loss journal entries against known exchange rates
    - _Requirements: 4.4, 4.5_

- [ ] 7. Checkpoint — P0 Complete
  - Ensure all tests pass, ask the user if questions arise.

- [ ] 8. P1 — Document Numbering
  - [ ] 8.1 Implement gapless document sequencer with transactional allocation
    - Move number allocation inside document creation transaction using `SELECT ... FOR UPDATE`
    - Implement year reset logic (detect fiscal year boundary, reset counter)
    - Implement format pattern: `{PREFIX}-{YEAR}-{ZERO_PADDED_COUNTER}`
    - _Requirements: 6.1, 6.2, 6.3, 6.4, 6.5_

  - [ ]* 8.2 Write property tests for gapless document numbering
    - **Property 11: Gapless Document Numbering**
    - **Validates: Requirements 6.1, 6.2**

  - [ ]* 8.3 Write property tests for document number format
    - **Property 12: Document Number Format**
    - **Validates: Requirements 6.4**

  - [ ]* 8.4 Write property tests for concurrent number uniqueness
    - **Property 13: Concurrent Number Uniqueness**
    - **Validates: Requirements 6.5**

- [ ] 9. P1 — CORS Lockdown
  - [ ] 9.1 Implement environment-aware CORS configuration
    - Replace `CorsLayer::permissive()` with production/development mode switching
    - In production: restrict to `CORS_ALLOWED_ORIGINS` env var (comma-separated list)
    - In development: permit all origins
    - Omit CORS headers for non-allowed origins
    - _Requirements: 7.1, 7.2, 7.3_

  - [ ]* 9.2 Write unit tests for CORS mode switching
    - Test production mode with allowed origin, blocked origin, and development mode permissive
    - _Requirements: 7.1, 7.2, 7.3_

- [ ] 10. P1 — Secrets Management and Startup Validation
  - [ ] 10.1 Implement secret loading and fail-fast startup validation
    - Validate all required secrets at startup: `DATABASE_URL`, `REDIS_URL`, `JWT_ACCESS_SECRET`, `JWT_REFRESH_SECRET`, `MPESA_CONSUMER_KEY`, `MPESA_CONSUMER_SECRET`
    - Implement `Redacted<T>` wrapper type (displays `[REDACTED]` in Debug/Display)
    - Ensure no secret values appear in logs
    - Fail fast with descriptive error identifying the missing secret
    - _Requirements: 9.1, 9.3, 9.4_

- [ ] 11. P1 — Void and Delete Flows
  - [ ] 11.1 Implement void flow for posted invoices and bills
    - Add `POST /api/v1/invoices/{id}/void` and `POST /api/v1/bills/{id}/void` routes
    - Pre-check: reject void if payments applied (HTTP 409)
    - Create reversing JE (debits ↔ credits swapped) within a transaction
    - Set status to `Voided`, store void reason and voided_by
    - _Requirements: 10.1, 10.2, 10.4_

  - [ ] 11.2 Implement delete flow for draft documents
    - Add `DELETE /api/v1/invoices/{id}` and `DELETE /api/v1/bills/{id}` routes
    - Pre-check: status must be `draft`; return HTTP 409 otherwise
    - Hard delete record + line items (CASCADE)
    - _Requirements: 10.3, 10.5_

  - [ ]* 11.3 Write property tests for void creates reversing journal entry
    - **Property 16: Void Creates Reversing Journal Entry**
    - **Validates: Requirements 10.1, 10.2**

  - [ ]* 11.4 Write property tests for draft deletion completeness
    - **Property 17: Draft Deletion Completeness**
    - **Validates: Requirements 10.3**

- [ ] 12. P1 — Pagination
  - [ ] 12.1 Implement standardized pagination extractor and response
    - Create `PaginationParams` extractor (default limit=50, max=500, default offset=0)
    - Create `PaginatedResponse<T>` struct with `data`, `total_count`, `limit`, `offset`, `has_more`
    - Apply to all list endpoints using `COUNT(*) OVER()` pattern
    - _Requirements: 11.1, 11.2, 11.3, 11.4_

  - [ ]* 12.2 Write property tests for pagination correctness
    - **Property 18: Pagination Correctness**
    - **Validates: Requirements 11.1, 11.3**

- [ ] 13. P1 — User Management API and UI
  - [ ] 13.1 Implement user management API endpoints
    - Create/enhance `POST /api/v1/users` (create pending user, enqueue invitation email)
    - Create `PUT /api/v1/users/{id}/role` (update role)
    - Create `POST /api/v1/users/{id}/deactivate` (deactivate + revoke all refresh tokens)
    - Create `POST /api/v1/users/{id}/reactivate` (re-enable account)
    - _Requirements: 12.3, 12.4, 12.5_

  - [ ] 13.2 Implement User Management UI in `zavora-erp-ui`
    - Create Settings > Users page listing current users with role, status, last_active
    - Add "Invite User" modal with email and role selector
    - Add role change dropdown (immediate save)
    - Add deactivate button with confirmation dialog
    - _Requirements: 12.1, 12.2_

- [ ] 14. P1 — Settings Persistence
  - [ ] 14.1 Wire all Settings tabs to API persistence
    - Company tab: save branding and company details to `entity_settings`
    - Tax tab: save VAT registration, rates, WHT configuration
    - Payments tab: save M-Pesa paybill, Flutterwave keys, bank transfer preferences
    - Document Numbers tab: save sequence prefixes, start numbers, year_reset flags via `PUT /api/v1/settings/sequences`
    - Extend `engine.reload_config()` to refresh all config sections after save
    - _Requirements: 13.1, 13.2, 13.3, 13.4, 13.5_

  - [ ]* 14.2 Write integration tests for settings persistence
    - Test save and reload for each tab (Company, Tax, Payments, Document Numbers)
    - _Requirements: 13.1, 13.2, 13.3, 13.4_

- [ ] 15. P1 — CI Pipeline
  - [ ] 15.1 Create GitHub Actions CI workflow
    - Create `.github/workflows/ci.yml`
    - Rust job: `cargo clippy --workspace -- -D warnings`, `cargo build`, `sqlx migrate run`, `cargo test --workspace`
    - Frontend job: `npm ci`, `npx tsc --noEmit`, `npx eslint . --max-warnings 0`, `npm run build`
    - Use service containers (PostgreSQL 17, Redis 7)
    - Target: all checks within 10 minutes
    - _Requirements: 14.1, 14.2, 14.3, 14.4, 14.5, 14.6_

- [ ] 16. P1 — Containerization and Deployment
  - [ ] 16.1 Create production Dockerfiles and compose configuration
    - Create multi-stage `Dockerfile` for API (Rust builder → debian-slim runtime)
    - Create multi-stage `Dockerfile` for Frontend (Node builder → Nginx)
    - Create `docker-compose.prod.yml` with API, Frontend, PostgreSQL, Redis, reverse proxy (TLS)
    - _Requirements: 15.1, 15.2, 15.3, 9.2_

  - [ ] 16.2 Implement health endpoint and graceful shutdown
    - Enhance `/health` endpoint to check PgPool (`SELECT 1`) and Redis (`PING`)
    - Return HTTP 503 with failing component details when unhealthy
    - Implement graceful shutdown with `tokio::signal::ctrl_c()` + 30-second drain timeout
    - _Requirements: 15.4, 15.5, 15.6_

- [ ] 17. P1 — Backups and Migration Safety
  - [ ] 17.1 Document backup runbook and ensure migration safety
    - Create `docs/BACKUP_RUNBOOK.md` with pg_dump/pg_restore procedures
    - Ensure `sqlx::migrate!()` on startup logs each migration version applied
    - Ensure failed migration halts startup with descriptive error
    - CI migration testing (fresh DB → migrations → test suite)
    - _Requirements: 16.1, 16.2, 16.3, 16.4_

- [ ] 18. Checkpoint — P1 Complete
  - Ensure all tests pass, ask the user if questions arise.

- [ ] 19. P2 — Posting Group Matrices
  - [ ] 19.1 Implement posting group resolver in `zavora-erp-core`
    - Create `src/posting/groups.rs` with `resolve_vat_posting()` and `resolve_general_posting()` functions
    - Implement matrix lookup: (biz_group_id × prod_group_id) → accounts
    - Implement fallback to entity defaults when combination not configured (log warning)
    - Wire resolver into invoice and bill posting flows
    - _Requirements: 17.1, 17.2, 17.3, 17.4_

  - [ ] 19.2 Implement posting group CRUD API routes
    - CRUD for `vat_business_groups`, `vat_product_groups`, `vat_posting_matrix`
    - CRUD for `general_business_groups`, `general_product_groups`, `general_posting_matrix`
    - Add posting group assignment endpoints for customers, vendors, products
    - _Requirements: 17.1, 17.2, 17.3_

  - [ ] 19.3 Implement posting group matrix editor UI
    - Create Settings > Posting Accounts page with matrix editor
    - Display VAT and General posting group matrices as editable tables
    - Allow configuring group combinations with account selectors
    - _Requirements: 17.5_

  - [ ]* 19.4 Write property tests for VAT posting group matrix lookup
    - **Property 19: Posting Group Matrix Lookup**
    - **Validates: Requirements 17.1, 17.4**

  - [ ]* 19.5 Write property tests for general posting group matrix lookup
    - **Property 20: General Posting Group Matrix Lookup**
    - **Validates: Requirements 17.2, 17.4**

- [ ] 20. P2 — M-Pesa STK Push Integration
  - [ ] 20.1 Implement STK Push initiation and callback handling
    - Create `POST /api/v1/payments/mpesa-stk-push` endpoint
    - Implement Daraja OAuth token caching in Redis
    - Submit STK Push request with phone, amount, account reference (invoice number)
    - Store `CheckoutRequestID` in `mpesa_transactions` with status=`pending`
    - Implement callback handler: correlate via `CheckoutRequestID`, record payment atomically
    - Implement M-Pesa IP allowlist validation for callbacks
    - Map Daraja error codes to user-friendly messages
    - _Requirements: 18.1, 18.2, 18.3, 18.4, 18.5, 8.1, 8.2, 8.3, 8.4_

  - [ ]* 20.2 Write property tests for M-Pesa IP validation
    - **Property 14: M-Pesa IP Validation**
    - **Validates: Requirements 8.1**

  - [ ]* 20.3 Write property tests for M-Pesa callback idempotency
    - **Property 15: M-Pesa Callback Idempotency**
    - **Validates: Requirements 8.4**

- [ ] 21. P2 — Notification Workers
  - [ ] 21.1 Implement Redis Streams-based notification queue and workers
    - Create notification queue using `XADD`/`XREADGROUP` (Redis Streams)
    - Implement worker loop: pick up message → deliver (email/SMS/WhatsApp) → XACK
    - Implement exponential backoff retry (up to 3 attempts)
    - Log delivery status (queued, sent, delivered, failed) for audit
    - Wire notification events into invoice send, payment received, and user invitation flows
    - _Requirements: 19.1, 19.2, 19.3, 19.4_

  - [ ]* 21.2 Write integration tests for notification queue lifecycle
    - Test enqueue, delivery, retry on failure, and max attempts marking
    - _Requirements: 19.1, 19.2, 19.3, 19.4_

- [ ] 22. P2 — Supplier Credit Note Line Items
  - [ ] 22.1 Implement supplier credit note line items in `zavora-erp-core`
    - Extend supplier credit note creation to accept and store line items (product, quantity, unit_price, vat_treatment, gl_account_code)
    - Modify posting to create journal entries per line (using each line's GL account)
    - Return line items in supplier credit note retrieval
    - _Requirements: 20.1, 20.2, 20.3_

  - [ ]* 22.2 Write property tests for supplier credit note line round-trip
    - **Property 21: Supplier Credit Note Line Round-Trip**
    - **Validates: Requirements 20.1, 20.3**

  - [ ]* 22.3 Write property tests for supplier credit note line-level posting
    - **Property 22: Supplier Credit Note Line-Level Posting**
    - **Validates: Requirements 20.2**

- [ ] 23. P2 — Statutory Payroll Accuracy
  - [ ] 23.1 Fix payroll statutory calculations in `zavora-erp-core`
    - Add insurance relief: `min(SHA_contribution × 0.15, insurance_relief_cap)` deducted from PAYE
    - Round final PAYE to 0dp using `round_paye()` (nearest shilling)
    - Verify NHIF → SHA transition rate (2.75%) against KRA 2025 rates
    - Verify NSSF Tier I/II rates and 36,000 cap
    - Verify housing levy 1.5% employer + 1.5% employee
    - _Requirements: 21.1, 21.2, 21.3, 21.4, 21.5, 21.6_

  - [ ]* 23.2 Write property tests for payroll deduction accuracy
    - **Property 23: Payroll Deduction Accuracy**
    - **Validates: Requirements 21.1, 21.2, 21.3, 21.4, 21.6**

  - [ ]* 23.3 Write property tests for PAYE rounding to nearest shilling
    - **Property 24: PAYE Rounding to Nearest Shilling**
    - **Validates: Requirements 21.5**

- [ ] 24. P2 — Rate Limiting
  - [ ] 24.1 Implement rate limiting middleware
    - Add `governor = "0.7"` and `tower-governor` dependencies
    - Configure login endpoint: 10 req/min per IP
    - Configure authenticated endpoints: 60 req/min per user_id
    - Configure M-Pesa callback: 30 req/min per IP
    - Add body size limit: 10 MB max via `DefaultBodyLimit`
    - Return HTTP 429 with `Retry-After` header when limit exceeded
    - Return HTTP 413 for oversized request bodies
    - _Requirements: 22.1, 22.2, 22.3, 22.4_

  - [ ]* 24.2 Write integration tests for rate limiting
    - Test burst requests trigger 429, verify Retry-After header
    - Test body size limit returns 413
    - _Requirements: 22.1, 22.2, 22.3, 22.4_

- [ ] 25. P2 — Observability
  - [ ] 25.1 Implement structured logging and metrics
    - Switch `tracing_subscriber` to JSON format in production
    - Add request_id, user_id, entity_id, method, path, status_code, latency_ms to spans
    - Add `metrics = "0.24"` + `metrics-exporter-prometheus`
    - Expose `/metrics` endpoint (http_requests_total, http_request_duration_seconds, db_query_duration_seconds, active_connections)
    - Include request_id in all error responses
    - _Requirements: 23.1, 23.2, 23.4_

  - [ ] 25.2 Implement OpenTelemetry distributed tracing
    - Add `tracing-opentelemetry` + `opentelemetry-otlp` dependencies
    - Propagate `traceparent` header
    - Export spans to configurable OTLP collector endpoint
    - _Requirements: 23.3_

- [ ] 26. P2 — Performance Optimization
  - [ ] 26.1 Eliminate N+1 queries and optimize detail endpoints
    - Refactor invoice detail, bill detail, payment detail to use `LEFT JOIN` on line items
    - Use batch loading for journal lines and payment allocations
    - Verify list endpoints return within 200ms for datasets up to 100k records
    - _Requirements: 24.2, 24.3_

- [ ] 27. P2 — Customer Statements
  - [ ] 27.1 Implement customer statement generation API
    - Create `GET /api/v1/customers/{id}/statement?from=...&to=...` endpoint
    - Query invoices, payments, and credit notes for customer in date range
    - Compute opening_balance, running balance, and closing_balance
    - Return structured statement response with transaction list
    - _Requirements: 25.1, 25.2_

  - [ ] 27.2 Wire customer statement to notification delivery
    - Add "Send Statement" action that enqueues delivery via email/WhatsApp
    - _Requirements: 25.3_

  - [ ] 27.3 Implement customer statement UI in frontend
    - Create UI to select customers, date range, and trigger statement generation/sending
    - _Requirements: 25.4_

  - [ ]* 27.4 Write property tests for customer statement completeness and balance
    - **Property 25: Customer Statement Completeness and Balance**
    - **Validates: Requirements 25.1, 25.2**

- [ ] 28. P2 — Invoice Template Editor
  - [ ] 28.1 Implement invoice template persistence API
    - Create `PUT /api/v1/settings/invoice-template` endpoint
    - Persist template configuration (logo_url, primary_color, footer_text, field visibility) in `entity_settings.invoice_template`
    - Apply template config during PDF generation
    - _Requirements: 26.2, 26.3_

  - [ ] 28.2 Implement invoice template editor UI
    - Create template editor page with logo, colors, footer text, field visibility controls
    - Implement live preview of template as user edits
    - _Requirements: 26.1, 26.4_

- [ ] 29. Checkpoint — P2 Complete
  - Ensure all tests pass, ask the user if questions arise.

- [ ] 30. P3 — Dashboard Polish
  - [ ] 30.1 Add error boundaries, skeleton loaders, and empty state handling
    - Wrap each dashboard widget in React Error Boundary with fallback (error message + retry button)
    - Add skeleton loaders (pulse animation) during data fetch
    - Replace `NaN%` with `0%` or "No data" for undefined/zero values
    - Handle API errors per-widget without crashing the page
    - _Requirements: 27.1, 27.2, 27.3, 27.4_

- [ ] 31. P3 — Build Warnings Cleanup
  - [ ] 31.1 Fix all Rust clippy and frontend ESLint warnings
    - Fix all `cargo clippy` warnings across the workspace (unused imports, dead code, etc.)
    - Add `-D warnings` flag to CI Rust job
    - Fix all ESLint warnings in `zavora-erp-ui`
    - Set `--max-warnings 0` in CI frontend job
    - _Requirements: 28.1, 28.2, 28.3_

- [ ] 32. P3 — Individual Report Pages
  - [ ] 32.1 Create dedicated report page routes and components
    - Create routes: `/reports/profit-and-loss`, `/reports/balance-sheet`, `/reports/cash-flow`, `/reports/trial-balance`, `/reports/general-ledger`, `/reports/ar-ageing`, `/reports/ap-ageing`, `/reports/vat`
    - Each page: date range filter, comparison toggle, export (CSV/PDF) button
    - Calls existing `POST /api/v1/reports` with appropriate `report_type`
    - Create Reports menu/index page linking to all report pages
    - _Requirements: 29.1, 29.2, 29.3_

- [ ] 33. P3 — Document Sequences UI
  - [ ] 33.1 Implement document sequences settings page
    - Create Settings > Document Numbers tab displaying table of sequence configs
    - Show prefix, next number, and year_reset flag for each document type
    - Make fields editable inline with Save action
    - _Requirements: 30.1, 30.2_

  - [ ] 33.2 Implement sequence start number validation in API
    - `PUT /api/v1/settings/sequences` validates new start number ≥ current counter
    - Return HTTP 422 with explanation if start number is lower than last issued number
    - _Requirements: 30.3_

  - [ ]* 33.3 Write property tests for sequence start number validation
    - **Property 26: Sequence Start Number Validation**
    - **Validates: Requirements 30.3**

- [ ] 34. Final Checkpoint — All Tiers Complete
  - Ensure all tests pass, ask the user if questions arise.

## Notes

- Tasks marked with `*` are optional and can be skipped for faster MVP
- Each task references specific requirements for traceability
- Checkpoints ensure incremental validation at each priority tier boundary
- Property tests validate universal correctness properties from the design document using `proptest`
- Unit tests validate specific examples and edge cases
- The P0 tier is a hard blocker — nothing in P1+ should be started until P0 passes
- P1 tasks can be parallelized where no data dependencies exist (e.g., CORS and Secrets can run in parallel)
- Migration 006 must be applied before any other task runs

## Task Dependency Graph

```json
{
  "waves": [
    { "id": 0, "tasks": ["1.1"] },
    { "id": 1, "tasks": ["2.1", "5.1", "6.1"] },
    { "id": 2, "tasks": ["2.2", "2.3", "2.4", "2.5", "3.1", "5.2", "5.3", "5.4", "5.5"] },
    { "id": 3, "tasks": ["2.6", "3.2", "3.3", "3.4", "3.5", "4.1"] },
    { "id": 4, "tasks": ["3.6", "3.7", "3.8", "4.2", "4.3", "6.2", "6.3", "6.4"] },
    { "id": 5, "tasks": ["8.1", "9.1", "10.1", "11.1", "11.2", "12.1"] },
    { "id": 6, "tasks": ["8.2", "8.3", "8.4", "9.2", "11.3", "11.4", "12.2", "13.1", "14.1"] },
    { "id": 7, "tasks": ["13.2", "14.2", "15.1", "16.1", "16.2", "17.1"] },
    { "id": 8, "tasks": ["19.1", "20.1", "21.1", "22.1", "23.1", "24.1"] },
    { "id": 9, "tasks": ["19.2", "19.4", "19.5", "20.2", "20.3", "21.2", "22.2", "22.3", "23.2", "23.3", "24.2", "25.1"] },
    { "id": 10, "tasks": ["19.3", "25.2", "26.1", "27.1"] },
    { "id": 11, "tasks": ["27.2", "27.3", "27.4", "28.1"] },
    { "id": 12, "tasks": ["28.2", "30.1", "31.1", "32.1", "33.1"] },
    { "id": 13, "tasks": ["33.2", "33.3"] }
  ]
}
```
