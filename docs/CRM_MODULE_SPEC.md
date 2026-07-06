# Zavora ERA — CRM Module (optional add-in)

> **Status:** In progress · **Owner:** Eng · **Started:** 2026-07-05
>
> An **optional, non-blocking** CRM for tenants with sales teams: leads →
> pipeline/opportunities → win/lose, activities, and a **customer portal**
> (`/customerportal`) for customer **onboarding** (self-serve or sales-assisted)
> and **self-service** (profile, invoices/statement, support tickets), plus
> essential CRM capabilities and analytics. Additive and gated — it never affects
> the core ERP flows and is off until a tenant enables it.
>
> This doc is the source of truth + progress tracker.

## 1. Principles
- **Opt-in, per-tenant.** A `crm_settings.enabled` flag (default **false**) gates
  all CRM UI and API. Existing accounting/HR/payroll flows are untouched.
- **Additive only.** New tables + routes; no changes to core schemas. Safe to
  ship dark.
- **Builds on what exists.** Reuses `customers` (accounts), auth/roles, audit,
  notifications, and the **portal principal pattern** (`vendor_users` /
  `employee_users` → new `customer_users`, JWT role `Customer`).
- **A lead becomes a customer.** Leads are pre-sales; on conversion they create
  (or link) a `customers` record and (optionally) a portal login.

