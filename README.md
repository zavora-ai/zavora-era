# Zavora ERP — Core ERP Engine

A full-featured double-entry accounting engine built in Rust, targeting Kenyan
SMEs with Wave Apps feature parity plus Kenya-specific compliance (KRA iTax,
M-Pesa, PAYE/NSSF/SHA/HELB, WHT). Ships an immutable double-entry ledger, a
multi-tenant REST API, and a React web UI.

> **Project status.** Recent changes are logged in [`CHANGELOG.md`](CHANGELOG.md);
> outstanding work (procurement, posting-group matrices, CI/containerization,
> observability, …) is tracked in [`REMAINING.md`](REMAINING.md).

## Architecture

```
zavora-erp-core/    — Library crate: domain models, business logic, DB operations
zavora-erp-api/     — Binary crate: Axum REST API server + hourly scheduler
zavora-erp-ui/      — React + Vite + TypeScript web client
migrations/         — PostgreSQL schema + immutability triggers
scripts/qbo/        — QuickBooks → Zavora import/replay/compare tooling
```

## Prerequisites

- Rust 1.75+
- PostgreSQL 15+
- Redis 7+
- Node 18+ (for the web UI)

## Quick Start

```bash
# 1. Copy environment file
cp .env.example .env

# 2. Start Postgres + Redis (Docker)
docker compose up -d

# 3. Run the server (migrations auto-apply on startup)
cargo run --bin zavora-erp-api

# Server starts on http://localhost:8080 (configurable via BIND_ADDR)

# 4. (optional) Run the web UI
cd zavora-erp-ui && npm install && npm run dev
```

> **Local dev defaults.** `docker-compose.yml` publishes Postgres on host port
> `5433` and Redis on `6380`. The `.env` `DATABASE_URL`/`REDIS_URL` already point
> at these. The web UI (`zavora-erp-ui`, Vite) runs on `http://localhost:3000`
> and proxies `/api` to the API server.

### Configuration (`.env`)

| Variable | Purpose |
|---|---|
| `DATABASE_URL` | PostgreSQL connection string |
| `REDIS_URL` | Redis connection string (sessions, audit stream, caches) |
| `BIND_ADDR` | API listen address (default `0.0.0.0:8080`) |
| `APP_ENV` | `development` / `production` (gates secure-cookie + prod behaviour) |
| `RUST_LOG` | Tracing filter, e.g. `info,zavora_erp_api=debug` |
| `JWT_ACCESS_SECRET` / `JWT_REFRESH_SECRET` | Signing secrets (required; startup fails fast if missing) |
| `JWT_ACCESS_TTL_SECS` / `JWT_REFRESH_TTL_SECS` | Token lifetimes |

## Authentication & Multi-Tenancy

Each **tenant** (entity) is a self-contained company: its own chart of accounts,
masters, ledger and settings. Tenants are created by signup; all other data is
scoped per-request to the authenticated user's `entity_id`.

- **Sign up** provisions a new tenant + owner user and seeds the Kenya Standard
  chart of accounts and default settings.
- **Login** issues a short-lived **access token** (JWT, kept in memory by the UI)
  and a **refresh token** stored in an **httpOnly, SameSite=Strict cookie**
  (`era_refresh`) — never in localStorage. Passwords are hashed with Argon2id.
- Every protected route is gated by global auth middleware; master-data writes
  additionally enforce role checks.

```
POST /api/v1/auth/signup    — Create a tenant + owner (organization_name, organization_type, kra_pin?, email, display_name, password)
POST /api/v1/auth/login     — Authenticate; returns access token + sets refresh cookie
POST /api/v1/auth/refresh   — Exchange the refresh cookie for a new access token
POST /api/v1/auth/logout    — Revoke the refresh token and clear the cookie
POST /api/v1/auth/register  — (deprecated) legacy user creation
```

## Chart of Accounts

