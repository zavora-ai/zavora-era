# Remaining Work — Zavora ERP

**Single source of truth for what is *not* yet done (or is incomplete).** For what
*is* done, see [`CHANGELOG.md`](CHANGELOG.md). Keep this file honest — verify
against the code before marking anything done, and move completed items into the
changelog rather than leaving stale ticks behind.

Legend: ⬜ not started · 🟡 partial · ✅ done (kept briefly for context, then moved to CHANGELOG)  
Priority: **P0** blocker / correctness · **P1** before go-live · **P2** fast-follow · **P3** polish

_Last reconciled against the codebase: **2026-07-09**_  
_Source: four-pass audit (product/functional · accounting · UX E2E · **Amos AI**)
against `feat/portals-page` / main tip ≈ `85ecf01`._

> **Independent validation (2026-07-09, second pass).** Ten of the highest-severity
> claims were re-verified line-by-line against main. **All ten confirmed real** —
> this review is accurate, not stale:
> §1.1 vendor unapplied leg (`payments.rs` VendorPayment branch: CR Bank full +
> CR Unapplied excess — wrong sign; the journal engine's balance rule makes this
> **fail the posting** rather than corrupt the ledger, so vendor
> overpayments/advances currently error out), vendor WHT auto-remit
> (`payments.rs` ~646: DR WHT Payable / CR Bank on every vendor payment), bill
> FCY as base (`routes/bills.rs` ~127: `currency: base_ccy` + doc `fx_rate` on
> every line), inventory/GRNI defaults (`posting/mod.rs`: 1300 = VAT Input,
> 3010 = Trade Creditors), GRNI never cleared (zero `inventory_clearing`
> references in the bill-post path), asset create register-only
> (`services/assets.rs` insert, JE only in depreciation), AR ageing drafts
> (`reporting.rs`: `status NOT IN ('paid','voided')` only, no type filter),
> product track-inventory E2E break (`catalog.rs` binds the flag only), CORS
> permissive (`main.rs:511`), and Amos `required_scopes` (`amos/src/scope.rs`:
> the ~30 tools added since the map was written all fall through to `erp:read`
> — a Viewer session could post pay runs, close periods, or transmit to KRA).
> Fix order recommendation stands as written in §1.1/§7.6.

> **All ten fixed (2026-07-09).** Every defect above is closed on main —
> see the ✅ entries in §1.1, §4.2, §5, §7.1 for the fix shape, verification
> evidence, and the two follow-ups noted (edit-time inventory enable; set
> `CORS_ALLOWED_ORIGINS` in the prod deploy env).

> **Doc hygiene.** Prior `REMAINING.md` (2026-07-05) was stale: procurement/P2P,
> bill lines, posting-group matrices, notifications, OCR, and prod Docker/deploy
> were listed as missing while already shipped. Those are corrected below.
> Cross-check also: [`Specs.md`](Specs.md) (Wave parity claims),
> [`docs/UI_GAPS.md`](docs/UI_GAPS.md), [`docs/REPORTING_ROADMAP.md`](docs/REPORTING_ROADMAP.md),
> [`docs/HR_MODULE_SPEC.md`](docs/HR_MODULE_SPEC.md), [`docs/AMOS.md`](docs/AMOS.md)
> (some tables still stale vs code — see §7),
> [`docs/PRODUCTION_READINESS.md`](docs/PRODUCTION_READINESS.md) (snapshot only).

> **Amos** program reference: [`docs/AMOS.md`](docs/AMOS.md), `amos/README.md`,
> `amos/system.md`, `amos/AGENTS.md`. Full backlog: **§7**.

---

## Already shipped (do not re-open)

These were incorrectly still open in older backlog files; verified present:

| Item | Evidence |
|---|---|
| Procurement / P2P (PR → tender → LPO → GRN → 3-way match → debit notes → expense claims → vendor portal) | `zavora-erp-core/src/services/procurement.rs`, migrations `036`/`042`–`045`, UI `pages/procurement/*`, `pages/portal/*` |
| Posting-group matrices + control accounts per biz group | `zavora-erp-core/src/posting/groups.rs`, Settings UI `PostingGroupsTab.tsx` |
| Bill line items + per-line GL + dimensions on bill post | `bill_lines`, `services/bills.rs`, `routes/bills.rs` post path |
| Supplier CN line items + post | `services/supplier_credit_notes.rs`, `SupplierCreditNotesPage.tsx` |
| Notification delivery (Email/SMS/WhatsApp/InApp) | `services/messaging.rs`, `notification_worker.rs`, Settings providers |
| OCR receipt capture (manual + optional xberg) | `services/ocr.rs`, `ocr_provider.rs` |
| Tenant create/switch/archive/restore/leave | `routes/auth_tenants.rs`, `TenantSwitcher.tsx` |
| RBAC v2 route registry + authz coverage test | `middleware/route_perms.rs`, `tests/authz_coverage.rs` |
| eTIMS OSCU/VSCU | `services/etims.rs`, migration `049`, `EtimsPage.tsx` |
| Enterprise payroll (masters, variable inputs, reports, UI) | `docs/PAYROLL_HR_ENTERPRISE.md`, `services/payroll.rs` |
| Optional CRM + customer portal | `docs/CRM_MODULE_SPEC.md`, migration `046` |
| Leave + staff ESS | `services/leave.rs`, `pages/staff/*` |
| POS sessions / sell / Z-report | `services/pos.rs`, `pages/pos/*` |
| Amos (broad): voice+chat, MCP ERP+browser, 16 skills, 11 ambient routines, memory, session history, REST JWT auth, wrong-tenant honesty, showcase, plan entitlements | `amos/`, `docs/AMOS.md` §2–§5b, CHANGELOG Jul 2025–2026 |
| Prod Docker + compose + deploy workflow | `docker-compose.prod.yml`, `zavora-erp-api/Dockerfile`, `amos/Dockerfile`, `.github/workflows/deploy.yml` |

