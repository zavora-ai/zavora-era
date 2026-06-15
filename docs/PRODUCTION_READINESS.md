# Production Readiness — Remaining Work

Status of Zavora ERP toward full production use. Items are grouped by area and
marked with priority: **P0** (blocker for any real use), **P1** (required before
go-live), **P2** (important, fast-follow), **P3** (nice to have).

Legend: ✅ done · 🟡 partial · ⬜ not started

---

## Completed in recent work

- ✅ Estimates feature (missing `estimate_lines` table) — fixed (migration 003)
- ✅ Bank account GL column mismatch (`gl_account_code` → `gl_account`)
- ✅ UI ↔ API auth alignment: identity headers, `/auth/login`, `/users`, Login page, route guard
- ✅ Missing routes used by the UI: `DELETE /bank-accounts/{id}`, `POST /payments/mpesa-stk-push`
- ✅ Agent endpoints now require auth + role
- ✅ M-Pesa webhook idempotency (unique receipt claim; migration 004)
- ✅ Atomic draft creation for invoices and estimates (single transaction)
- ✅ Posting setup (Phase 1 resolver + Phase 3 editable UI, live reload) — see `POSTING_SETUP.md`

---

## 1. Data integrity & accounting correctness

- ⬜ **P0 — Transaction atomicity for ledger-coupled flows.** `record_payment`,
  `post_invoice`, `create_credit_note`, and `apply_unapplied_payment` perform
  multiple autocommit writes plus a self-contained journal transaction. A failure
  mid-sequence can leave torn state (payment without balance update, balances
  reduced without a posted journal). Refactor `journal::create_and_post` into a
  transaction-aware variant and thread one transaction through each operation
  (FX gain/loss and reminder side-effects can run post-commit).
- ⬜ **P1 — Rounding policy.** Line VAT is computed without explicit rounding and
  the journal balance check uses exact `Decimal` equality. Define a 2dp rounding
  policy for monetary fields and add a rounding line / tolerance so VAT-derived
  imbalances cannot block posting.
- ⬜ **P1 — Correct unapplied-payments accounts.** Default `3050` is not in the
  CoA; split into customer vs vendor unapplied accounts (Phase 2) and set valid
  defaults (e.g. `1700`/`9100`, `3600`/`9110`).
- ⬜ **P1 — Document numbering.** Numbers are consumed even on failed inserts
  (gaps) and the `year_reset` flag is never applied (counter never resets per
  year). KRA/ETR contexts often require gapless, year-scoped numbering.
- 🟡 **P1 — Bill posting (`post_bill`) review.** Confirm VAT input is posted and
  routed through `posting.vat_input`; verify WHT and expense lines balance.
- ⬜ **P2 — Supplier credit notes store no line items** (only `gross_total`).
- ⬜ **P2 — Statutory payroll/tax accuracy.** PAYE relief handling omits
  SHA/insurance relief; PAYE not rounded. Requires tax-professional validation
  before filing-grade use.

## 2. Security & multi-tenancy

- ⬜ **P0 — Real authentication.** Login currently resolves identity by email and
  the API trusts `X-User-*` headers (gateway model). Implement verified auth
  (JWT/OIDC or password hashing + sessions) and stop trusting raw headers from
  the browser.
- ⬜ **P0 — Per-request tenant scoping.** All data is scoped to the startup
  `ENTITY_ID`; the authenticated `ctx.entity_id` is ignored. Either commit to
  single-tenant-per-process explicitly, or scope every query by `ctx.entity_id`
  for true multi-tenancy.
- ⬜ **P1 — CORS lockdown.** `CorsLayer::permissive()` must be restricted to known
  origins in production.
- ⬜ **P1 — M-Pesa callback authenticity.** Validate Daraja callbacks (IP
  allowlist / signature) and correlate via `CheckoutRequestID`/`AccountReference`
  rather than a client-supplied `invoice_id`.
- ⬜ **P1 — Secrets & TLS.** Move DB/Redis/provider credentials to a secret store;
  terminate TLS; never commit real secrets (`.env` is gitignored — keep it so).
- ⬜ **P2 — Rate limiting and request size limits** on public endpoints
  (login, webhooks, uploads).

## 3. Posting groups (Phase 2 / 4)

- ⬜ **P2 — VAT Posting Setup matrix.** VAT Business × VAT Product groups →
  rate + output/input accounts; wire into invoice/bill line VAT resolution.
- ⬜ **P2 — General Posting Setup matrix.** General Business × General Product
  groups → sales / purchase / COGS accounts.
- ⬜ **P2 — Customer / Vendor / Inventory posting groups** → receivables/payables
  and inventory/COGS/variance accounts.
- ⬜ **P2 — Setup UI** for the matrices (extends Settings → Posting Accounts).
- ⬜ **P3 — Migrate master-record account fields** onto posting-group references
  with backward compatibility (Phase 4).

## 4. Functional gaps

- ⬜ **P1 — Void / delete flows.** No void route (status `Voided` exists unused);
  no delete for draft invoices/bills; customers/vendors/products only toggle
  `is_active`.
- ⬜ **P1 — Pagination** on list endpoints (and the spec's paginated GL detail).
- 🟡 **P1 — User management UI.** Backend `/users` exists; no screen to invite
  users or assign roles.
- 🟡 **P1 — Settings save.** Only the Posting Accounts tab persists. The Company /
  Tax / Payments / Document Numbers tabs render values but their Save button is
  not wired; and non-posting settings need full live reload (`reload_config` only
  refreshes posting today).
- ⬜ **P2 — Bank statement import** (CSV/MT940/OFX) is a stub.
- ⬜ **P2 — M-Pesa STK Push** gateway integration not implemented (endpoint
  returns a clear "not configured" error).
- 🟡 **P2 — OCR receipt capture / notification delivery** (email/WhatsApp/SMS):
  verify real providers are wired vs. queue-only.
- ⬜ **P3 — Dashboard polish** (e.g. "NaN%" on empty data), empty/loading states,
  form validation, error boundaries.

## 5. Quality, testing & operations

- ⬜ **P0 — Automated tests.** No unit/integration tests exist. Accounting logic
  (balancing, posting, payroll, FX, period close) needs a test suite before
  production. Add property/golden tests for journal balancing.
- ⬜ **P1 — CI pipeline:** build, test, `cargo clippy`, migration check, UI build/lint.
- ⬜ **P1 — Containerization & deploy:** Dockerfiles for API and UI, production
  compose/manifests, reverse proxy, health/readiness probes.
- ⬜ **P1 — Backups & migration safety:** DB backup/restore runbook; review
  destructive migrations; consider down-migrations.
- ⬜ **P2 — Observability:** structured logs, metrics, request tracing; durable
  audit consumer for the Redis audit stream.
- ⬜ **P2 — Performance:** index review, eliminate N+1 queries on detail views.
- ⬜ **P3 — Clean up build warnings** (unused imports/variables) and enable
  `-D warnings` in CI.

---

## Suggested sequence to go-live

1. **P0 correctness/security:** ledger transaction atomicity, real auth, tenant
   scoping, automated tests for the posting paths.
2. **P1 hardening:** rounding policy, numbering, CORS/TLS/secrets, webhook
   authenticity, pagination, void/delete, settings save + user management UI, CI,
   containerized deploy, backups.
3. **P2 depth:** posting-group matrices, bank import, payments/OCR/notification
   integrations, observability.
4. **P3 polish.**
