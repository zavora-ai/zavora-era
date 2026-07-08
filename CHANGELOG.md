# Changelog

All notable changes to Zavora ERP are recorded here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/). The project is not yet
versioned/tagged; entries are grouped by the date the work landed on `main`.

For what is **not** yet built, see [`REMAINING.md`](REMAINING.md).

## [Unreleased]

### 2026-07-08 — Amos Ambient Ops, full domain coverage, longevity hardening + sample-company onboarding

Amos graduates from a chat-only assistant to a **scheduled + reactive practice**
with a full accountant tool surface, and the platform gets a hardened
multi-month data story. Landed via PRs #44–#48.

#### Added
- **Ambient Ops (phases 1–3)** — Amos's practice calendar
  (`amos/routines/*.toml`): cron-scheduled **routine sub-agents** (one-shot
  Gemini Flash text agents over the same scoped + audited ERP toolset — never
  the browser, only declared scopes). Ships seven routines: morning briefing
  (07:00), eTIMS compliance sweep (18:00), AR chase list (Mon), bank-recon
  check (Fri), PAYE prep (5th), VAT prep (14th) and the month-end close pack
  (3rd). Every run lands in the `amos_runs` ops ledger, the ERP **in-app
  notification inbox**, and a live `routine_done` push. The live Amos knows
  the calendar (`{ops}` prompt block + `ops_status` tool), can fire any
  routine on demand (`run_routine`, a **Routines panel** with Run-now and
  pause/resume ⏸/▶), and the ERP **reacts**: an eTIMS auto-transmit failure
  fires Amos's trigger webhook with the event context
  (`AMOS_WEBHOOK_URL`/`AMOS_WEBHOOK_SECRET`; the secret opens exactly
  `POST /api/ops/run/*` and nothing else). Prep routines end at a report —
  posting/filing/closing stays a human decision in the live session.
- **Full accountant domain coverage** for Amos: AR invoicing (draft →
  VAT-verified gross → confirm → post → **eTIMS check + retry**) with a
  `record-customer-invoice` skill; new `inventory-ops`, `bank-reconciliation`
  and `tax-filing` (KRA calendar) skills; `month-end-review` gains period
  close/reopen. mcp-erp grew matching thin tools: reconciliation
  compute/complete-and-lock, `close_period`/`reopen_period`, tax filings
  list/file/remit. Fixed: the eTIMS tools were referenced by the persona but
  missing from the session allowlist. A regression test pins the skill pack
  (13 skills) and every coverage tool.
- **Session history**: transcripts persist to `amos_sessions` on close (they
  were distilled-then-destroyed); a **Past sessions** panel with a transcript
  viewer. The dialogue in which a posting was approved is now auditable.
- **Sample company at signup**: an opt-in "Explore with sample data" choice
  provisions a realistic Kenyan-SME dataset through the real service layer
  (6 customers, 5 vendors, 8 products, 8 invoices) so the dashboard, ageing,
  P&L/BS and GL are populated to explore; opt-out yields a clean workspace.

#### Changed / hardened
- **Amos endpoint auth**: every `/api/*` and `/showcase/*` route now requires
  the user's ERP JWT (`Authorization: Bearer`, or `?token=` for image loads) —
  the ledger snapshot, distilled memories and evidence screenshots were
  previously readable unauthenticated.
- **Per-session panel state**: tasks/evidence/active-skill moved from
  process-global to per-session — two concurrent sessions no longer overwrite
  each other's workplan or see each other's evidence.
- **Memory hygiene**: near-duplicate memories dedup on store (cosine ≥ 0.9 on
  Postgres; exact-match guard on the in-memory fallback), a `forget` tool +
  per-row delete in the Memory panel, and the panel lists by true recency
  (new adk-memory `list_recent`).
- **Longevity details**: showcase retention sweep (14-day default) + a
  per-session evidence cap; the session transcript cap is a rolling tail (long
  sessions keep their conclusions, not their greeting); Amos replies render as
  sanitized markdown (headings/bold/code/lists/tables), streamed per chunk.

#### Verified
- Live endpoint probes (dev-unauth off): 401 without/with wrong-tenant token,
  200 with a valid one; webhook secret opens only the trigger route.