---

## 1. Accounting correctness (P0 / P1)

Ledger engine foundations are strong (balanced functional JE + ≤0.01 rounding,
period gates, DB immutability triggers, year-end atomic close, snapshots). Gaps
below break **book quality** or control-vs-subledger sign-off.

### 1.1 P0 — Journal / cash / multi-currency defects

- ✅ **P0 — Vendor payment unapplied leg inverted / unbalanced.** *(Fixed
  2026-07-09.)* Vendor excess now **DRs** a dedicated `unapplied_vendor_credits`
  posting account (default **9110** Unapplied Vendor Credits, an asset — the
  vendor owes it back): `DR AP (applied) + DR 9110 (excess) = CR Bank (total)`.
  Customer side unchanged on 9100 (liability). Verified live: a pure vendor
  advance posts `DR 9110 / CR 1020` balanced.

- ✅ **P0 — Vendor WHT auto-remitted through bank on every vendor payment.**
  *(Fixed 2026-07-09.)* The `DR WHT Payable / CR Bank` legs are removed from
  vendor payments (`payments.rs`): paying a bill clears the net AP only; the
  WHT liability stays on 3210 until `POST /tax-filings/{id}/remit` moves the
  cash to KRA. No more double-clear risk.

- ✅ **P0 — Bill / supplier-CN FCY posted as base currency.** *(Fixed
  2026-07-09.)* Bill post now carries the **bill's** currency + fx_rate on every
  line (`routes/bills.rs`), mirroring `post_invoice`. Verified live: a USD 100
  bill @130 posts `currency=USD, functional=13,000 KES`. (Supplier CN already
  posted in document currency — validation note corrected.)

- ✅ **P0 — Default inventory / GRNI accounts wrong vs Kenya COA.** *(Fixed
  2026-07-09.)* `PostingSetup` defaults now `inventory_asset = 1500` and
  `inventory_clearing = 3020` — a new seeded **Goods Received Not Invoiced**
  liability (COA template + migration `050` backfills existing entities).

- ✅ **P0 — Receive goods then bill double-counts; GRNI never cleared.** *(Fixed
  2026-07-09.)* Bill post now routes lines for **inventory-tracked products** to
  `DR inventory_clearing (GRNI)` instead of expense (`routes/bills.rs`): the
  receipt booked `DR Inventory / CR GRNI`, the bill clears GRNI, and COGS books
  at issue — no double count. Non-stock lines still hit their expense accounts.
  (Procurement GRN remains quantity-only; the GL pair is the standalone
  `receive_inventory` + bill.)

- ✅ **P0 — Fixed asset create does not capitalise.** *(Fixed 2026-07-09.)*
  `CreateAssetRequest` gains `funding_account`: when set, `create_asset` posts
  `DR asset account / CR funding` (bank / AP / opening-balance equity) in the
  same transaction (`JournalSource::FixedAsset`). Deliberately optional —
  an asset bought via a bill line coded to the FA account is already in the GL,
  and an unconditional JE would double-post. Verified live: `DR 2550 /
  CR 1020` for a 120k laptop.

- ✅ **P0 — AR ageing / dashboard open AR include drafts (and untyped docs).**
  *(Fixed 2026-07-09.)* AR ageing, dashboard receivable/overdue and outstanding
  lists now filter `status NOT IN ('draft','paid','voided','cancelled',
  'written_off') AND invoice_type = 'invoice'`; AP side excludes
  `draft`/`pending_approval`. Verified live: a 999,999 draft no longer moves
  `total_receivable`.

### 1.2 P1 — Control, close, tax reports, FX scope

- ✅ **P1 — No AR/AP control-account reconciliation report.** *(Fixed
  2026-07-09.)* New `ControlAccountRecon` report: Σ open invoice/bill balances
  (functional) vs the posted GL balance of every control account each side
  posts to (flat setup + business-group overrides), with per-side difference
  and in-balance flag. In the Reports launcher + CSV export.

- ✅ **P1 — Soft close traps operational AR/AP.** *(Fixed 2026-07-09.)* New
  per-tenant `period_controls.soft_close_allow_documents` (default OFF,
  migration `051`): when enabled, document sources (invoice / bill / credit
  note / payment) may post into soft-closed periods while everything else
  stays locked. Settable via the settings API (`SettingsPatch.period_controls`);
  Settings UI toggle is a follow-up.

- ✅ **P1 — Bill post not atomic with status update.** *(Fixed 2026-07-09.)*
  `post_bill` now posts the JE and flips the bill to `posted` in ONE
  transaction (`create_and_post_in_tx`), mirroring `post_invoice`.

- ✅ **P1 — Journal reverse does not set original status to `reversed`.**
  *(Fixed 2026-07-09.)* The reversal posting and the original's status flip to
  `reversed` commit in the same transaction.

- ✅ **P1 — `create_and_post_in_tx` skips account existence/active checks.**
  *(Fixed 2026-07-09.)* Every posting now verifies each line account exists
  and is active, naming the offenders in the error. Test harnesses seed the
  Kenya-standard chart so integration tests post like production tenants.

- ✅ **P1 — VAT return uses single flat VAT accounts only.** *(Fixed
  2026-07-09.)* The return aggregates every VAT account the tenant posts to:
  flat setup + all accounts routed by `vat_posting_matrix`.

