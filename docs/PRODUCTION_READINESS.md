# Production Readiness — Status Snapshot

Status of Zavora ERP toward full production use, reconciled against the codebase
on **2026-06-27**.

> **This is a snapshot, not the live backlog.** Outstanding work is tracked in
> [`../REMAINING.md`](../REMAINING.md); completed work is in
> [`../CHANGELOG.md`](../CHANGELOG.md). Update those two files as work lands;
> refresh the summary here only at milestones.

Legend: ✅ done · 🟡 partial · ⬜ not started

---

## Done since the last snapshot

The earlier version of this doc predated the end-to-end audit. These P0/P1 items
it listed as outstanding are now **done** (see CHANGELOG for detail):

- ✅ **P0 — Real authentication.** JWT + Argon2id; global middleware gates every
  protected route; refresh token in an httpOnly SameSite=Strict cookie.
- ✅ **P0 — Per-request tenant scoping.** Services scope by `ctx.entity_id`;
  manual journal posting, login lookup and the schedulers are all multi-tenant.
- ✅ **P0 — Transaction atomicity.** `create_and_post_in_tx` threads one
  transaction through payment / invoice / credit-note / year-end-close flows.
- ✅ **P0 — Automated test foundation.** 49 tests (proptest + integration); see
  `REMAINING.md` for coverage gaps.
- ✅ **P1 — Rounding policy.** Sub-cent tolerance shared between validator and
  poster; rounding line to the configured account.
- ✅ **P1 — Unapplied-payments account.** Defaults to a seeded account (`9100`).
- ✅ **P1 — Document numbering.** Gapless allocation (`FOR UPDATE`) with year
  reset; Document Numbers settings persist.
- ✅ **P1 — Settings save.** Company / Tax / Payments / Document Numbers tabs all
  persist with live reload.
- ✅ **P1 — Void / delete flows** and **pagination** on list endpoints.
- ✅ **P1 — User management UI** (list + invite with role + optional password).
- ✅ **P2 — Bank statement import** (CSV/MT940/OFX), idempotent.
- ✅ **P2 — M-Pesa STK Push** initiation + callback (idempotent, orphan recovery).
- ✅ **Reporting** Phases 1–3 & 6: per-report pages, branded document
  preview/print, statement→GL→source drill-down, budgets / custom builder /
  scheduled / consolidation.
- ✅ Secret startup validation; `/health` checks Postgres + Redis.

## Still outstanding (summary)

The full, prioritised list lives in [`../REMAINING.md`](../REMAINING.md). Headlines:

- 🟡 **Operations:** CI pipeline, production containerization + TLS, backup
  runbook, graceful-shutdown drain, wider test coverage, observability.
- ⬜ **Security:** CORS lockdown, rate limiting, M-Pesa callback authenticity.
- ⬜ **Features:** procurement / P2P; posting-group matrices; supplier-CN & bill
  line items + AP dimensions; notification real send-out; OCR capture.
- 🟡 **Tax/payroll:** statutory PAYE relief (SHA/insurance) + rounding —
  needs tax-professional validation.
