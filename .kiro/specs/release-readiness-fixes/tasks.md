# Implementation Plan: Release Readiness Fixes

## Overview

This plan addresses 10 pre-release issues in priority order: BLOCKING issues first (settings save, supplier CN full reversal), then the cross-cutting pagination change, followed by independent features that can be parallelized. Shared frontend components (Skeleton, ErrorRetry, WidgetErrorBoundary, usePagination) are built before the pages that consume them.

## Tasks

- [x] 1. Fix Settings Save (BLOCKING)
  - [x] 1.1 Wire SettingsPage save mutation to existing PUT /api/v1/settings endpoint
    - Convert uncontrolled inputs to controlled state (useState initialized from query data)
    - Add useMutation hook calling `updateSettings` in `api/client.ts`
    - Show success toast on save, error toast on failure (preserving form edits)
    - Disable "Save Changes" button and show spinner while request is in-flight
    - Apply to Company, Tax, and Payment tabs
    - _Requirements: 1.1, 1.2, 1.3, 1.4, 1.5, 1.6_

- [x] 2. Fix Supplier Credit Note Full Reversal (BLOCKING)
  - [x] 2.1 Implement bill-line-copy logic in `create_supplier_credit_note` service
    - In `zavora-erp-core/src/services/supplier_credit_notes.rs`, when `req.lines.is_empty()` AND `req.applies_to_bill` is `Some(bill_id)`:
      - Fetch bill lines from `bill_lines WHERE bill_id = $1`
      - If bill lines empty, return `ErpError::ValidationFailed { message: "Bill has no lines to reverse" }`
      - Map each BillLine → CreateInvoiceLineRequest (description, quantity, unit_price, account_code, vat_treatment)
    - When explicit lines are provided, use them without copying (existing behavior)
    - Mirror pattern from `create_credit_note()` in `invoicing/credit_note.rs`
    - _Requirements: 2.1, 2.2, 2.3, 2.5_

  - [ ]* 2.2 Write property tests for supplier CN full reversal (Property 1 & 2)
    - **Property 1: Full Reversal Copies All Bill Lines**
    - **Property 2: Explicit Lines Override Full Reversal**
    - Use `proptest` to generate arbitrary bill lines and verify copy behavior
    - **Validates: Requirements 2.1, 2.2, 2.3**

  - [x] 2.3 Update BillsPage to show success notification and refresh credit note history after full reversal
    - Ensure the create-credit-note dialog passes empty lines for full reversal
    - Invalidate bill detail query on success to refresh credit note history
    - _Requirements: 2.4_

- [ ] 3. Checkpoint - Verify BLOCKING fixes
  - Ensure all tests pass, ask the user if questions arise.

- [x] 4. Add Pagination Infrastructure
  - [x] 4.1 Create shared PaginationParams extractor and PaginatedResponse type
    - Create `zavora-erp-api/src/routes/pagination.rs` module
    - Implement `PaginationParams` struct with `effective_limit()` (default 50, max 500) and `effective_offset()` (default 0, min 0)
    - Implement `PaginatedResponse<T>` struct with `data`, `total_count`, `limit`, `offset`, `has_more`
    - Register module in `zavora-erp-api/src/routes/mod.rs`
    - _Requirements: 3.1, 3.2, 3.3, 3.4_

  - [ ]* 4.2 Write property tests for pagination invariants (Property 3)
    - **Property 3: Pagination Envelope Invariants**
    - Use `proptest` on `PaginationParams` methods: limit clamped to [1,500], default 50, has_more = (offset + limit) < total_count
    - **Validates: Requirements 3.2, 3.3, 3.4**

  - [x] 4.3 Apply pagination to transactional list endpoints (invoices, bills, customers, vendors, payments, estimates, journal-entries, products, accounts)
    - Update each list handler to accept `Query(PaginationParams)`
    - Add `SELECT COUNT(*)` query before the data query
    - Add `LIMIT $n OFFSET $m` to data queries
    - Return `PaginatedResponse` envelope instead of bare `Vec<T>`
    - Affected files: `routes/invoices.rs`, `routes/bills.rs`, `routes/parties.rs`, `routes/payments.rs`, `routes/estimates.rs`, `routes/journal.rs`, `routes/catalog.rs`, `routes/accounts.rs`
    - _Requirements: 3.1, 3.4_

  - [x] 4.4 Create frontend `usePagination` hook and `PaginationControls` component
    - Create `zavora-erp-ui/src/hooks/usePagination.ts` with page/limit/offset from URL search params
    - Create `zavora-erp-ui/src/components/shared/PaginationControls.tsx` with next/previous buttons and page indicator
    - Wire into `useSearchParams` for URL sync (bookmarking, back/forward navigation)
    - _Requirements: 3.5, 3.6_

  - [x] 4.5 Integrate pagination into transactional frontend list pages
    - Update list pages to use `usePagination` hook and pass limit/offset to API calls
    - Update react-query hooks to include page params in query keys
    - Add `PaginationControls` below each DataTable
    - Update `api/client.ts` list functions to accept and pass pagination params
    - Affected pages: invoices, bills, customers, vendors, payments, estimates, journal entries, products, accounts
    - _Requirements: 3.5, 3.6_