- ✅ **P1 — FX revaluation revalues any residual FCY account.** *(Fixed
  2026-07-09.)* Scope is now monetary balance-sheet items only (IAS 21):
  Asset/Liability/Contra types minus inventory, fixed assets and accumulated
  depreciation. With the bill-FCY fix (§1.1), open FCY AP now enters the
  reval set correctly.

- ✅ **P1 — Supplier CN dimensions not carried on post.** *(Fixed 2026-07-09.)*
  Full-reversal copies read the bill lines' dimensions and the SCN reversal JE
  lines carry them — reversals credit the same analytical buckets the bill
  debited.

- ✅ **P1 — Period close has no pre-close checklist gate.** *(Fixed
  2026-07-09.)* Hard close runs a checklist — draft invoices, unposted bills,
  draft JEs dated in the period, depreciation not run through period end —
  and refuses with named blockers. `force=true` overrides knowingly and is
  recorded in the audit event. (Recon/tax-filed checks can be added to
  `pre_close_checklist` as those flows harden.)

- ⬜ **P2 — Accruals / deferrals / prepaid amortisation engines.**  
  COA has prepaid (`1400`); no first-class schedules (manual JE only).

- ⬜ **P2 — Cash flow (indirect) relies on fixed account ranges + plug.**  
  Approximate vs auditor worksheet (`services/reporting.rs` cash flow). Custom
  COA misses ranges.

- ⬜ **P2 — Consolidation depth.**  
  Multi-entity TB + intercompany AR/AP elim by shared KRA PIN
  (`routes/consolidation.rs`). No ownership %, goodwill, full equity elim.

- ⬜ **P2 — Opening balances do not seed AR/AP subledger documents.**  
  Opening TB JE only (`routes/onboarding.rs` `post_opening_balances`).

- 🟡 **P2 — Realised FX on payment.**  
  Partial document-rate vs payment-rate + WHT residual on receipts; uneven open-
  item multi-currency settlement coverage (`services/payments.rs`).

---

## 2. Tax & payroll accuracy

- ✅ **P0/P1 — Insurance relief always zero on pay run.** *(Fixed 2026-07-09.)*
  The run path now derives insurance relief (ITA s.31: 15% of premiums, capped
  KES 5,000/month) from deduction lines categorised `insurance` and feeds it to
  the compute engine. Unit-tested (uncapped + capped). Follow-up: a dedicated
  employee "insurance premium" field would be tidier than the deduction-category
  convention.

- ✅ **P1 — PAYE not rounded to nearest shilling.** *(Fixed 2026-07-09.)* Net
  PAYE now rounds to the nearest whole shilling (`compute.rs`), so payslips, the
  P10 and iTax agree to the cent. Unit-tested.

- 🟡 **P1 — iTax is record-only, not file export.** *(PAYE done 2026-07-09.)*
  New `GET /payroll/{run_id}/itax-csv` emits the KRA iTax `B_Employees_Dtls`
  layout, importable into the PAYE-return workbook. VAT/WHT iTax upload packs
  still to do; the VAT-return and WHT-schedule reports export generic CSV, not
  the iTax template layout.

- 🟡 **P1 — CIT is estimate only.** *(Provision posting added 2026-07-09.)*
  `POST /tax/cit/provision` books the provision (DR 8500 / CR 3510) as an
  incremental true-up; the estimate now adds back the tax expense so it's
  computed on profit before tax (no feedback loop). Still decision-support for
  the computation itself — the final CT computation/return remains iTax.

- ⬜ **P1 — eTIMS production ops maturity.**  
  OSCU/VSCU implemented (`services/etims.rs`); sandbox/prod credentials, failure
  UX, and live KRA validation still operational risk. Amos webhook on transmit
  failure helps but is not a substitute for device/ops runbook.

---

## 3. Product / Specs parity (Wave + Kenya extras)

From [`Specs.md`](Specs.md) and marketing vs code:

- ⬜ **P1 — Card payments (Flutterwave).**  
  Specs claim “Card via Flutterwave”. Settings toggle + key fields only
  (`settings/mod.rs` `flutterwave_*`, `SettingsPage.tsx`). **No charge or
  webhook flow.** Either implement or remove from Specs/pricing/UI.

- ⬜ **P1 — Bank auto-feeds (KCB / Equity / NCBA / open banking).**  
  Specs claim auto-import. Reality: **manual** statement import
  (CSV/MT940/OFX/PDF/Excel) only (`services/bank.rs`, Banking UI).

- ⬜ **P1 — CBK FX rates auto-load.**  
  Specs claim auto-loaded. Reality: manual FX rates (`FxRatesPage.tsx`); Amos
  may web-search rates but does not write CBK feed.

- ⬜ **P1 — Public invoice / payment portal + `viewed_at`.**  
  Specs: portal open stamps `viewed_at`. Column exists
  (`invoicing/invoice.rs`, migration `001`); **never set**. No public pay-link
  page. Customer portal is view/tickets only (`CustomerPortal.tsx`).

- ⬜ **P2 — Recurring invoice `auto_charge`.**  
  Field persisted (`recurring_invoices`, UI); **no** saved payment method or
  charge engine.

- 🟡 **P2 — Inventory FIFO.**  
  Specs/model mention FIFO layers (`inventory/mod.rs`); issue/receive use **WAC
  only** (`services/inventory.rs`). UI copy still mentions FIFO
  (`ProductsPage.tsx`).

- 🟡 **P2 — Multi-warehouse.**  
  `warehouse_id` on inventory model; no warehouse master/UI (type field only in
  UI types).