## 2. Personas
| Persona | Who | Needs |
|---|---|---|
| Sales rep | Owner/Admin/**SalesRep** | Work leads, move deals through the pipeline, log activities |
| Sales manager | Owner/Admin | Pipeline analytics, forecast, team activity |
| Customer | External `customer_users` (role `Customer`) | Onboard, view invoices/statement, raise/track tickets, update profile |

New roles: **`SalesRep`** (CRM without finance) and the external **`Customer`**
principal (portal-scoped, row-level to their own account).

## 3. Data model (migration `046_crm.sql`, additive)
```
crm_settings(entity_id PK, enabled bool default false, default_pipeline_id, updated_at)

crm_leads(id, entity_id, name, company, email, phone, source, status
  [New|Working|Qualified|Unqualified|Converted], owner_user_id, rating, notes,
  converted_customer_id NULL, converted_opportunity_id NULL, created_at, updated_at)

crm_pipelines(id, entity_id, name, is_default, created_at)
crm_stages(id, entity_id, pipeline_id, name, sort_order, probability NUMERIC,
  is_won bool, is_lost bool)

crm_opportunities(id, entity_id, name, pipeline_id, stage_id,
  customer_id NULL, lead_id NULL, amount NUMERIC, currency, expected_close_date,
  probability NUMERIC, status [Open|Won|Lost], owner_user_id, lost_reason,
  created_at, closed_at)
crm_opportunity_events(id, entity_id, opportunity_id, from_stage, to_stage,
  note, actor_id, at)   -- pipeline movement audit

crm_activities(id, entity_id, kind [Task|Call|Meeting|Email|Note], subject, notes,
  due_date, done bool, done_at, related_type [Lead|Opportunity|Customer],
  related_id, owner_user_id, created_at)

-- Customer portal principal (mirrors vendor_users/employee_users)
customer_users(id, entity_id, email, display_name, password_hash NULL,
  status [invited|active|suspended], customer_id NULL, set_token, set_token_expires,
  last_login, created_at, UNIQUE(entity_id, email))

crm_tickets(id, entity_id, customer_id, subject, description, status
  [Open|Pending|Resolved|Closed], priority [Low|Normal|High|Urgent],
  assigned_to_user_id NULL, created_by_customer_user_id NULL, created_at, updated_at)
crm_ticket_messages(id, ticket_id, entity_id, author_kind [staff|customer],
  author_id, body, created_at)
```
Seed a default pipeline (Lead In → Qualified → Proposal → Negotiation → Won/Lost)
with stage probabilities on first enable.

## 4. Customer portal (`/customerportal`)
Separate surface, same shape as the vendor portal:
- **Public auth:** `/customerportal/login`, `/customerportal/register` (self-onboarding),
  `/customerportal/set-password` (accept an assisted invite).
- **Onboarding:**
  - *Self:* prospect registers → creates a **lead** (+ pending `customer_users`);
    a rep qualifies/converts. Optionally auto-create a `customers` record.
  - *Assisted:* a rep converts a lead / picks a customer and **invites** them
    (email + set-password token) — mirrors `invite-ess`.
- **Self-service (row-scoped to their `customer_id`):** profile, **invoices &
  statement** (reuse AR statement), **support tickets** (create/view/reply).
- **Isolation:** `customer_users` JWT role `Customer`; back-office `parse_role`
  rejects it and `CustomerContext` rejects non-`Customer` tokens (exactly like
  vendor/staff). Every `/customerportal/*` query filtered to `ctx.customer_id`.

## 5. API surface (all gated by `crm_settings.enabled`)
**CRM (back-office)** under `/api/v1/crm`:
- `GET|POST /crm/leads`, `GET|PUT /crm/leads/{id}`, `POST /crm/leads/{id}/convert`
- `GET|POST /crm/pipelines`, `GET|POST /crm/stages`
- `GET|POST /crm/opportunities`, `PUT /crm/opportunities/{id}`,
  `POST /crm/opportunities/{id}/move` (stage), `/win`, `/lose`
- `GET|POST /crm/activities`, `POST /crm/activities/{id}/done`
- `GET /crm/tickets`, `POST /crm/tickets/{id}/reply`, status changes
- `GET /crm/analytics` (pipeline by stage, win rate, forecast, activity counts)
- `GET|PUT /crm/settings` (enable/disable, default pipeline)

**Customer portal** under `/api/v1/customerportal`:
- `POST /customerportal/login|register|set-password|refresh|logout`, `GET /me`
- `GET /me/invoices`, `GET /me/statement`
- `GET|POST /me/tickets`, `POST /me/tickets/{id}/reply`
- `GET|PUT /me/profile`

## 6. Analytics
Pipeline value by stage, weighted **forecast** (Σ amount×probability of open
deals), **win rate** (won ÷ closed), average deal size & sales-cycle days,
activities logged, leads-by-source and conversion rate. Computed from
`crm_opportunities`/`crm_activities`/`crm_leads`.

## 7. Non-blocking guarantees
- Migration additive; no core tables altered.
- All CRM routes return 404/forbidden (feature-disabled) unless
  `crm_settings.enabled = true` for the tenant.
- CRM sidebar group and `/customerportal` are hidden/inert until enabled.
- Zero coupling into invoicing/ledger except **reading** AR (statement/invoices)
  for the portal and **linking** a converted lead to a `customers` row.

## 8. Phased delivery
1. **Foundation** — `crm_settings` flag + schema (`046_crm.sql`) + core models +
   feature-flag service (lazy default off, seed default pipeline on enable).
2. **Services + API** — leads (CRUD + convert), pipelines/stages, opportunities
   (move/win/lose + events), activities, tickets, analytics; all flag-gated.
3. **Customer portal** — `customer_users` auth + onboarding (self/assisted) +
   self-service (profile, invoices/statement, tickets).
4. **UI** — CRM shell (pipeline **kanban**, leads, contacts, activities, analytics
   dashboard) gated by the flag; `/customerportal` UI.
5. **Polish** — optional Amos CRM skill; verification (build/test, migration, tsc,
   Playwright).

## 9. Progress
Legend: ⬜ todo · 🟡 in progress · ✅ done
- ✅ Phase 1 — this doc; `046_crm.sql` (validated); core models (`crm/mod.rs`);
  feature-flag + pipeline-seed service (`services/crm.rs`, default off). Core compiles.
- ✅ Phase 2 — services (leads CRUD+convert, pipelines/stages, opportunities
  move/win/lose + events, activities, tickets, weighted analytics) +
  `/api/v1/crm` routes (all flag-gated). **Live-verified**: enable → seed pipeline;
  lead→convert→opportunity; move→Proposal; win; analytics (forecast 40k = 80k×50%,
  win-rate 100%, conversion 100%).
- ✅ Phase 3 — customer portal: `CustomerContext` middleware (role `Customer`);
  `/api/v1/customerportal` login/register(self-onboard → active login + CRM lead)/
  set-password/forgot/refresh/logout/me + self-service (profile, invoices,
  statement, tickets, all row-scoped); staff assisted invite
  (`POST /crm/customers/invite-portal`). **Live-verified**: self-register → lead
  created; ticket create/list; back-office endpoints 401 for a Customer token;
  assisted invite → login OK.
- ✅ Phase 4 — UI. **Back-office CRM shell** (`/crm`, nav group): flag-gated
  (opt-in CTA when disabled) with Overview analytics, Pipeline **kanban**
  (stage-move + win/lose), Leads (create/convert) and Activities tabs, plus a
  Disable control. **Customer portal** (`/customerportal`, separate surface +
  `customerClient.ts`): login/self-register/set-password + shell with
  Invoices & Statement, Support tickets (create/open/reply), and Profile.
  **Playwright-verified**: analytics tie out (open 80k, forecast 40k, win 100%,
  conversion 1/2); kanban shows Beta pilot in Proposal; customer login →
  unlinked notice, ticket thread + reply round-trip.
- ✅ Phase 5 — Amos `crm` skill (`amos/skills/crm/SKILL.md`, UI-first: flag check,
  read analytics off-screen, manage leads/deals/activities/portal invites, confirm
  win/lose/convert/disable) + full verification: `cargo test --workspace` green
  (152 passed / 0 failed), migration 046 applied on startup, `tsc --noEmit` clean,
  Playwright end-to-end on both surfaces. CHANGELOG updated.

**CRM module complete.** Optional, per-tenant, additive; core ERP unaffected.