- [ ] 5. Checkpoint - Verify pagination
  - Ensure all tests pass, ask the user if questions arise.

- [x] 6. Build Shared Frontend Components (Loading/Error States)
  - [x] 6.1 Create Skeleton, ErrorRetry, and WidgetErrorBoundary shared components
    - Create `zavora-erp-ui/src/components/shared/Skeleton.tsx` with `SkeletonCard` and `SkeletonTable` components (Tailwind animate-pulse)
    - Create `zavora-erp-ui/src/components/shared/ErrorRetry.tsx` with error icon, message, and retry button
    - Create `zavora-erp-ui/src/components/shared/WidgetErrorBoundary.tsx` as React class error boundary wrapping children with ErrorRetry fallback
    - _Requirements: 4.1, 4.2, 4.3, 4.4, 4.5_

  - [x] 6.2 Add loading and error states to DashboardPage
    - Show `SkeletonCard` placeholders while dashboard summary is loading
    - Wrap each widget/chart section in `WidgetErrorBoundary`
    - Show `ErrorRetry` with refetch callback when the main query fails
    - _Requirements: 4.1, 4.3, 4.5_

  - [x] 6.3 Add loading and error states to SettingsPage
    - Show skeleton loaders in place of form fields while settings query is loading
    - Show `ErrorRetry` when settings fetch fails, preserving any user edits already entered
    - _Requirements: 4.2, 4.4_

- [x] 7. Implement Recurring Invoices Backend
  - [x] 7.1 Implement CRUD service functions for recurring invoices
    - In `zavora-erp-core/src/services/invoicing.rs` (or new `recurring_invoices.rs`), implement:
      - `create_recurring_invoice`: validate customer belongs to entity, frequency valid, end_date >= start_date, lines non-empty; persist to `recurring_invoices` table; set next_run = start_date
      - `update_recurring_invoice`: partial update with same validation
      - `delete_recurring_invoice`: remove schedule from DB
      - `list_recurring_invoices`: return all schedules for entity
    - Return HTTP 422 with descriptive message for validation failures
    - _Requirements: 5.1, 5.2, 5.3, 5.4, 5.5, 5.6_

  - [x] 7.2 Add recurring invoice API routes
    - Register `POST /api/v1/recurring-invoices` → `create_recurring`
    - Register `GET /api/v1/recurring-invoices` → `list_recurring`
    - Register `PUT /api/v1/recurring-invoices/{id}` → `update_recurring`
    - Register `DELETE /api/v1/recurring-invoices/{id}` → `delete_recurring`
    - Wire route handlers in `zavora-erp-api/src/routes/` (new file or extend existing)
    - _Requirements: 5.1, 5.3, 5.4, 5.5_

  - [ ]* 7.3 Write property tests for recurring invoice persistence (Property 4 & 5)
    - **Property 4: Recurring Invoice Persistence Round-Trip**
    - **Property 5: Recurring Invoice Validation Rejects Invalid Inputs**
    - Use `proptest` to generate valid/invalid CreateRecurringInvoiceRequest variants
    - **Validates: Requirements 5.1, 5.2, 5.6**