- 🟡 **P2 — AI bank categorisation.**  
  Specs “AI suggestion engine”. Reality: history-based `AiSuggestion`
  (`services/bank.rs` `suggest_from_history`); not model-powered.

- ⬜ **P2 — Invoice status “viewed”.**  
  No payment portal → no viewed tracking (related to public portal gap).

- ⬜ **P3 — Mobile native app.**  
  Specs explicit v2; web shell also not mobile-ready (§6).

- 🟡 **P2 — Statutory fields on document PDFs.**  
  Registration number / address / phone captured in Settings
  ([`docs/UI_GAPS.md`](docs/UI_GAPS.md) #6 fixed); **still partial on invoice/
  statement PDF headers** (UI_GAPS #9 open).

- 🟡 **P3 — Base currency / fiscal year-end at signup.**  
  Editable in Settings (UI_GAPS #5 fixed); not on Create Organization form
  (UI_GAPS #8 deferred — `provision_tenant` + period seed).

---

## 4. Module depth (built shell, missing legs)

### 4.1 HR lifecycle — [`docs/HR_MODULE_SPEC.md`](docs/HR_MODULE_SPEC.md)

- ✅ Phase 1-ish: leave + ESS (payslips/leave/profile) — largely shipped.  
- 🟡 Phase 2: onboarding cases UI/API (`services/hr_onboarding.rs`,
  `OnboardingPage.tsx`) — hire-from-candidate not full ATS.  
- ⬜ **P2 — Phase 3 Recruitment (ATS).** Jobs, public careers board, candidate
  pipeline. Marketing `/careers` is static content (`InfoPage.tsx`), not ATS.  
- ⬜ **P2 — Timesheets / attendance** feeding casual/contract pay.  
- ⬜ **P3 — Phase 4 polish:** leave accrual automation in scheduler, leave
  liability report, org chart from `manager_id`, HR dashboard widgets.  
- ⬜ **P2 — Full offboarding** (leave encashment, checklist completeness).

### 4.2 Inventory / POS

- ✅ **P0 — Product “track inventory” does not create stock master (E2E break).**
  *(Fixed 2026-07-09.)* `create_product` now requires a SKU when
  `track_inventory`, creates the linked `inventory_item`, sets
  `products.inventory_item_id`, and books honest opening stock (qty **and**
  unit cost, both wired through from `ProductsPage.tsx`) as
  `DR inventory / CR 9300 Opening Balance Equity`. Verified live end-to-end.
  Enabling tracking on an *existing* product via edit also creates + links the
  item now (SKU required up front; stock arrives via receive/adjust so
  quantities stay auditable).  
- 🟡 POS: session sell + Z + mobile stock only; **no refund/void/hold/offline**
  (`services/pos.rs`, `pages/pos/*`).  
- 🟡 POS nav split: Sell / Till Sessions / Stock (Mobile) as three sidebar items.

### 4.3 CRM / portals

- ✅ Optional CRM core complete per `docs/CRM_MODULE_SPEC.md`.  
- ⬜ **P2 — CRM v2:** email integration, sequences, opportunity → estimate, richer
  contacts.  
- ⬜ **P1 — Customer portal pay-online** (even M-Pesa-only).  
- ⬜ **P2 — Staff ESS expense claims / richer self-service** (claims exist
  back-office only: `ExpenseClaimsPage.tsx`).  
- 🟡 Vendor portal: tenders/bids/POs/statement; limited dispute/messaging.

### 4.4 Procurement residual

- ✅ Core P2P shipped (see “Already shipped”).  
- 🟡 **P2 — Match exception workflow** (tolerances, partial-bill policy, match
  approval roles) beyond qty/price report + bill gate.  
- 🟡 **P2 — Budget commitment enforcement** vs analytics-only
  (`budget_commitments` in procurement service).

### 4.5 Amos

See **§7** (full Amos AI backlog from fourth-pass audit). Do not track
coverage/security gaps only here.

### 4.6 Sample data

- 🟡 Signup sample company seeds customers/vendors/products/invoices only
  (`services/sample_data.rs`) — no bills, payroll, inventory, bank recon story.

---

## 5. Security hardening

- ✅ **Amos session tenant isolation** (2026-07-05) — JWT entity gate, wrong-
  tenant honesty, portal Vendor/Employee refuse, REST JWT, webhook-only ops
  trigger. See CHANGELOG / `docs/AMOS.md` §5b.  
  **Not complete authority:** Amos tool scope + service-account execution still
  open (see **§7.1**).  
- ✅ **P0/P1 — CORS lockdown.** *(Fixed 2026-07-09.)* `main.rs` now builds the
  layer from `CORS_ALLOWED_ORIGINS` (comma-separated; unset → localhost dev
  origins only; `*` → explicit permissive escape hatch). Credentials allowed for
  the listed origins so the refresh cookie works. Verified: foreign origins get
  no `access-control-allow-origin`. **Set `CORS_ALLOWED_ORIGINS` in the prod
  deploy env.**  
- ⬜ **P1 — M-Pesa callback authenticity.** Idempotency + orphan recovery done;
  **no** IP allowlist / signature validation
  (`routes/payments.rs` `mpesa_callback`, `payments/daraja.rs`).  
- ⬜ **P1 — TLS termination** + secrets in managed store (startup secret
  validation already fails fast). Prod Caddy in `docker-compose.prod.yml` is
  the intended TLS edge — document/ops still incomplete.  
- ⬜ **P2 — Rate limiting & body limits** on login, webhooks, uploads generally.
  Signup has Redis rate limit (`routes/auth_signup.rs`); not global
  (`governor` / `DefaultBodyLimit`).  
- ⬜ **P2 — Graceful API shutdown drain** (Amos has graceful shutdown; API called
  out historically as missing).

---

## 6. UX / end-to-end failure modes

Things that **look complete** but **fail user journeys**. Cross-ref §1–§4 for
accounting/product roots of some UX failures.

### 6.1 P0 — Broken happy paths

- ✅ **Tracked goods product → post invoice** *(fixed 2026-07-09 — see §4.2:
  create AND edit now create/link the stock item).*  
- ⬜ **Desktop-only shell.** Fixed 260px sidebar + `pl-[260px]`, no hamburger
  (`AppShell.tsx`, `Sidebar.tsx`) — phone/tablet unusable.  
- ✅ **Header search is decorative.** *(Fixed 2026-07-09.)* Now a working ⌘K
  command palette (`CommandPalette.tsx`): the header search and Cmd/Ctrl-K open
  it; type-to-filter over every app page (sourced from the sidebar nav),
  arrow-key + Enter to jump.  
- ⬜ **Silent mutation failures on critical actions.** e.g. invoice list
  `postMutation` / `deleteMutation` have no `onError`
  (`InvoicesPage.tsx`) — credit limit / stock / period errors invisible.  
- 🟡 **RBAC UI vs API drift.** *(Nav gated 2026-07-09.)* The sidebar now hides
  destinations the user lacks the read-permission for (`usePermissions().can()`
  + an href→permission map; unmapped items stay visible, backend still
  enforces), and empty section headers collapse. Per-page ACTION buttons
  (Post/Send/Approve) still lean on coarse `hasRole` — migrating those to
  `can()` is the remaining drift.
  <!-- superseded detail: --> Backend: granular perms + route registry.
  Most UI buttons use coarse `hasRole(ROLES_POST|…)` (`utils/roles.ts`).
  `usePermissions().can()` used almost only on Users/Roles pages. Custom roles
  (e.g. “Bookkeeper”) **hide** Post/Send even when allowed, or show actions that
  **403**. Sidebar **not filtered** by permission — Viewer sees full nav.

### 6.2 P1 — Misleading success / friction

- ⬜ **Email/SMS/WhatsApp send without delivery config.** Send invoice modal can
  mark sent / queue (`InvoicesPage.tsx` send flow; `send_invoice` degrades when
  SMTP unconfigured — `services/invoicing.rs`). No pre-flight “Email not
  configured → Notification providers” gate. Same for invites / forgot-password
  (always-neutral success — `ForgotPasswordPage.tsx`).  
- ⬜ **Nav overload (~50 items, 10 sections)** always shows Procurement/POS/
  Payroll/CRM for pure service SMEs (`Sidebar.tsx`).  
- ⬜ **Invoice/bill status tab counts are page-local**, not server totals
  (`InvoicesPage.tsx` filters `invoices` page slice).  
- ⬜ **Overdue tab depends on hourly scheduler** (`scheduler.rs` marks
  `overdue`); fresh past-due stay `posted` until tick.  
- ⬜ **No “Pay” on bill/invoice list rows** — must navigate Payments and re-pick
  party.  
- ⬜ **Apply unapplied payment requires pasting document UUID**
  (`PaymentsPage.tsx`) — no open-doc picker.  
- ⬜ **Credit note UI = full reverse only** (reason + `lines: []`)
  (`InvoiceDetailPage.tsx` CreditNoteModal) — no partial lines.  
- ✅ **Estimate convert does not open new invoice** *(Fixed 2026-07-09.)* The
  convert mutation now navigates to the created invoice (`?highlight=<id>`) and
  surfaces errors, instead of silently staying on the estimates list.  
- ⬜ **Bill approve → post two-step** with tiny row buttons; easy to leave
  approved unposted (`BillsPage.tsx`).  
- ⬜ **Banking import vs Reconciliation vs Transactions** split across three
  nav items without guided flow.  
- ⬜ **Amos iframe blank if service down** (dev `localhost:8090`; prod
  `/amos-app`) — `AmosPage.tsx`.  
- ⬜ **Tax remittance UI requires raw liability/bank GL codes**
  (`TaxFilingsPage.tsx`) — weak labelling for non-accountants.  
- ⬜ **Inconsistent feedback:** mix of inline toasts (Settings), `window.alert`
  (POS, products, FX, assets…), silent mutations. No global toast.

### 6.3 P2 — Portal / secondary journeys

- ⬜ Customer portal: no pay; unlinked account empty state only.  
- ⬜ Staff ESS: no expense claim submit.  
- ⬜ POS: popup-blocked print; no refund UX.  
- ⬜ Import page: customers/vendors/products CSV only; row-by-row; no opening
  balances/invoices (`ImportPage.tsx`).  
- ⬜ Work-as-of date powerful but easy to mis-post without form-level banner
  (`utils/workDate.ts`, UserMenu only).

### 6.4 Open UI_GAPS items — [`docs/UI_GAPS.md`](docs/UI_GAPS.md)

- 🟡 Capture base currency / year-end at signup (#8).  
- 🟡 Surface company statutory fields on invoice/PDF templates (#9).

---

## 7. Amos AI (agent layer)

**Primary paths:** `amos/src/{auth,scope,mcp,agent,ops,persona,plan,guard,memory,routes}.rs`,
`amos/{system.md,AGENTS.md,mcp.json,skills/,routines/}`, ERP embed
`zavora-erp-ui/src/pages/amos/AmosPage.tsx`, program doc [`docs/AMOS.md`](docs/AMOS.md).

**Identity model (three principals):**

1. **Human session** — user’s ERP JWT (WS handshake + REST); gates served entity.  
2. **MCP ERP tools** — `ZAVORA_EMAIL` / `ZAVORA_PASSWORD` service account
   (`amos/mcp.json`, `amos/src/erp.rs`) — **not** the human’s token.  
3. **Browser showcase** — `ERP_LOGIN_EMAIL` / `ERP_LOGIN_PASSWORD` (often same as service user).

### 7.0 Already solid (do not re-open as “missing product”)

- Gemini Live voice + chat; workplan / evidence / memory / past sessions / routines UI.  
- One-tenant identity gate + typed refusals (`wrong_tenant`, `portal_account` for
  Vendor/Employee) — `auth.rs`, `routes.rs`.  
- REST `/api/*` + `/showcase/*` JWT-gated; webhook secret only for
  `POST /api/ops/run/*`.  
- Entity-scoped memory + session history; forget/dedup.  
- **16 skills** (test-pinned in `skills.rs`): record-vendor-bill,
  record-customer-invoice, record-payment, inventory-ops, bank-reconciliation,
  tax-filing, month-end-review, manage-procurement, financial-reporting,
  manual-journal, hr-payroll, crm, payment-run, management-accounts,
  cash-forecast, erp-showcase.  
- **11 ambient routines** (`routines/*.toml`): morning-briefing, etims-sweep,
  recon-nudge, ar-chase, paye-prep, vat-prep, month-end-pack, installment-tax,
  payment-run-prep, management-pack, annual-accounts.  
- Sub-agents: `analyze_attachment`, `web_search` (plan-gated); session clock.  
- Showcase auto-login wrap; prompt injection substring guard; remember secret filter.  
- Deploy: `amos/Dockerfile`, Caddy `/amos-app`, deploy secrets.

### 7.1 P0 — Authority & write safety

- ✅ **P0 — Incomplete `required_scopes` (many writes treated as `erp:read`).**
  *(Fixed 2026-07-09.)* `amos/src/scope.rs` now classifies the full mcp-erp
  mutating surface: postings/statutory/period-locks (incl. `post_pay_run`,
  `approve_pay_run`, `mark_pay_run_paid`, `close_period`, `reopen_period`,
  `file_tax_return`, `remit_tax_filing`, `etims_transmit_invoice`,
  `run_depreciation`, `run_fx_revaluation`, `complete_reconciliation`,
  `receive_goods`, `adjust_stock`, `create_debit_note`) → `ledger:post`;
  drafts/masters/payroll-prep/procurement-workflow/sends/imports/budgets →
  `erp:write`. A unit test pins every mutating tool name so an unclassified
  addition fails the build. “Viewer cannot post” is now true across the board.

- ✅ **P0 — Confirm-before-write is prompt-only.** *(Fixed 2026-07-09.)* Now a
  code gate: in interactive sessions, every `ledger:post` tool call blocks
  inside `ScopedTool` until the user clicks **Approve & post** on a card in the
  chat (tool name + args preview). Decline or a 120s timeout returns a refusal
  to the model and audits `Denied` — a spoken "yes" cannot post. Wire-up:
  `SessionState::Confirmations` (oneshot broker) → `confirm_request`/`confirm`
  WS frames → Approve/Decline card in `assets/index.html`. Ambient routines
  pass no session handle and stay deliberately unattended (eTIMS sweep).
  `AMOS_CONFIRM_WRITES=0` is the explicit demo/dev escape hatch.

- ⬜ **P0 — Tool execution as service account bypasses human ERP RBAC.**  
  mcp-erp logs in as `ZAVORA_*` (`mcp.json` / `erp.rs`). Session scopes are a
  pre-filter only; API sees Accountant-class service user. Ledger actor ≠ human
  confirmer. Compliance relies on Amos transcript/`amos_audit_events`, not ERP
  actor. *Mitigated 2026-07-09 by the confirm-before-write code gate above
  (every posting now has a recorded human click behind it), but the ERP actor
  is still the service user.*  
  **Fix design (cross-repo, do as its own change):** amos retains the session
  JWT from the WS handshake and injects it per tool call (`__user_token` arg
  added AFTER the model turn, stripped from anything echoed back);
  `mcp-erp/src/zavora.rs` uses it as the bearer for that request, falling back
  to the service login. Open issue to solve first: access tokens live 15 min —
  the embedded shell must push refreshed tokens over the existing `context`
  WS frame or writes start 401ing mid-session. Needs coordinated releases of
  amos + mcp-erp (`amos/Dockerfile` pins `MCP_ERP_REF`).

### 7.2 P1 — Roles, persona, multi-tenant product, plans

- ⬜ **P1 — Coarse role → scope map vs RBAC v2.**  
  `Principal::scopes` (`auth.rs`): Owner/Admin/Accountant → write+post; Approver
  → write; **else read-only**. Ignores granular permissions. **Editor** and
  custom roles (e.g. Bookkeeper) mis-mapped.  
  Block **Customer** portal role like Vendor/Employee (currently not in the
  portal refuse list).  
  Prefer scopes from `GET /auth/permissions` or JWT permission claims.

- ⬜ **P1 — Hardcoded company persona.**  
  `amos/system.md` + `AGENTS.md` assume **Zavora Technologies Ltd** (not VAT-
  registered, FY 2025, etc.). `persona.rs` only substitutes `{ui_url}`, skills,
  agents rules, memories, ops, now — **not** settings. UI `/api/context` has real
  company name; model instruction does not. Wrong for any other tenant’s Amos
  instance.  
  **Fix:** `{company_context}` from `/settings` (name, currency, VAT flag, FY).

- ⬜ **P1 — One Amos = one tenant (ops/product).**  
  By design (`AMOS_SERVED_ENTITY_ID` or service entity). Multi-tenant SaaS =
  **N Amos processes**. Documented future path (`docs/AMOS.md` §6). Not a bug;
  still a scale/product gap.

- ⬜ **P1 — Plan entitlements spoofable.**  
  `plan.rs` trusts WS handshake / `AMOS_PLAN` env for voice + web_search. No
  server-side billing claim. Client can send `business`/`scale`.

- ⬜ **P1 — Memory is entity-scoped, not per-user.**  
  All users of the tenant share profile/lessons (`memory.rs` `user_scope` =
  entity id). Privacy/confusion risk multi-user tenants.

- ⬜ **P1 — Docs drift.**  
  `docs/AMOS.md` §2.2 still describes ~13 skills / old coverage; §2.7 routine
  table incomplete (7 vs 11). Coverage map still lists asset/FX as UI-only while
  tools exist (`list_fixed_assets`, `run_depreciation`, `run_fx_revaluation` —
  pinned in `skills.rs` test). Refresh docs to match code.

### 7.3 P2 — Capability / tool surface

- 🟡 **UI-only for agent** (`system.md` coverage map; still accurate for these):  
  AR credit notes, estimates/quotes, recurring journal templates, budgets setup
  UX, POS shift management. Prefer MCP tools + skills when product prioritises.  
- 🟡 **Skill tools always on.** `mcp.rs` `agent_tools` = base `ERP_TOOLS` ∪
  browser ∪ **union of all skills’ allowed-tools** (not per-active-skill).
  Progressive disclosure is playbook text only. Gemini Live tool-count pressure
  (`docs/AMOS.md` §6).  
- ⬜ No skills for: FA **capitalisation**, opening balances, consolidation,
  partial CN/write-off, user/RBAC admin, eTIMS **device setup**.  
- 🟡 CRM skill UI-first; flag-gated CRM returns API errors if disabled — no hard
  Amos gate.  
- 🟡 Inherits ERP correctness bugs (§1 accounting, §4.2 inventory product link,
  payroll relief) — skills assume ledger math is right.

### 7.4 P2 — Security hardening (Amos-specific)

- 🟡 Prompt injection: substring list only (`guard.rs`) — paraphrase bypass.  
- 🟡 `looks_like_secret` keyword filter on remember — not real secret detection.  
- ⬜ Never set `AMOS_DEV_ALLOW_UNAUTH=1` in prod (`routes.rs`).  
- 🟡 Showcase retention/caps exist (CHANGELOG); keep monitoring disk/PII.

### 7.5 P2/P3 — Ops, UX, tests

- ⬜ **P1/P2 — Embed blank if Amos process down** (`AmosPage.tsx` iframe to
  `:8090` / `/amos-app`) — no ERP fallback shell message.  
- 🟡 Ambient ops quality depends on ERP notification providers + live ledger.  
- 🟡 Deploy coupling: path/`Dockerfile` builds `adk-rust` + `mcp-erp` outside
  ERP workspace (root `Cargo.toml` note). Branch/pin drift risk.  
- 🟡 SIGKILL leaks MCP/Playwright children (docs note).  
- ⬜ **Tests missing:** scope matrix for all skill-unlocked mutators; Viewer
  cannot `complete_reconciliation` / `post_pay_run` / `etims_transmit`; hard
  confirm gate (once built); persona settings injection; plan spoof resistance.  
  (Existing: skill pack pin, routine registry safety, plan resolve, memory
  hygiene, clock, subagents.)  
- ⬜ **P3 — Single-process multi-tenant Amos** (per-user token data path).  
- ⬜ **P3 — Hard “Approve posting” UI control** (dual-control beyond chat yes).

### 7.6 Suggested Amos fix order

1. Complete `required_scopes` + regression test (P0).  
2. Hard confirm gate for ledger writes in live sessions (P0).  
3. User-scoped MCP auth / audit actor = human (P0/P1).  
4. Scopes from permissions + block Customer; fix Editor/custom (P1).  
5. Dynamic `{company_context}` from settings (P1).  
6. Refresh `docs/AMOS.md` tables (P1).  
7. Server-bound plan claims for voice/web_search (P1).  
8. Skills for CN/estimates/POS if product priority (P2).  
9. Embed offline fallback (P2).

---

## 8. Operations & quality

- ✅ **P1 — PR CI quality gate.** *(Fixed 2026-07-09.)* `.github/workflows/ci.yml`
  runs on every PR to main: cargo build + full test suite against Postgres/Redis
  service containers (migrations run via the test harness), amos build + tests
  (adk-rust cloned as sibling), UI `tsc --noEmit` + production build. Clippy
  `-D warnings` deferred until the warning backlog is cleared (§8 P3). Once the
  workflow has a green history, add the three jobs as required status checks on
  the `main-pr` ruleset.  
- 🟡 **P1 — Containerization & deploy.** **Exists** (`docker-compose.prod.yml`,
  Dockerfiles, deploy workflow). Remaining: readiness probes maturity, graceful
  API drain, runbooks.  
- ⬜ **P1 — Backups & migration safety.** No `docs/BACKUP_RUNBOOK.md`;
  pg_dump/pg_restore procedure + destructive-migration review (include
  `amos_sessions` / `amos_audit_events` / memory tables).  
- 🟡 **P1 — Automated tests.** ~150–180 unit/integration-style tests across
  crates (grew past old “49”). Strong pockets: money, statutory goldens, payment
  flows, OCR parse, authz registry, some payroll/CIT, Amos skill/routine pins.
  **Gaps:** period close properties, FX reval, posting-group matrices, tenant
  isolation fuzz, eTIMS contract vs KRA sandbox, procurement 3-way edges, POS
  cash variance, portal abuse, vendor overpay JE, product→inventory→invoice E2E,
  Amos scope matrix (§7.1), load/N+1. Expand toward catalogue in
  `docs/production-readiness/tasks.md` (file self-stale on checkboxes — use as
  property list only).  
- ⬜ **P2 — Observability.** Structured logs directionally present; Prometheus
  `/metrics`, OpenTelemetry tracing, durable consumer for Redis audit stream;
  Amos tool-latency metrics.  
- ⬜ **P2 — Performance.** N+1 on invoice/bill/payment detail; list endpoints
  under load; customer dropdown loads full unpaginated list.  
- ⬜ **P3 — Build-warning cleanup** + `-D warnings` in CI.

---

## 9. Reporting roadmap tail

See [`docs/REPORTING_ROADMAP.md`](docs/REPORTING_ROADMAP.md). Phases 1–3 and 6
done; remaining:

- 🟡 **Phase 4 — Numeric tie-out tests** for bills/payroll/inventory/bank-rec
  reports (SQL re-validated; integration tie-out tests pending).  
- 🟡 **Phase 5 — Dimensions tail.** Per-line bill dimensions **on bills are
  done**; supplier CN dimensions still missing (§1). Per-account capture
  controls; snapshot key account+dimension+period (Option B) if volume warrants.  
- ✅ **P1 — Ageing/report filters** exclude drafts / non-invoice types *(fixed
  2026-07-09 with the §1.1 AR ageing fix — same commit covers dashboard +
  ageing)*.  
- ⬜ **P1 — VAT return multi-account** (align with §1.2).

---

## 10. Suggested sequence

### Wave A — Trust the books & money (P0)

1. Vendor payment unapplied + separate vendor advance account.  
2. Stop auto-remitting WHT on vendor pay; remit only via tax filing.  
3. Bill/supplier-CN multi-currency = document currency (mirror invoice).  
4. Fix `PostingSetup` defaults: inventory **1500**, GRNI dedicated non-control.  
5. Product track-inventory → create/link inventory item (+ honest opening stock).  
6. AR/AP ageing & dashboard: posted invoices/bills only.  
7. Fixed-asset capitalisation JE (or explicit Capitalise action).  
8. Stock purchase path: bill clears GRNI when received.  
9. Bill post atomicity; surface JE errors in UI.  
10. **Amos:** complete `required_scopes` + tests; hard confirm gate for posts
    (§7.1).

### Wave B — Go-live hardening (P1)

11. CORS + M-Pesa callback auth + rate limits on auth/webhooks.  
12. PR CI (test/clippy/tsc + amos tests when feasible) before deploy.  
13. Backup/restore runbook + drill.  
14. Payroll insurance relief field + nearest-shilling policy + tax-pro review.  
15. Permission-aware nav + `can()` on all action buttons.  
16. Global mutation error toast; honest “email not configured”.  
17. Real search or remove header search.  
18. Responsive shell.  
19. AR/AP control recon report; VAT return all VAT accounts; FX reval scope.  
20. **Amos:** user-scoped MCP auth; permission-based scopes; dynamic company
    context; refresh `docs/AMOS.md` (§7.2).

### Wave C — Product honesty & depth (P2)

21. Flutterwave: implement or strip from Specs/pricing.  
22. Public invoice view/pay (M-Pesa) + `viewed_at`.  
23. Pay shortcuts + unapplied document picker + partial credit notes.  
24. FIFO/warehouse only if merchandising tenants need them.  
25. HR ATS/timesheets only if product prioritises them.  
26. **Amos:** tools/skills for CN, estimates, POS; plan binding; embed offline
    fallback (§7.3–7.5).  
27. Observability + performance pass.

### Wave D — Polish (P3)

28. Nav IA / role-based module packs.  
29. Accrual engines, consolidation depth, native mobile.  
30. Build-warning cleanup; multi-tenant single-process Amos if ever required.

---

## 11. Reference index (audit trail)

| Pass | Focus | Primary code / docs |
|---|---|---|
| 1 Functional / product | Specs parity, modules, ops | `Specs.md`, `CHANGELOG.md`, migrations `001`–`049`, UI `App.tsx` routes |
| 2 Accounting | JE integrity, tax, inventory, FX | `services/journal.rs`, `payments.rs`, `invoicing.rs`, `bills` routes, `inventory.rs`, `assets.rs`, `fx.rs`, `reporting.rs`, `posting/mod.rs`, `coa_template.rs`, triggers `002`/`026` |
| 3 UX E2E | Journeys that fail | `Sidebar.tsx`, `AppShell.tsx`, `Header.tsx`, `ProductsPage.tsx`, `InvoicesPage.tsx`, `PaymentsPage.tsx`, `utils/roles.ts`, `hooks/usePermissions.ts`, portals, `AmosPage.tsx` |
| 4 Amos AI | Agent authority, skills, ops, persona | `amos/src/{auth,scope,mcp,agent,ops,persona,plan,guard,memory,routes}.rs`, `amos/{system.md,AGENTS.md,mcp.json,skills/,routines/}`, `docs/AMOS.md` |

_When closing an item: delete or move to CHANGELOG with date; do not leave stale ⬜._