- Routine runs against the real ledger: manual morning briefing (cash split,
  pending payables, overdrawn USD flagged) and a webhook-fired eTIMS sweep
  that honoured its event context; reports delivered to the in-app inbox.
- `amos` tests 10 passed (memory hygiene, skill pack, routine registry).

### 2026-07-07 — KRA eTIMS (OSCU/VSCU), Amos sub-agents & plans, RBAC v2 enforcement

#### Added
- **KRA eTIMS real-time invoice transmission** (`services/etims.rs`, migration
  `049`): device configuration + initialisation (`selectInitOsdcInfo`), sales
  transmission (`saveTrnsSalesOsdc`) with tax-code mapping
  (A/B/C/D/E ↔ VAT treatments), automatic item registration (`saveItem`),
  **credit notes** as credit/refund receipts referencing the original invoice,
  SCU receipt fields (rcptSign, intrlData, SCU id) + verification QR on
  invoices and POS receipts, auto-transmit after posting (best-effort), an
  eTIMS Settings page, invoice/credit-note transmit actions in the UI, and
  `etims_status`/`etims_transmit_invoice` Amos tools.
- **Amos specialist sub-agents** (Gemini 3 Flash): `analyze_attachment` reads
  PDFs/images the user attaches (paperclip; context-cached for repeat
  questions) and `web_search` answers external questions with Google-grounded,
  cited results. Plus a **session clock** (user timezone + work-as-of posting
  date), **plan tiers** (voice/web-search gated by plan; text-only sessions
  never generate billable audio) and **entity-aware openers + contextual
  follow-up chips** generated after each answer.
- **Honest tenant gate**: a session for an organisation this Amos doesn't
  serve now sees "Amos is being set up for your workspace" instead of a dead
  "Offline" (typed refusal codes on the auth handshake).

#### Changed
- **RBAC v2 is the single gate**: authorization is enforced centrally by the
  declarative route-permission registry (default-deny, granular, custom-role
  aware); the legacy coarse `require_role` shim and its ~200 call sites were
  removed (net −356 lines).

### 2026-07-06 — Optional CRM module + customer portal + Amos CRM skill

Added an **optional, per-tenant CRM** (off by default, additive, never touches the
accounting ledger) — see [`docs/CRM_MODULE_SPEC.md`](docs/CRM_MODULE_SPEC.md).

#### Added
- **Feature flag** (`crm_settings.enabled`, default off): every CRM data route is
  gated by `is_enabled`; enabling seeds a default "Sales Pipeline" (Lead In →
  Qualified → Proposal → Negotiation → Won → Lost). Core ERP is unaffected.
- **Sales pipeline:** leads (CRUD + convert), opportunities (create, stage-move
  with event log + probability, win/lose), activities (task/call/meeting/email/
  note), and support tickets — all under `/api/v1/crm/*` (migration `046_crm.sql`).
- **Analytics:** open pipeline value/count, weighted forecast (Σ amount×probability),
  win rate, average won deal, pipeline-by-stage, and lead conversion.
- **Customer portal** (`/customerportal`): a separate `customer_users` principal
  (JWT role `Customer`, isolated from back-office/staff/vendor sessions).
  Self-onboarding (sign-up creates a portal login + a CRM lead), sales-assisted
  invite (`POST /crm/customers/invite-portal`), and per-customer self-service
  (profile, invoices, statement, support tickets with a message thread).
- **UI:** a flag-gated back-office CRM shell (`/crm`) with Overview analytics,
  a pipeline **kanban**, leads and activities; plus the customer-portal surface
  (login/self-register/set-password + invoices/statement/support/profile).
- **Amos:** a `crm` skill playbook (`amos/skills/crm/SKILL.md`) — checks the flag,
  reads pipeline analytics off-screen, and manages leads/deals/activities/portal
  invites through the UI, confirming win/lose/convert/disable first.

#### Verified
- Workspace tests green (152 passed, 0 failed); migration 046 applies on startup;
  UI `tsc --noEmit` clean; Playwright: CRM overview/kanban and the customer portal
  (login → statement → ticket thread + reply) exercised end to end; a `Customer`
  token is rejected (401) by every back-office endpoint.

### 2026-07-05 — Enterprise HR & Payroll + Amos payroll skill

