# Zavora ERP — Core ERP Engine

A full-featured double-entry accounting engine built in Rust, targeting Kenyan SMEs with Wave Apps feature parity plus Kenya-specific compliance (KRA iTax, M-Pesa, PAYE/NSSF/NHIF/HELB, WHT).

> **Project status.** Recent changes are logged in [`CHANGELOG.md`](CHANGELOG.md);
> outstanding work (procurement, posting-group matrices, CI/containerization,
> observability, …) is tracked in [`REMAINING.md`](REMAINING.md).

## Architecture

```
zavora-erp-core/    — Library crate: domain models, business logic, DB operations
zavora-erp-api/     — Binary crate: Axum REST API server
migrations/         — PostgreSQL schema + immutability triggers
```

## Prerequisites

- Rust 1.75+
- PostgreSQL 15+
- Redis 7+

## Quick Start

```bash
# 1. Copy environment file
cp .env.example .env

# 2. Start Postgres + Redis (Docker)
docker compose up -d

# 3. Run the server (migrations auto-apply on startup)
cargo run --bin zavora-erp-api

# Server starts on http://localhost:8080 (configurable via BIND_ADDR)
```

> **Local dev defaults.** `docker-compose.yml` publishes Postgres on host port
> `5433` and Redis on `6380`. The `.env` `DATABASE_URL`/`REDIS_URL` already point
> at these. The web UI (`zavora-erp-ui`, Vite) runs on
> `http://localhost:3000` and proxies `/api` to the API server.

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

### Health
- `GET /health`

### Dashboard
- `GET /api/v1/dashboard` — Financial overview summary

### Chart of Accounts
- `GET /api/v1/accounts` — List accounts
- `POST /api/v1/accounts` — Create account
- `GET /api/v1/accounts/{code}` — Get account
- `PUT /api/v1/accounts/{code}` — Update account

### Fiscal Periods
- `GET /api/v1/periods` — List periods
- `POST /api/v1/periods` — Generate periods for a year
- `POST /api/v1/periods/{id}/close` — Close a period
- `POST /api/v1/periods/{id}/reopen` — Reopen a soft-closed period
- `POST /api/v1/periods/year-end-close` — Execute year-end close for a fiscal year (body: `{"fiscal_year": 2025}`)

### Journal Entries
- `POST /api/v1/journal-entries` — Create and post
- `POST /api/v1/journal-entries/validate` — Validate without posting

### Customers & Vendors
- `GET /api/v1/customers` — List customers
- `POST /api/v1/customers` — Create customer
- `GET /api/v1/vendors` — List vendors
- `POST /api/v1/vendors` — Create vendor

### Invoicing
- `POST /api/v1/invoices` — Create invoice
- `POST /api/v1/invoices/{id}/post` — Post to GL
- `POST /api/v1/invoices/{id}/send` — Send to customer

### Bills (AP)
- `POST /api/v1/bills` — Create bill
- `POST /api/v1/bills/{id}/approve` — Approve bill

### Payments
- `POST /api/v1/payments` — Record payment
- `POST /api/v1/payments/mpesa-callback` — M-Pesa Daraja webhook

### Banking
- `POST /api/v1/bank/import` — Import a bank statement (CSV / MT940 / OFX) into the categorisation queue

Bank import is idempotent: re-importing the same file for a bank account is
rejected, and individual duplicate lines are skipped.

**CSV format.** The first row is treated as a header when it contains any of
`date`, `description`, `amount`, or `balance` (case-insensitive). Columns are
**positional** (not matched by name). Dates accept `YYYY-MM-DD`, `DD/MM/YYYY`,
or `MM/DD/YYYY`. Supported layouts:

| Columns | Layout | Notes |
|---|---|---|
| 3 | `date, description, amount` | negative amount = money out (debit), positive = money in (credit) |
| 4 | `date, description, amount, balance` | as above, with running balance |
| 5+ | `date, description, debit, credit, balance` | explicit debit/credit columns; leave one blank |

There is no separate reference column — put identifying text in `description`.
Every data row must have a debit or credit, or the whole file is rejected (no
partial imports). Example (5-column):

```csv
Date,Description,Debit,Credit,Balance
2026-06-01,Customer deposit,,1000.00,1000.00
2026-06-02,Bank charge,50.00,,950.00
```

### Fixed Assets
- `POST /api/v1/assets` — Register an asset
- `POST /api/v1/assets/depreciation/run` — Depreciation catch-up (optional `?date=`, defaults to today)

Depreciation is an **idempotent catch-up**: each run books every month still due
(from the asset's `depreciated_through` up to the target month) and cannot
double-post a period. The hourly scheduler runs it automatically for **all
tenants** at month rollover, so a manual run is only needed to book early.

### Payroll
- `POST /api/v1/payroll/run` — Run payroll
- `POST /api/v1/payroll/{id}/approve` — Approve pay run
- `POST /api/v1/payroll/{id}/post` — Post to GL

### Reports
- `POST /api/v1/reports` — Generate any report type

### Agent API (Agentic Layer)
- `POST /api/v1/agent/post` — Post journal entry from agent
- `POST /api/v1/agent/report` — Run report from agent

### Settings
- `GET /api/v1/settings` — Get configuration
- `PUT /api/v1/settings` — Update configuration

## Kenya-Specific Features

- **PAYE** — Progressive tax bands per KRA Finance Act 2024
- **NSSF** — Tier I & II (6% up to KES 36,000)
- **SHA** — 2.75% of gross (replaces NHIF)
- **Housing Levy** — 1.5% employee + 1.5% employer
- **HELB** — Per-employee deduction
- **WHT** — Auto-computed from vendor category (5–30%)
- **VAT** — Standard 16%, Petroleum 8%, Zero-rated, Exempt
- **KRA Asset Classes** — Declining balance (37.5%, 30%, 25%, 12.5%)
- **M-Pesa Daraja** — STK Push payment links on invoices
- **iTax Data Export** — VAT return preparation

## Immutability Guarantees (DB-Level)

- Posted journal entries cannot be mutated (Postgres trigger)
- Hard-closed periods reject all new lines (Postgres trigger)
- Non-negative inventory enforcement (Postgres trigger)
- Audit trail emitted inside DB transactions

## License

Proprietary — Zavora Technologies Ltd
