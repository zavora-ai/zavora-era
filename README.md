# Zavora ERA — Core ERP Engine

A full-featured double-entry accounting engine built in Rust, targeting Kenyan SMEs with Wave Apps feature parity plus Kenya-specific compliance (KRA iTax, M-Pesa, PAYE/NSSF/NHIF/HELB, WHT).

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

# 2. Create database
createdb zavora_era

# 3. Run the server (migrations auto-apply on startup)
cargo run --bin zavora-erp-api

# Server starts on http://localhost:8080
```

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