Rebuilt payroll from a single-shot calculator into an enterprise module (see
[`docs/PAYROLL_HR_ENTERPRISE.md`](docs/PAYROLL_HR_ENTERPRISE.md)) and taught Amos
to run and analyse it.

#### Added
- **Effective-dated, editable statutory config** (`payroll_statutory_config`):
  PAYE bands, NSSF, SHA, Housing Levy, reliefs, NITA — versioned by effective
  date so historical runs stay reproducible; edited from a Payroll Settings UI.
- **Masters:** earning types, deduction types, departments (with a reusable
  `DepartmentSelect` lookup + inline "add new" used by the employee card and
  procurement requisitions). Employee card now uses dynamic allowances (honouring
  the `taxable` flag) and a department dropdown (`department_id`).
- **Config-driven batch engine:** variable per-run inputs (bonuses/overtime/
  voluntary & loan deductions), recurring items, loan amortization, joiner/leaver
  proration, and YTD accumulators; set-based loads for scale. Balanced GL posting
  with the salary expense **split by department (dimension-tagged)**, deductions
  credited to liability accounts, and loan repayments recorded.
- **Pay-run lifecycle:** draft → approve → post → paid, with list/detail/recompute/
  delete endpoints and a **prepare→review→commit UI** (history, per-run adjustments
  that auto-recompute, pre-run validation, richer payslip PDF).
- **Reports:** payroll register, statutory remittance schedule, P9, and net-pay
  bank/EFT file — tie-out verified against a balanced posted journal.
- **Amos `hr-payroll` skill** + prompt update, and **mcp-erp** HR/payroll tools
  (`list_employees`, `list_fiscal_periods`, `list_departments`, `list_pay_runs`,
  `get_pay_run`, `run_payroll`, `add_pay_run_input`, `recompute_pay_run`,
  `approve_pay_run`, `post_pay_run`, `mark_pay_run_paid`; `run_report` extended
  with the payroll report types) so Amos can run, adjust, commit and analyse
  payroll and explain PAYE/NSSF/SHA/Housing/HELB in plain language.
- Migrations `041` (payroll schema) and `044` (unquote `employment_type`).


### 2026-07-05 — Amos: complete tenant isolation & agent guardrails

Amos is now a scoped, auditable agent. Each deployment serves exactly one
entity and refuses everything else. See [`docs/AMOS.md`](docs/AMOS.md) §5b.

#### Added
- **Session identity gate.** The embedded ERP page hands the signed-in user's
  access token to the Amos iframe (`postMessage`); Amos verifies it with the
  shared `JWT_ACCESS_SECRET` (signature/expiry/type/issuer) and requires the
  token's `entity_id` to equal the served entity. Wrong-entity, forged, expired,
  or missing token ⇒ the WebSocket session is refused before the agent runner is
  built — no tools, data, memory, or showcase. (`amos/src/auth.rs`, `routes.rs`;
  `AmosPage.tsx` token handoff.)
- **Role-based tool scoping** via `adk-auth` (`check_scopes`): the user's ERP
  role grants `erp:read`/`erp:write`/`ledger:post`; each ERP/browser tool is
  wrapped to check its required scope before running, so a read-only user's
  session cannot post to the ledger regardless of what the model attempts.
  (`amos/src/scope.rs`.)
- **Prompt-injection & exfiltration guardrails** (`amos/src/guard.rs`): inbound
  user turns are screened for instruction-override and secret/cross-tenant
  probes before reaching the model; the `remember` tool rejects secret-shaped
  content.
- **Audit trail** (`amos/src/audit.rs`): session authentication and every tool
  access (allowed/denied) are written to a dedicated `amos_audit_events` table,
  keyed by entity + user + session.
- **Entity-scoped memory**: memory is keyed by the served entity, isolating it
  per tenant.

