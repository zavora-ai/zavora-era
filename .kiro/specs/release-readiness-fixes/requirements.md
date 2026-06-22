# Requirements Document

## Introduction

This specification addresses 10 identified issues that would block or embarrass a production release of the Zavora ERP application. These issues were discovered during a comprehensive UI audit from the perspective of end-user use cases. They span broken save operations, backend logic gaps, missing pagination, absent loading states, stub handlers, missing CRUD operations, missing pages, and missing UI surfaces for existing backend functionality.

The fixes are prioritized into two tiers: BLOCKING (Issues 1–2, which cause data loss or 100% failure rates) and MEDIUM (Issues 3–10, which degrade quality and user experience). The system targets Kenya SMEs with KRA eTIMS compliance, built on a Rust/Axum backend with PostgreSQL + Redis, and a React/TypeScript frontend using Vite, Tailwind, and react-query.

## Glossary

- **Settings_Page**: The React settings page component (`SettingsPage.tsx`) containing tabs for Company, Tax, Payments, Posting Accounts, and Document Numbers configuration
- **Settings_API**: The backend endpoint (`PUT /api/v1/settings`) that persists entity configuration changes
- **Credit_Note_Service**: The backend service (`supplier_credit_notes.rs`) responsible for creating and posting supplier credit notes
- **Bills_Page**: The React page component (`BillsPage.tsx`) displaying vendor bills and providing credit note creation
- **Pagination_Module**: The limit/offset query parameter handling and paginated response envelope across all list endpoints
- **Frontend**: The `zavora-erp-ui` React/TypeScript single-page application
- **API_Server**: The `zavora-erp-api` Axum application serving JSON REST endpoints
- **Recurring_Invoice_Service**: The backend service (`invoicing/recurring.rs`) responsible for creating and managing recurring invoice schedules
- **Estimate_Service**: The backend service managing estimate lifecycle (draft, sent, accepted, declined, converted)
- **Vendor_Detail_Page**: A React page displaying a single vendor's complete activity history including bills, payments, and credit notes
- **Notification_Inbox**: A UI component (bell icon + drawer) in the application header for consuming in-app notifications
- **Notification_Worker**: The background service (`notification_worker.rs`) that delivers notifications via configured channels
- **Dashboard_Page**: The React dashboard page component displaying summary metrics, charts, and quick actions
- **Customer_Statement_Page**: The React page for generating and viewing customer account statements
- **Core_Engine**: The `zavora-erp-core` Rust library containing business logic and services

---

## Requirements

### Requirement 1: Settings Save Functionality

**User Story:** As a business owner, I want the Settings page Save button to persist my changes for Company, Tax, and Payment tabs, so that configuration edits are not silently discarded on page refresh.

#### Acceptance Criteria

1. WHEN the user edits Company settings and clicks "Save Changes", THE Settings_Page SHALL call the Settings_API with the updated company information and persist changes to the database
2. WHEN the user edits Tax settings and clicks "Save Changes", THE Settings_Page SHALL call the Settings_API with the updated tax configuration and persist changes to the database
3. WHEN the user edits Payment settings and clicks "Save Changes", THE Settings_Page SHALL call the Settings_API with the updated payment preferences and persist changes to the database
4. WHEN the Settings_API returns a success response, THE Settings_Page SHALL display a success toast notification confirming the save
5. IF the Settings_API returns an error, THEN THE Settings_Page SHALL display an error toast notification with the failure reason and retain the user's unsaved edits in the form
6. WHILE the save request is in-flight, THE Settings_Page SHALL disable the "Save Changes" button and display a loading indicator to prevent duplicate submissions

---

### Requirement 2: Supplier Credit Note Full Reversal

**User Story:** As an accountant, I want to create a full-reversal supplier credit note from a posted bill without manually entering line items, so that the credit note workflow matches the existing customer credit note behavior.

#### Acceptance Criteria