- [ ] 8. Implement Estimate Draft Edit and Delete
  - [ ] 8.1 Add update and delete service functions for draft estimates
    - In `zavora-erp-core/src/services/invoicing.rs`, implement:
      - `update_estimate_draft`: check status = 'draft' (409 if not), validate, persist changes to header + line items
      - `delete_estimate_draft`: check status = 'draft' (409 if not), delete estimate and its lines
    - _Requirements: 6.1, 6.2, 6.3, 6.4_

  - [ ] 8.2 Add PUT and DELETE routes for estimates
    - Register `PUT /api/v1/estimates/{id}` → `update` handler in `zavora-erp-api/src/routes/estimates.rs`
    - Register `DELETE /api/v1/estimates/{id}` → `delete` handler
    - Map `ErpError::Conflict` to HTTP 409
    - _Requirements: 6.1, 6.2, 6.3, 6.4_

  - [ ]* 8.3 Write property test for estimate status guard (Property 6)
    - **Property 6: Non-Draft Estimate Status Guard**
    - Use `proptest` to generate estimates in non-draft statuses and verify 409 rejection
    - **Validates: Requirements 6.3, 6.4**

  - [ ] 8.4 Add Edit and Delete buttons to frontend estimate detail
    - Show "Edit" and "Delete" action buttons when estimate status is Draft
    - "Edit" opens form pre-populated with current values
    - "Delete" shows confirmation dialog, calls DELETE endpoint, navigates to list on success
    - Add `updateEstimate` and `deleteEstimate` functions to `api/client.ts`
    - _Requirements: 6.5, 6.6_

- [ ] 9. Implement Vendor Detail Page
  - [ ] 9.1 Add enriched GET /api/v1/vendors/{id} endpoint with summary stats
    - Compute total_billed (sum of bill gross_totals), total_paid (sum of vendor payments), outstanding_balance (unpaid bills - unapplied credit notes)
    - Include bill_count, payment_count, credit_note_count
    - Return `VendorDetail` response shape
    - _Requirements: 7.4_

  - [ ]* 9.2 Write property test for vendor outstanding balance computation (Property 7)
    - **Property 7: Vendor Outstanding Balance Computation**
    - Use `proptest` to generate sets of bills and credit notes, verify balance = sum(unpaid bills) - sum(posted CN totals)
    - **Validates: Requirements 7.3**

  - [ ] 9.3 Create VendorDetailPage frontend component
    - Create `zavora-erp-ui/src/pages/vendors/VendorDetailPage.tsx`
    - Info header card: name, contact, KRA PIN, payment terms
    - Balance summary card with total_billed, total_paid, outstanding
    - Tabbed sections: Bills | Payments | Credit Notes (reuse DataTable)
    - Action buttons: "New Bill", "Record Payment" navigating to respective create pages with vendor pre-selected
    - Add route in App.tsx: `/vendors/:id`
    - Wire vendor list row clicks to navigate to detail page
    - _Requirements: 7.1, 7.2, 7.3, 7.5_

- [ ] 10. Implement Notification Inbox
  - [ ] 10.1 Add notification API routes (list, mark-read, mark-all-read, unread-count)
    - Create `zavora-erp-api/src/routes/notifications.rs` with:
      - `GET /api/v1/notifications` — list with optional `unread_only` filter, paginated
      - `PATCH /api/v1/notifications/{id}/read` — mark single as read (idempotent)
      - `POST /api/v1/notifications/mark-all-read` — mark all as read for user
      - `GET /api/v1/notifications/unread-count` — return `{ count: i64 }`
    - Register routes in main router
    - _Requirements: 8.6_

  - [ ] 10.2 Create NotificationInbox frontend component
    - Create `zavora-erp-ui/src/components/layout/NotificationInbox.tsx`
    - Bell icon with unread count badge (polling every 30s via `refetchInterval`)
    - Dropdown drawer on click: notification list sorted by most recent
    - Each item shows: title, body preview, relative timestamp, read/unread indicator
    - Click notification → mark as read + navigate to related resource
    - "Mark all as read" button at top of drawer
    - Integrate into `Header.tsx` replacing existing static bell
    - _Requirements: 8.1, 8.2, 8.3, 8.4, 8.5, 8.6_