#### Config
- `amos` env: `JWT_ACCESS_SECRET` + `JWT_ISSUER` (prod, from the API's secret),
  optional `AMOS_SERVED_ENTITY_ID`, and `AMOS_DEV_ALLOW_UNAUTH` (dev only).

### 2026-07-04 — Amos: your personal AI accountant

Zavora ERA gains an agentic layer: **Amos**, a realtime voice + chat AI
accountant (Gemini Live via adk-realtime) built for non-accountant business
owners. Lives in the new standalone `amos/` crate (`:8090`) and is embedded in
the web UI at `/amos` behind the "Amos — AI Accountant" sidebar button, so the
ERP shell (navigation + branding) stays consistent. Documentation:
[`amos/README.md`](amos/README.md); screenshot in the main README.

#### Added
- **Realtime voice + chat agent.** Mic audio streams to Gemini Live (16 kHz up
  / 24 kHz down) with live transcripts both ways; typing works mid-voice
  session. Session UI shows a live business snapshot (cash, AR/AP, overdue,
  bank balances straight from the ledger), a **workplan** panel, screenshot
  **evidence cards**, and a timestamped **activity trail** with "Posting"
  badges on ledger writes.
- **MCP toolset.** Tools bridge into the realtime session from two MCP servers
  (configured in `amos/mcp.json`, Kiro format with `${VAR}` env expansion):
  `mcp-erp` with a new **zavora backend** (JWT auth with auto re-login; bills,
  payments incl. KES WHT and non-cash funding, reports, dashboard, journal
  posting — see the mcp-erp repo), and `@playwright/mcp` driving a headed
  Chrome through the ERP for showcasing, with **deterministic auto-login**
  wrapped around `browser_navigate`.
- **Skills (agentskills.io standard).** Drop-in `SKILL.md` playbooks under
  `amos/skills/` teach Amos consistent multi-step procedures via progressive
  disclosure (catalog line in the prompt, full body on demand through a
  `use_skill` tool). Ships six: record-vendor-bill, record-payment,
  financial-reporting, manual-journal, erp-showcase, month-end-review. A
  skill's `allowed-tools` extends the MCP tool allowlist.
- **File-based agent configuration.** System prompt (`amos/system.md`),
  operating rules (`amos/AGENTS.md`, incl. confirm-before-post and
  never-invent-figures guardrails), MCP servers (`amos/mcp.json`) — all
  editable without recompiling.
- **Service user.** Amos calls the API as `amos@zavora.ai` (Accountant role);
  the visible browser signs in as a configurable account (`ERP_LOGIN_*`).
- **Semantic long-term memory (2026-07-05).** Amos now learns as he works:
  pgvector-backed memory (adk-memory `PostgresMemoryService` + Gemini
  embeddings, sharing the ERP database — postgres image switched to
  `pgvector/pgvector:pg17`). Profile facts and the latest session summary are
  injected into every session's prompt; per-skill *lessons* ride along with
  playbooks when `use_skill` loads them; failed workplan tasks auto-file
  lessons; a session-close distiller extracts durable knowledge from each
  transcript. New `remember`/`recall` tools, a Memory panel in the UI, and
  `GET /api/memories`. Upstream: `PostgresMemoryService::add_entry`
  implemented in adk-memory (was "not implemented").

#### Fixed
- **Production deploy broke after the initial Amos merge** (PR #30 → #31): the
  API Docker image only copies `zavora-erp-core`/`zavora-erp-api`, so listing
  `amos` as a workspace member failed the image build — and `amos`
  path-depends on `../../adk-rust`, which can never exist in that context.
  `amos` is now its own cargo workspace with a committed lockfile; the root
  workspace and Docker build are back to their previous shape.
- **Upstream (adk-rust): batched Gemini tool calls dropped.** The Gemini Live
  translator emitted only the first function call of a parallel batch, leaving
  the model waiting forever for the missing responses (sessions stalled, then
  aborted server-side). Fixed in `adk-realtime` to emit every call.

### 2026-06-28 — Multi-currency onboarding hardening (real-company validation)

Surfaced while setting up a real Kenyan services company end-to-end through the
UI (not VAT-registered; trades heavily in USD/EUR). See `docs/UI_GAPS.md`.

#### Added
- **Bank account → GL account selector.** The Add Bank Account form now lets you
  choose which ledger account a bank account posts to (asset, non-control;
  defaults to the KES bank GL). Previously every account silently defaulted to
  one GL code, so a USD account and an M-Pesa till co-mingled with the KES bank
  in a single ledger account — breaking per-account balances and FX revaluation
  of the foreign account. The backend already accepted `gl_account`; only the UI
  was missing it. (`BankingPage`.)
- **Future fiscal periods can be locked.** `close_period` now permits a **soft
  close from `Future`** (previously rejected outright), and the Periods page
  shows the Soft Close action on future periods. Future periods are postable, so
  they must be lockable — e.g. to stop stray postings into the auto-seeded next
  year while back-booking a prior one. Hard close still requires a prior soft
  close. (`services::periods`, `PeriodsPage`.)
- **WHT rates self-heal at startup.** Migration 021 seeds the statutory KRA WHT
  rates once with `ON CONFLICT DO NOTHING`, so a wiped/restored volume could
  leave the table empty while the migration ledger still showed 021 applied —
  making every WHT lookup silently resolve to 0 (tax not withheld). Startup now
  backfills any missing statutory category via `wht::ensure_seeded`, without
  overwriting an admin's customised rate.
- **Services-first chart of accounts & posting defaults.** The Kenya Standard
  seed gained `1310 WHT Receivable`, `1610 Unpaid Share Capital`, `5250 Royalty
  Income`, and `7350 Software, Cloud & Subscriptions`; default sales → `5100`
  Service Revenue and default purchase → `7350` (was goods-centric). New
  non-registered tenants default to **VAT Exempt** so they never accidentally
  charge output VAT, and the Products form derives its tax-treatment default from
  the company's VAT registration.
- **Company statutory fields + editable currency/year-end.** Settings → Company
  now captures **Registration Number**, **Registered Address**, and **Phone**
  (`BrandingConfig.registration_number`), and makes **Base Currency** and
  **Fiscal Year-End** editable (previously read-only). Non-resident vendors are
  prompted that WHT may apply on services/royalties.

#### Fixed
- **FX Rates page crash.** The exchange-rate `rate` arrives from the API as a
  JSON string (Rust `Decimal` serialises to a string); the FX Rates table called
  `rate.toFixed(4)`, throwing and blanking the page after a rate was saved. Now
  coerces with `Number()` before formatting.
- **Posting Accounts control-account picker.** A/R and A/P roles now correctly
  include control accounts in their picker (they *are* the GL control accounts);
  other roles still exclude them.

### 2026-06-28 — User-driven tenant management & OCR receipt capture

#### Added
- **PDF / spreadsheet bank-statement import (review-before-commit).** Most Kenyan
  banks issue PDF statements and M-Pesa issues XLSX; `POST /bank/import/extract`
  sends the file to the xberg extraction sidecar (PDF text layer, OCR for scans,
  or spreadsheet cells), parses the text into candidate transaction rows with
  per-row confidence, and returns them **without writing anything**. The Banking
  import dialog gained a **PDF tab** with an editable review table
  (date/description/debit/credit/balance; low-confidence rows flagged; drop bad
  rows); **Confirm** serialises the reviewed rows to CSV and commits through the
  existing deterministic CSV importer, so idempotency, dedup and the
  categorisation queue stay the single source of truth. OCR'd financial rows are
  never auto-committed. (`services::statement_pdf`.)
  - **M-Pesa statements parse cleanly** (dedicated parser; validated against a
    real merchant XLSX — 28/28 transactions, totals reconcile to the statement
    summary to the cent).
  - **Generic bank PDFs** are best-effort: PDF text extraction often scrambles
    multi-column tables, so the generic parser is a starting point that the user
    must review/correct. **Per-bank templates** for the common banks are the
    planned next step.
- **Inventory now posts to the general ledger.** Standalone **Receive Stock** and
  **Issue Stock** previously updated quantities/value but never touched the GL,
  leaving inventory off-ledger (trial balance didn't reflect stock). They now
  post journals within the same transaction — receipt: DR Inventory / CR
  Goods-Received-Not-Invoiced clearing; issue: DR COGS / CR Inventory — and each
  stock movement is linked to its journal entry (`stock_movements.journal_entry_id`,
  migration 034). `PostingSetup` gained `inventory_asset`, `cost_of_goods_sold`,
  and `inventory_clearing` accounts. The transaction-scoped `_in_tx` variants
  used during invoice posting stay GL-free, so invoices still single-book COGS
  (no double-posting). Verified: Inventory/GRNI/COGS balances tie to the
  subledger.
- **Posting-group assignment on master records.** Customers, vendors and products
  now have **Posting Groups** selectors on their create forms (business group +
  VAT group / product group + VAT product group), so the BC-style matrices are
  actually usable — a customer assigned to e.g. an "Export" business group routes
  its A/R to that group's control account. Verified end-to-end (export-group
  invoice posted A/R to the group account, not the default). Group ids are
  surfaced on the list/detail responses.
- **Customer Payment History report.** New report listing every receipt from a
  customer over a period — date, payment number, method, reference, amount, and
  the portion still unapplied (on-account) — with totals. Available in the
  report catalogue (party + period controls), rendered in the UI, and CSV-
  exportable. Previously this report type silently returned an empty body.
- **Consolidation: FX translation + intercompany elimination.** The consolidated
  trial balance now **translates** each entity's functional balances into a
  chosen `presentation_currency` via `exchange_rates` (latest rate on/before the
  date; 1:1 when already in that currency, flagged in `untranslated` when no rate
  is on file), and **eliminates intercompany AR/AP** between the consolidated
  entities — receivables/payables to a party whose KRA PIN matches a sister
  entity — surfaced in an `eliminations` section. Previously it only summed
  functional amounts and flagged mixed currencies.
- **M-Pesa STK Push (Daraja) client.** Real Lipa-na-M-Pesa STK Push: OAuth token
  + `processrequest` via a new `payments::daraja` client, wired to
  `POST /payments/mpesa-stk-push` (resolves the invoice balance + customer phone,
  triggers the prompt, returns the checkout request id). Deployment-gated by
  `MPESA_CONSUMER_KEY`/`SECRET`/`SHORTCODE`/`PASSKEY`/`CALLBACK_URL` (sandbox vs
  production via `MPESA_ENV`); returns a clear "not configured" error when the
  credentials are absent. _Note: the outbound Daraja calls require live Safaricom
  credentials and have not been exercised end-to-end; the password/timestamp/
  MSISDN helpers are unit-tested._
- **Per-tenant notification providers (self-service).** Each tenant can now
  configure its **own** delivery credentials in **Settings → Providers** — SMTP
  (email), Africa's Talking (SMS), Twilio (WhatsApp) — instead of relying only
  on the deployment-wide env vars. The worker resolves a tenant's provider per
  message and falls back to the deployment provider when a tenant hasn't set its
  own. Secrets (SMTP password, API key, auth token) are **encrypted at rest**
  (AES-256-GCM via `NOTIF_ENC_KEY`, `crate::crypto`) in a new
  `notification_providers` table (migration 033), are **write-only** in the UI
  (the API returns only `has_secret`, never plaintext; leave blank to keep), and
  Owner/Admin-gated. A **"Send test"** button delivers a one-off message to
  verify credentials. API: `GET|PUT /notification-providers`,
  `POST /notification-providers/{channel}/test`.
- **Notification event preferences (admin).** A new **Settings → Notifications**
  tab lets Owners/Admins choose, per event type, whether the event fires and on
  which channels (Email/SMS/WhatsApp/In-App) — overriding the previously
  hardcoded routing at the notification call sites (`InvoiceSent`,
  `CreditLimitExceeded`, `PeriodCloseWarning`, `ScheduledReport`, and the other
  transactional events). Stored as per-tenant overrides
  (`notification_settings`, migration 032); a missing row uses the built-in
  default. Automatic events are fully skipped when disabled; an explicit invoice
  send still delivers by email regardless. Invoice payment **reminders** remain
  per-customer (`ReminderPolicy`). API: `GET|PUT /notification-settings`;
  resolved at call sites via `notification_prefs::effective_channels`.
- **Notification delivery history (admin).** A read-only, Owner/Admin-gated
  delivery-history view surfaces the full notification record across **all**
  channels (email, SMS, WhatsApp, in-app) — status, recipient, error, and
  timestamps — not just the in-app inbox. `GET /notifications/delivery`
  (paginated; filter by channel/status/event/recipient/date) and
  `GET /notifications/delivery/stats` (counts by status and channel) back a new
  **Notifications** admin page (stats cards, filters, status badges with inline
  failure reasons). Makes the email/SMS/WhatsApp send-out observable and
  debuggable. Indexes added in migration 031.
- **Notification delivery — all channels wired (P2).** The notification worker
  now performs real send-out on every channel, not just Email. SMS via
  **Africa's Talking** (Kenyan gateway; `AT_USERNAME`/`AT_API_KEY`/optional
  `AT_SENDER_ID`) and WhatsApp via **Twilio** (`TWILIO_ACCOUNT_SID`/
  `TWILIO_AUTH_TOKEN`/`TWILIO_WHATSAPP_FROM`) join the existing SMTP email and
  InApp channels. Providers are built once at startup from the environment and
  are **env-gated with graceful degradation** — an unconfigured channel logs a
  clear "not configured" error and never blocks the others; the worker logs
  which channels are live. Phone numbers are normalised to Kenyan E.164
  (`07.. / 7.. / 254.. / +..` → `+254..`), and HTML notification bodies are
  reduced to plain text for SMS/WhatsApp. This unblocks scheduled/emailed
  reports and all reminder flows. (`services::messaging`.)
- **User-driven tenant lifecycle.** The in-app tenant switcher now supports the
  full lifecycle on top of signup: **create**, **switch**, **archive (close)**,
  **restore**, and **leave**. Archiving is a reversible soft-close
  (`entity_settings.archived_at`, migration 029) — a hard delete is deliberately
  not offered because the immutability triggers block deleting posted journal
  lines and the ledger/audit trail must be retained for compliance. Owner-only
  archive/restore; archiving the caller's last active tenant and a sole Owner
  leaving are both refused; switching into an archived tenant is blocked; every
  action writes an audit event. API: `POST /auth/tenants/{id}/archive` ·
  `/unarchive` · `/leave`; `GET /auth/tenants?include_archived=true`.
- **OCR receipt capture (P2)** completed end-to-end. `POST /receipts/capture`
  now accepts a `multipart/form-data` image/PDF upload (8 MiB cap), stores the
  image, runs OCR **synchronously**, and returns extracted fields with per-field
  confidence; the review UI lets a user correct fields and `POST
  /receipts/confirm` posts a **VAT-inclusive** bill (net line + recomputed VAT,
  no double-counting). OCR is a **pluggable provider**: the default
  `ManualReviewProvider` needs no external service (the reviewer types the
  fields), and an optional **xberg** sidecar (`OCR_PROVIDER=xberg`,
  `XBERG_URL`, `XBERG_OCR_TIMEOUT_SECS`) performs real extraction over HTTP,
  degrading gracefully to manual review when unconfigured or unreachable — the
  same convention as the M-Pesa gateway. Receipt images stay on-prem (a local
  OCR backend); no data is sent to third parties by default.


### 2026-06-27 — End-to-end audit: idempotency, atomicity & multi-tenancy

Backend and UI fixes from a full end-to-end accounting audit, plus a complete
external-ledger → Zavora rebuild used as the correctness oracle.

#### Added
- **External-ledger rebuild harness** (`scripts/migrate/`): extract a sample
  company from another accounting system, set up a matching Zavora tenant +
  chart + masters, replay transactions through the real AR/AP/banking flows,
  and compare Zavora reports against the source reports. Result: **P&L matches
  the source to the cent**; Balance Sheet reconciled (tax neutralised). See
  `sample_data/migration/comparison_report.md`.
- **Bank statement import** is now wired end-to-end (CSV / MT940 / OFX) and
  **idempotent** — re-importing the same file is rejected (file `content_hash`),
  and duplicate lines are skipped (`dedup_key`). Migration 027.
- **Year-end close** is now functional, atomic and idempotent: closing + opening
  entries post in one transaction; `YearEndClose`/`OpeningBalance` sources are
  allowed into hard-closed periods (migration 026); `POST /periods/year-end-close`
  wired. UI gains a "Close Year" action (shown once all of a year's periods are
  hard-closed).
- **Asset depreciation** rewritten as an idempotent **catch-up** run
  (`depreciated_through`, migration 025): a run books every missed month up to the
  target and cannot double-post. The scheduler now books prior months **for all
  tenants** automatically.
- **AR/AP Ageing** reports render as proper tables (were raw JSON).
- **Audit Trail** is now audit-grade: who / what / when / before→after, with
  actor names and emails resolved server-side.

#### Fixed
- **Multi-tenancy**: report schedules, recurring invoices/journals and reminders
  now iterate **all tenants** (were bound to the startup entity). Recurring
  journals advance `next_run` in the same transaction as the post; recurring
  invoices use the scheduled date, not "today".
- **Posting**: `unapplied_payments` now defaults to a seeded account (`9100`)
  instead of a non-existent `3050` that broke the trial balance.
- **Auth**: login resolves the user globally instead of scoping to the served
  entity (new tenants could not sign in).
- **Journal validation** uses the same sub-cent rounding tolerance as the poster.
- **Invoicing**: discount lines post as a positive debit to revenue (no negative
  journal lines).
- **Payments**: the M-Pesa callback recovers orphaned receipt claims instead of
  rejecting them forever.
- **Reports UI**: Balance Sheet "Total Liabilities + Equity" no longer shows
  `NaN`; report drill-down no longer hangs on "Generating report…".
- **Assets UI**: register form sent a display label instead of the category enum
  (422), and depreciation posted to phantom GL codes — now uses the seeded chart.
- **UI contracts**: Record Payment, New Journal Entry, Import Statement,
  Auto-Categorise and Close Year actions were broken or dead — payloads fixed and
  actions wired.

### 2026-06-22 — Accounting feature completion (onboarding → period-end → tax)

#### Added
- **Onboarding**: opening-balances entry; bulk CSV import for
  customers/vendors/products.
- **Period-end**: recurring / accrual / prepayment journals.
- **AR**: bad-debt write-off.
- **Tax**: VAT / PAYE / WHT filing + remittance workflow.
- **Inventory**: stock-take adjustment.
- **Banking**: formal bank reconciliation (complete & lock).
- **Reporting**: Statement of Changes in Equity + direct-method cash flow.
- **Release hardening (R3–R10)**: list-endpoint pagination; loading/error states
  + shared components; persisted recurring invoices; estimate draft edit/delete;
  vendor detail page; in-app notification inbox; new-tenant dashboard empty
  state; customer-statement send action.
- Inventory **Add Item**, `GET /payments/{id}` receipt preview, **Document
  Numbers** settings persistence, **asset depreciation** run, **FX revaluation**
  run (engines existed; routes were stubs — now wired).
- Supplier credit notes from posted bills (mirrors invoice credit notes).

### 2026-06-17 → 06-19 — Reporting & document output

#### Added
- **Reports**: typed Reports UI with balancing badges, CSV/Print-PDF/Excel
  export and comparative periods; full-page branded statement layout.
- Full statement set: Trial Balance, Balance Sheet, P&L, Cash Flow, General
  Ledger, AR/AP Ageing, VAT Return; customer & vendor statements; KRA statutory
  (PAYE P10, WHT schedule, VAT by rate); payroll summary; bank-reconciliation
  summary; income-by-customer / expense-by-vendor; inventory valuation &
  fixed-asset register.
- **Componentized** the reports monolith into per-report pages (Phase 1) and
  added **branded preview & print** for invoices, estimates, credit notes, bills
  and receipts (Phase 2).
- **Drill-down**: statement → GL → source document (migration 015).
- **Dimensions** (analytical accounting): masters, capture on journal & invoice
  lines, dimensional-analysis report (migrations 017/020).
- **Customisation**: budgets + Budget-vs-Actual (016); custom report builder
  (018); scheduled + emailed reports (019); multi-entity consolidated trial
  balance.
- **Performance**: per-account period-balance snapshots for O(periods) as-at;
  denormalised `entity_id`/`date` onto journal lines; composite indexes.

### Earlier — Foundation & auth hardening

#### Added
- Estimates, supplier credit notes, eTIMS status, journal-entry reversal,
  period-management UI; role-aware action gating.
- **Auth hardening**: JWT + Argon2id password hashing; global middleware gates
  every protected route; role checks on all master-data writes; access token in
  memory only, refresh token in an httpOnly SameSite=Strict cookie;
  `/auth/logout` revokes + clears.
- **Posting setup**: per-tenant GL resolver + editable Settings → Posting
  Accounts UI with live reload (see `docs/POSTING_SETUP.md`).
- M-Pesa webhook idempotency; atomic draft creation for invoices/estimates.