The engine ships a **Kenya Standard** COA template (`ledger::coa_template`) seeded
on tenant signup. It also supports importing an external chart — the
`scripts/qbo/` tooling replays a QuickBooks export (e.g. the "Craig's Design and
Landscaping" sample) into a tenant, mapping QuickBooks account types onto Zavora
account types and **repointing the posting setup** (AR/AP, default bank, default
sales/purchase, rounding) at the imported accounts. Because both sets can
coexist, a tenant's live chart may contain more accounts than the bare Kenya
template; GL determination always resolves through the per-tenant
`PostingSetup`, never hardcoded codes.

## API Endpoints

All endpoints are under `/api/v1` and require a `Bearer` access token unless
noted. List endpoints are paginated (`?limit=&offset=`, default 50 / max 500)
and return `{ data, total_count, limit, offset, has_more }`.

### Health & Dashboard
- `GET /health` — liveness (checks Postgres + Redis); **no auth**
- `GET /dashboard` — financial overview summary

### Users & Audit
- `GET|POST /users`, `PUT /users/{id}` — user management (list, invite, role/status)
- `GET /audit` — audit trail (paginated; actor names/emails resolved)
- `GET /audit/{object_type}/{object_id}` — history for one object

### Chart of Accounts
- `GET|POST /accounts`, `GET|PUT /accounts/{code}` — chart of accounts
- `POST /accounts/seed` — seed the Kenya Standard template

### Onboarding & Settings
- `POST /opening-balances` — enter opening trial balance
- `GET|PUT /settings` — entity configuration (branding, tax, payments, sequences, posting)

### Fiscal Periods
- `GET|POST /periods` — list / generate a year's periods
- `POST /periods/{id}/close` · `POST /periods/{id}/reopen` — soft/hard close & reopen
- `POST /periods/year-end-close` — atomic year-end close (body `{"fiscal_year": 2025}`)

### Journal Entries
- `POST /journal-entries` — create and post · `POST /journal-entries/validate` — validate only
- `GET /journal-entries/{id}` · `POST /journal-entries/{id}/reverse` — detail & reversing entry
- `GET|POST /recurring-journals`, `DELETE /recurring-journals/{id}`, `POST /recurring-journals/run`

### Parties & Catalog
- `GET|POST /customers`, `GET|PUT /customers/{id}` — customers
- `GET /customers/{id}/statement` · `POST /customers/{id}/send-statement`
- `GET|POST /vendors`, `GET|PUT /vendors/{id}` — vendors
- `GET|POST /employees`, `GET|PUT /employees/{id}` — employees
- `GET|POST /products`, `GET|PUT /products/{id}` — products/services

### Sales (AR)
- `GET|POST /invoices`, `GET /invoices/{id}` — invoices
- `POST /invoices/{id}/post` · `/send` · `/credit-note` · `/write-off` · `/etims-transmit`
- `GET|POST /estimates`, `GET /estimates/{id}` — quotes
- `POST /estimates/{id}/send` · `/accept` · `/decline` · `/convert`
- `GET|POST /recurring-invoices`, `PUT|DELETE /recurring-invoices/{id}`

### Purchases (AP)
- `GET|POST /bills`, `GET /bills/{id}` — bills
- `POST /bills/{id}/approve` · `POST /bills/{id}/post`
- `GET|POST /supplier-credit-notes`, `GET /supplier-credit-notes/{id}`

### Payments
- `GET|POST /payments`, `GET /payments/{id}` — record / view payments (receipt preview)
- `POST /payments/apply` — apply an unapplied payment to documents
- `POST /payments/mpesa-stk-push` — initiate an M-Pesa STK Push for an invoice
- `POST /payments/mpesa-callback` — M-Pesa Daraja webhook (idempotent)

### Banking & Transactions
- `GET|POST /bank-accounts`, `DELETE /bank-accounts/{id}`
- `POST /bank/import` — import a statement (CSV / MT940 / OFX) into the categorisation queue (idempotent)
- `POST /bank/confirm-match` — confirm a suggested match
- `GET /bank/reconciliations`, `POST /bank/reconciliations/compute|complete`, `POST /bank/reconcile/{id}`
- `GET /transactions` — categorisation queue
- `POST /transactions/{id}/categorise|split|exclude`, `POST /transactions/merge`
- `POST /receipts/capture` · `POST /receipts/confirm` — receipt (OCR) capture

### Inventory
- `GET|POST /inventory`, `POST /inventory/receive|issue|adjust` — items + stock movements/stock-take

### Fixed Assets
- `GET|POST /assets` — register / list assets
- `POST /assets/depreciation/run` — depreciation catch-up (optional `?date=`, defaults to today)

Depreciation is an **idempotent catch-up**: each run books every month still due
(from the asset's `depreciated_through` up to the target month) and cannot
double-post a period. The scheduler runs it automatically for **all tenants** at
month rollover, so a manual run is only needed to book early.

### Payroll
- `POST /payroll/run` — run payroll · `POST /payroll/{id}/approve` · `/post` · `/paid`

### Tax
- `GET|POST /tax-filings`, `POST /tax-filings/{id}/remit` — VAT/PAYE/WHT filing + remittance
- `GET|PUT /wht-rates` — view / configure withholding-tax rates

### FX
- `GET|POST /fx-rates` — exchange rates · `POST /fx/revaluation` — period-end revaluation (auto-reversing)

### Reports
- `POST /reports` — generate any report (see catalogue below) · `POST /reports/export` — CSV/Excel
- `GET|PUT /budgets` — budgets (drives Budget-vs-Actual)
- `GET|POST /custom-reports`, `GET|DELETE /custom-reports/{id}`, `GET /custom-reports/{id}/run`
- `GET /dimensions`, `POST /dimension-types`, `POST /dimension-values` — analytical accounting
- `GET /consolidation/entities` · `POST /consolidation/trial-balance` — multi-entity consolidation
- `GET|POST /report-schedules`, `DELETE /report-schedules/{id}` — scheduled/emailed reports

**Report catalogue** (`report_type` for `POST /reports`): `TrialBalance`,
`BalanceSheet`, `ProfitAndLoss`, `CashFlow`, `CashFlowDirect`, `EquityChanges`,
`GlDetail`, `ArAgeing`, `ApAgeing`, `CustomerStatement`, `VendorStatement`,
`CustomerPaymentHistory`, `IncomeByCustomer`, `ExpenseByVendor`,
`InventoryValuation`, `FixedAssetRegister`, `BudgetVsActual`,
`DimensionalAnalysis`, `BankReconSummary`, `PayrollSummary`, `PayeP10`,
`WhtCertificate`, `VatReturn`, `SalesTaxSummary`.

### Notifications
- `GET /notifications`, `GET /notifications/unread-count`
- `POST /notifications/{id}/read`, `POST /notifications/mark-all-read`

### Agent API (Agentic Layer)
- `POST /agent/post` — post a journal entry from an agent
- `POST /agent/report` — run a report from an agent

### Bank statement CSV format
The first row is treated as a header when it contains any of `date`,
`description`, `amount`, or `balance` (case-insensitive). Columns are
**positional** (not matched by name). Dates accept `YYYY-MM-DD`, `DD/MM/YYYY`, or
`MM/DD/YYYY`.

| Columns | Layout | Notes |
|---|---|---|
| 3 | `date, description, amount` | negative = money out (debit), positive = money in (credit) |
| 4 | `date, description, amount, balance` | as above, with running balance |
| 5+ | `date, description, debit, credit, balance` | explicit debit/credit columns; leave one blank |

Every data row must have a debit or credit, or the whole file is rejected (no
partial imports). Re-importing the same file is rejected and duplicate lines are
skipped. Example (5-column):

```csv
Date,Description,Debit,Credit,Balance
2026-06-01,Customer deposit,,1000.00,1000.00
2026-06-02,Bank charge,50.00,,950.00
```

## Background Scheduler

The API runs an hourly tick that processes, **across all tenants**: recurring
invoices, invoice reminders, scheduled/emailed reports, recurring journals, and
month-end asset depreciation. All jobs are idempotent and advance their own
next-run state transactionally.

## Kenya-Specific Features

- **PAYE** — Progressive tax bands per KRA Finance Act 2024
- **NSSF** — Tier I & II (6% up to KES 36,000)
- **SHA** — 2.75% of gross (replaces NHIF)
- **Housing Levy** — 1.5% employee + 1.5% employer
- **HELB** — Per-employee deduction
- **WHT** — Auto-computed from vendor category & residency (2–30%; rates seeded in `wht_rates`)
- **VAT** — Standard 16%, Petroleum 8%, Zero-rated, Exempt
- **KRA Asset Classes** — Declining balance (37.5%, 30%, 25%, 12.5%)
- **M-Pesa Daraja** — STK Push payment links on invoices
- **iTax Data Export** — VAT return preparation

These calculations are covered by golden-value tests (see Testing).

## Immutability Guarantees (DB-Level)

- Posted journal entries cannot be mutated (Postgres trigger)
- Hard-closed periods reject all new lines, except system year-end-close / opening entries (Postgres trigger)
- Non-negative inventory enforcement (Postgres trigger)
- Audit trail emitted inside DB transactions

## Testing

```bash
# All workspace tests
cargo test --workspace

# Core unit + integration tests (DB-backed tests skip gracefully without a database)
cargo test -p zavora-erp-core

# UI typecheck
cd zavora-erp-ui && npx tsc --noEmit
```

DB-backed integration/property tests use `TEST_DATABASE_URL` / `TEST_REDIS_URL`
when set, otherwise the docker-compose defaults; they skip (rather than fail)
when infrastructure is unreachable.

## License

Proprietary — Zavora Technologies Ltd