1. WHEN the user creates a supplier credit note with empty lines (indicating full reversal), THE Credit_Note_Service SHALL copy all line items from the original bill to the credit note
2. WHEN a full-reversal supplier credit note is created, THE Credit_Note_Service SHALL set each copied line's quantity and amount to match the original bill lines
3. WHEN the user provides explicit line items for a supplier credit note, THE Credit_Note_Service SHALL use the provided lines without copying from the original bill
4. WHEN a supplier credit note is created via full reversal, THE Bills_Page SHALL display a success notification and refresh the bill's credit note history
5. IF the original bill has no line items to copy, THEN THE Credit_Note_Service SHALL return a descriptive error indicating the bill has no lines to reverse

---

### Requirement 3: List Endpoint Pagination

**User Story:** As a user with growing business data, I want all list pages to load data in pages rather than fetching all records at once, so that the application remains responsive as data volumes grow.

#### Acceptance Criteria

1. THE API_Server SHALL accept optional `limit` and `offset` query parameters on all list endpoints (invoices, bills, customers, vendors, payments, estimates, journal entries, products, accounts)
2. WHEN `limit` is not specified, THE API_Server SHALL apply a default limit of 50 records
3. WHEN `limit` exceeds 500, THE API_Server SHALL cap the limit at 500 records
4. THE API_Server SHALL return a response envelope containing `data` (the results array), `total_count`, `limit`, `offset`, and `has_more` fields
5. WHEN the Frontend renders a list page, THE Frontend SHALL display pagination controls (next/previous or page numbers) and pass limit/offset to the API
6. WHEN the user navigates between pages, THE Frontend SHALL update the URL query parameters to allow bookmarking and browser back/forward navigation

---

### Requirement 4: Loading and Error States for Legacy Pages

**User Story:** As a user, I want the Dashboard and Settings pages to show loading indicators while data is being fetched and error states with retry options when requests fail, so that the UI is informative regardless of network conditions.

#### Acceptance Criteria

1. WHILE data is loading on the Dashboard_Page, THE Frontend SHALL display skeleton loaders in place of data widgets and charts
2. WHILE data is loading on the Settings_Page, THE Frontend SHALL display skeleton loaders in place of form fields
3. WHEN an API call fails on the Dashboard_Page, THE Frontend SHALL display an error message with a "Retry" button within the affected widget
4. WHEN an API call fails on the Settings_Page, THE Frontend SHALL display an error message with a "Retry" button and preserve any user edits already in the form
5. THE Frontend SHALL wrap each Dashboard widget in an error boundary so that a single widget failure does not crash the entire page

---

### Requirement 5: Recurring Invoices Backend Persistence

**User Story:** As a business owner, I want recurring invoice schedules created through the UI to be persisted in the database, so that the scheduler can generate invoices on the configured cadence.

#### Acceptance Criteria

1. WHEN a user submits a new recurring invoice schedule via `POST /api/v1/recurring-invoices`, THE Recurring_Invoice_Service SHALL validate the request and persist the schedule to the database
2. WHEN the recurring invoice is persisted, THE Recurring_Invoice_Service SHALL store the customer, line items, frequency, start date, end date (if provided), and next run date
3. WHEN a user retrieves recurring invoices via `GET /api/v1/recurring-invoices`, THE API_Server SHALL return all persisted schedules for the authenticated entity
4. WHEN a user updates an existing recurring invoice via `PUT /api/v1/recurring-invoices/{id}`, THE Recurring_Invoice_Service SHALL validate and persist the changes
5. WHEN a user deletes a recurring invoice via `DELETE /api/v1/recurring-invoices/{id}`, THE Recurring_Invoice_Service SHALL remove the schedule from the database
6. IF the request body fails validation (missing customer, invalid frequency, end date before start date), THEN THE Recurring_Invoice_Service SHALL return HTTP 422 with a descriptive validation error

---

### Requirement 6: Estimate Draft Edit and Delete

**User Story:** As an accountant, I want to edit or delete draft estimates, so that typos can be corrected and abandoned estimates can be removed without declining and re-creating them.

#### Acceptance Criteria