- [ ] 11. Implement Dashboard Empty State for New Tenants
  - [ ] 11.1 Extend dashboard summary API to include entity counts
    - Add `invoice_count`, `bill_count`, `payment_count` fields to `DashboardSummary` response
    - Compute via COUNT queries on invoices, bills, payments tables for the entity
    - Update `zavora-erp-api/src/routes/dashboard.rs`
    - _Requirements: 9.5_

  - [ ] 11.2 Create DashboardOnboarding component and conditional render
    - Create `zavora-erp-ui/src/pages/dashboard/DashboardOnboarding.tsx`
    - Extract `isNewTenant(summary)` utility function (invoice_count === 0 && bill_count === 0 && payment_count === 0)
    - Show welcome message + guided checklist: "Set up your company" → /settings, "Create your first customer" → /customers, "Send your first invoice" → /invoices, "Record a payment" → /payments, "Add a vendor" → /vendors
    - In DashboardPage: if `isNewTenant`, render DashboardOnboarding instead of charts
    - _Requirements: 9.1, 9.2, 9.3, 9.4, 9.5_

  - [ ]* 11.3 Write property test for dashboard empty state threshold (Property 8)
    - **Property 8: Dashboard Empty State Threshold**
    - Use `fast-check` to test `isNewTenant()` pure function with arbitrary count combinations
    - **Validates: Requirements 9.1, 9.4**

- [ ] 12. Implement Customer Statement Send Action
  - [ ] 12.1 Add POST /api/v1/customers/{id}/send-statement endpoint
    - Validate customer exists and has contact for selected channel
    - Reuse existing statement generation logic
    - Enqueue notification via `send_notification` service (Notification_Worker)
    - Return `{ status: "queued" }` on success
    - Return 422 if customer has no contact for selected channel
    - _Requirements: 10.3, 10.5_

  - [ ] 12.2 Create SendStatementDialog frontend component
    - Add "Send Statement" button alongside print/export options on customer statement page
    - Dialog shows: channel selector (Email/WhatsApp/SMS), customer contact preview
    - Disable channel options when customer lacks that contact, show prompt to update contact details
    - On confirm: call POST endpoint, show success toast
    - _Requirements: 10.1, 10.2, 10.3, 10.4, 10.5_

- [ ] 13. Final checkpoint - Full verification
  - Ensure all tests pass, ask the user if questions arise.
  - Run `cargo build --workspace` and `npx tsc -b` to verify no compile errors
  - Run `cargo test --workspace` to verify all backend tests pass

## Notes

- Tasks marked with `*` are optional and can be skipped for faster MVP
- Each task references specific requirements for traceability
- Checkpoints ensure incremental validation after each major milestone
- Property tests validate universal correctness properties from the design document
- Unit tests validate specific examples and edge cases
- BLOCKING issues (Tasks 1–2) must be completed before other work
- Pagination (Task 4) is cross-cutting and should precede feature work that uses list endpoints
- Shared components (Task 6) should be built before pages that consume them (Tasks 9–12)
- Tasks 7–12 are largely independent and can be parallelized

## Task Dependency Graph

```json
{
  "waves": [
    { "id": 0, "tasks": ["1.1", "2.1"] },
    { "id": 1, "tasks": ["2.2", "2.3"] },
    { "id": 2, "tasks": ["4.1"] },
    { "id": 3, "tasks": ["4.2", "4.3", "6.1"] },
    { "id": 4, "tasks": ["4.4", "6.2", "6.3"] },
    { "id": 5, "tasks": ["4.5", "7.1", "8.1", "11.1"] },
    { "id": 6, "tasks": ["7.2", "8.2", "9.1", "10.1", "12.1"] },
    { "id": 7, "tasks": ["7.3", "8.3", "8.4", "9.2", "9.3", "10.2", "11.2", "12.2"] },
    { "id": 8, "tasks": ["11.3"] }
  ]
}
```
