# Remaining Work — Zavora ERP

**Single source of truth for what is *not* yet built.** For what *is* done, see
[`CHANGELOG.md`](CHANGELOG.md). Keep this file honest — verify against the code
before marking anything done, and move completed items into the changelog rather
than leaving stale ticks behind.

Legend: ⬜ not started · 🟡 partial · ✅ done (kept briefly for context, then moved to CHANGELOG)
Priority: **P0** blocker · **P1** before go-live · **P2** fast-follow · **P3** polish

_Last reconciled against the codebase: 2026-07-05._

> **Amos** (the AI accountant) has its own program reference —
> [`docs/AMOS.md`](docs/AMOS.md) — covering architecture, what's built, and its
> roadmap. Its pending items (tenant isolation, guardrails) are folded into the
> security section below.

---

## 1. Functional features

- ⬜ **P1 — Procurement / P2P.** Purchase order → RFQ/tender → goods receipt →
  3-way match → debit notes → expense claims. Entirely absent. Largest functional
  gap. (Tracked as task #42.)
- ✅ **P2 — Posting-group matrices.** _(done — moved to CHANGELOG 2026-06-28.)_
  VAT Business × VAT Product → rate + output/input; General Business × General
  Product → sales/purchase/COGS; per-business-group A/R & A/P control accounts;
  matrix editor UI **and** per-record group assignment on customer/vendor/product
  forms. Resolution is a fallback chain (line override → matrix → flat
  `PostingSetup`), wired into invoicing, payments, supplier-CN and AR/AP control.
- 🟡 **P2 — Supplier credit notes & bill lines.** Supplier credit notes store only
  `gross_total` (no line items); bill/CN posting is header-level (one expense
  line), so per-line GL and per-line **dimension capture on the AP side** are
  missing.
- ✅ **P2 — Notification delivery.** _(done — moved to CHANGELOG 2026-06-28.)_
  All channels wired: Email (SMTP/lettre), SMS (Africa's Talking), WhatsApp
  (Twilio), InApp. Each is env-gated and degrades gracefully when unconfigured;
  the worker logs which channels are live. Scheduled/emailed reports now deliver.
- ✅ **P2 — OCR receipt capture.** _(done — moved to CHANGELOG 2026-06-28.)_
  Multipart upload → image stored → pluggable OCR provider → review (per-field
  confidence) → confirm → VAT-inclusive bill. Default provider is manual review
  (no external dependency); an optional **xberg** sidecar enables real
  extraction via `OCR_PROVIDER=xberg` + `XBERG_URL` (see `.env.example`).
- ✅ **P3 — Tenant management.** _(done — moved to CHANGELOG 2026-06-28.)_
  In-app create / switch / archive (close) / restore / leave, on top of the
  signup flow.

## 2. Tax & payroll accuracy

- 🟡 **P1 — Statutory payroll relief.** PAYE relief omits SHA/insurance relief and
  is not rounded to the nearest shilling. Needs tax-professional validation before
  filing-grade use. (Rates present: PAYE bands, NSSF Tier I/II, SHA 2.75%, Housing
  Levy 1.5%, HELB.)

## 3. Security hardening

- ✅ **P0 — Amos tenant isolation.** _(done 2026-07-05 — moved to CHANGELOG.)_
  Per-session identity gate: each Amos serves one entity and refuses any session
  whose verified JWT is for a different entity; role-scoped tools; entity-keyed
  memory; prompt-injection guardrails; audit trail. See
  [`docs/AMOS.md`](docs/AMOS.md) §5b.
- ⬜ **P1 — CORS lockdown.** Still `CorsLayer::permissive()`; must restrict to
  `CORS_ALLOWED_ORIGINS` in production.
- ⬜ **P1 — TLS termination** + secrets in a managed store (startup secret
  validation already fails fast on missing secrets).
- ⬜ **P2 — Rate limiting & request-size limits** on login, webhooks, uploads
  (`governor` / `DefaultBodyLimit`).
- ⬜ **P2 — M-Pesa callback authenticity.** IP allowlist / signature validation
  (callback idempotency + orphan recovery already done).

## 4. Operations & quality

- ⬜ **P1 — CI pipeline.** No `.github/workflows`. Need: `cargo clippy --workspace
  -D warnings`, build, `sqlx migrate run`, `cargo test`, UI `tsc --noEmit` +
  `eslint` + build, against Postgres/Redis service containers.
- ⬜ **P1 — Containerization & deploy.** Production Dockerfiles (API + UI),
  `docker-compose.prod.yml`, reverse proxy with TLS, readiness probes.
  (`/health` checks Postgres/Redis; **graceful shutdown drain is not implemented**.)
- ⬜ **P1 — Backups & migration safety.** No `docs/BACKUP_RUNBOOK.md`;
  pg_dump/pg_restore procedure + destructive-migration review.
- 🟡 **P1 — Automated tests.** 49 tests exist (`proptest` property tests +
  integration tests for payment flows + harness). Coverage gaps: posting/period
  close/FX/payroll property tests, settings persistence, tenant isolation,
  document numbering. Expand toward the property list in
  `docs/production-readiness/tasks.md`.
- ⬜ **P2 — Observability.** Structured JSON logs with request/user/entity ids;
  Prometheus `/metrics`; OpenTelemetry tracing; durable consumer for the Redis
  audit stream.
- ⬜ **P2 — Performance.** N+1 review on invoice/bill/payment detail endpoints;
  verify list endpoints under load.
- ⬜ **P3 — Build-warning cleanup** (unused vars in `bills.rs`/`invoicing.rs`) and
  enable `-D warnings` in CI.

## 5. Reporting (roadmap tail)

See [`docs/REPORTING_ROADMAP.md`](docs/REPORTING_ROADMAP.md). Phases 1–3 and 6 are
done; remaining:

- 🟡 **Phase 4 — Numeric tie-out tests** for bills/payroll/inventory/bank-rec
  reports (SQL re-validated; integration tie-out tests pending).
- 🟡 **Phase 5 — Dimensions tail.** Per-line bill dimensions (needs the bill-posting
  rework above); per-account capture controls; snapshot key extended to
  account+dimension+period (Option B) only if volume warrants.

---

## Suggested sequence to go-live

1. **P1 hardening:** CORS/TLS/secrets, CI pipeline, containerized deploy, backups,
   expand the test suite over the posting paths.
2. **P1 functional:** procurement (if in scope for v1), statutory payroll relief
   validation, notification real delivery.
3. **P2 depth:** posting-group matrices, supplier-CN/bill line items + AP
   dimensions, rate limiting, M-Pesa callback authenticity, observability,
   performance.
4. **P3 polish.**