1. WHEN a user submits an update to a draft estimate via `PUT /api/v1/estimates/{id}`, THE Estimate_Service SHALL validate and persist the changes to the estimate header and line items
2. WHEN a user deletes a draft estimate via `DELETE /api/v1/estimates/{id}`, THE Estimate_Service SHALL remove the estimate and its line items from the database
3. IF a user attempts to edit an estimate that is not in Draft status, THEN THE Estimate_Service SHALL return HTTP 409 with a message indicating only draft estimates can be edited
4. IF a user attempts to delete an estimate that is not in Draft status, THEN THE Estimate_Service SHALL return HTTP 409 with a message indicating only draft estimates can be deleted
5. WHEN a draft estimate is opened in the Frontend, THE Frontend SHALL display "Edit" and "Delete" action buttons
6. WHEN the user clicks "Edit", THE Frontend SHALL present the estimate form pre-populated with current values for modification

---

### Requirement 7: Vendor Detail Page

**User Story:** As an accountant, I want a vendor detail page showing bills, payments, and credit notes in one place, so that I have the same visibility into vendor activity that I have for customers.

#### Acceptance Criteria

1. WHEN a user clicks on a vendor in the vendors list, THE Frontend SHALL navigate to a vendor detail page displaying the vendor's information header (name, contact, tax PIN, payment terms)
2. THE Vendor_Detail_Page SHALL display a tabbed or sectioned view with the vendor's bills, payments, and supplier credit notes
3. THE Vendor_Detail_Page SHALL display the vendor's current outstanding balance (total unpaid bills minus unapplied credit notes)
4. WHEN the API_Server receives `GET /api/v1/vendors/{id}`, THE API_Server SHALL return the vendor record along with summary statistics (total billed, total paid, outstanding balance)
5. THE Vendor_Detail_Page SHALL provide action buttons to create a new bill or payment directly from the vendor context

---

### Requirement 8: Notification Inbox UI

**User Story:** As a user, I want a notification bell icon in the application header with a drawer showing my unread notifications, so that I can consume in-app notifications without navigating away from my current page.

#### Acceptance Criteria

1. THE Frontend SHALL display a bell icon in the application header showing the count of unread notifications as a badge
2. WHEN the user clicks the bell icon, THE Frontend SHALL open a drawer/dropdown listing recent notifications sorted by most recent first
3. WHEN a notification is displayed in the drawer, THE Frontend SHALL show the notification title, message preview, timestamp, and read/unread status
4. WHEN the user clicks a notification, THE Frontend SHALL mark the notification as read and navigate to the relevant resource (invoice, payment, bill) if applicable
5. WHEN the user clicks "Mark all as read", THE Frontend SHALL mark all visible notifications as read and update the unread badge count
6. THE Frontend SHALL fetch notifications from `GET /api/v1/notifications` and poll or use server-sent events for real-time updates

---

### Requirement 9: Dashboard Empty State for New Tenants

**User Story:** As a new user, I want the dashboard to show an onboarding guide when no data exists, so that I understand how to get started rather than seeing a page of zeros.

#### Acceptance Criteria

1. WHEN the Dashboard_Page loads and the tenant has zero invoices, zero bills, and zero payments, THE Frontend SHALL display an onboarding empty state instead of charts with zero values
2. THE onboarding empty state SHALL display a welcome message and a guided checklist of first actions (e.g., "Create your first customer", "Send your first invoice", "Record a payment")
3. WHEN the user clicks a checklist item, THE Frontend SHALL navigate to the relevant page for that action
4. WHEN the tenant has at least one invoice, bill, or payment, THE Frontend SHALL display the standard dashboard with charts and metrics
5. THE Frontend SHALL determine the empty state condition using the dashboard summary API response (total counts of key entities)

---

### Requirement 10: Customer Statement Send Action

**User Story:** As a business owner, I want a "Send Statement" button on the customer statement page, so that I can email or WhatsApp the statement to the customer directly from the UI.

#### Acceptance Criteria

1. WHEN a customer statement is generated, THE Frontend SHALL display a "Send Statement" button alongside the existing print/export options
2. WHEN the user clicks "Send Statement", THE Frontend SHALL present a confirmation dialog showing the customer's contact details (email and/or phone) and delivery channel options
3. WHEN the user confirms sending, THE API_Server SHALL enqueue a notification via the Notification_Worker to deliver the statement to the customer using the selected channel
4. WHEN the statement is queued for delivery, THE Frontend SHALL display a success notification confirming the statement was sent
5. IF the customer has no email or phone configured for the selected channel, THEN THE Frontend SHALL disable that channel option and display a message prompting the user to update customer contact details
