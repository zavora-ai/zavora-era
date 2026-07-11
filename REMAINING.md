# Remaining Work — Zavora ERP

**Single source of truth for what is *not* yet done (or is incomplete).** For what
*is* done, see [`CHANGELOG.md`](CHANGELOG.md). Keep this file honest — verify
against the code before marking anything done, and move completed items into the
changelog rather than leaving stale ticks behind.

Legend: ⬜ not started · 🟡 partial · ✅ done (kept briefly for context, then moved to CHANGELOG)  
Priority: **P0** blocker / correctness · **P1** before go-live · **P2** fast-follow · **P3** polish

_Last reconciled against the codebase: **2026-07-10**_  
_Source: four-pass audit (product/functional · accounting · UX E2E · **Amos AI**)
against `feat/portals-page` / main tip ≈ `85ecf01`._

> **Reconciled 2026-07-10 (PRs #79–#93).** A run of merged PRs closed several
> items this file listed open on 2026-07-09 — now flipped to ✅ inline and
> logged in [`CHANGELOG.md`](CHANGELOG.md) (2026-07-10): public invoice view/pay
> + `viewed_at` (§3, #85/#86), CBK FX auto-load (§3, #89), intercompany + group
> consolidation (§1.2, #90), optional multi-warehouse + 3PL (§3, #92), backup/
> restore runbook (§8, #93), responsive shell (§6.1, #80/#91), global toasts
> (§6.2, #81), send pre-flight (§6.2, #84), `can()` action buttons (§6.1, #83),
> Amos dynamic company persona (§7.2, #82), Amos role→scope + Customer block
> (§7.2, #87), and **user-scoped MCP auth — the §7.1 P0** (ledger actor is now
> the human, not the service account; cross-repo amos + mcp-erp). The remaining
> go-live P1 ops are **bank auto-feeds (§3)** and **eTIMS prod maturity (§2)**;
> the Amos P0 still needs a coordinated amos+mcp-erp release + client token
> refresh over the `context` frame (§7.1).

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
| **Card payments (Paystack)** — initialise → hosted `authorization_url` → HMAC-signed `charge.success` webhook records money; reusable auth codes | `zavora-erp-core/src/payments/paystack.rs`, `routes/payments.rs` (`paystack_initialize`, public `paystack_webhook`), migration `053_paystack.sql`, PR #72 (replaces the Flutterwave stub) |
| **SaaS subscription billing (Paystack)** — Free activates instantly; paid plans checkout at signup; status mirrors to `tenants.plan_key`; auto-renewals via reusable auth codes; manage-subscription screen | `zavora-erp-core/src/services/billing.rs`, `routes/billing.rs`, `SubscriptionTab.tsx`, migration `054_subscription_billing.sql`, PRs #73/#74 |
| **Public invoice view/pay portal + `viewed_at`** — tokenised `/pay/:token` page stamps `viewed_at`, Paystack checkout, copy/send pay-link | `services/public_invoice.rs`, migration `058_invoice_public_token.sql`, `PublicInvoicePage.tsx`, PRs #85/#86 |
| **CBK daily FX auto-load** — scheduler-driven rate sync + manual button | `services/fx.rs` `sync_cbk_rates`, `POST /fx-rates/sync-cbk`, `FxRatesPage.tsx`, PR #89 |
| **Intercompany + group consolidation** — both-sided IC charge, IC control accounts, consolidation with IC elimination, group-management UI | `services/intercompany.rs`, `services/consolidation.rs`, `routes/consolidation.rs`, migration `059_intercompany.sql`, PR #90 |
| **Optional multi-warehouse + 3PL** — warehouses (own/3PL), per-warehouse stock, transfers; non-breaking hooks keep `SUM(warehouse_stock)=on_hand` | `services/warehousing.rs`, `routes/warehouses.rs`, migration `060_warehousing.sql`, `WarehousesPage.tsx`, PR #92 |
| **Backup/restore runbook + migration safety** | `docs/BACKUP_RUNBOOK.md`, PR #93 |
| **Manufacturing v1 (BOM + work orders)** — recipe of components (+ labour/overhead), two-step work order (issue→WIP, complete→finished goods) with WAC costing; WIP (1510) nets to zero; reuses inventory + warehousing + journal engine. **v2 backlog:** [`docs/MANUFACTURING_ROADMAP.md`](docs/MANUFACTURING_ROADMAP.md) (routing/work-centres, capacity, MRP, scrap/yield variance, multi-level BOM, subcontracting, FIFO) | `services/manufacturing.rs`, `routes/manufacturing.rs`, migration `061_manufacturing.sql`, `ManufacturingPage.tsx` |

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

- ✅ **P2 — Accruals / deferrals / prepaid amortisation engines.** *(Fixed
  2026-07-09.)* First-class amortisation schedules (`services/amortization.rs`,
  migration `055`): a prepaid-expense or deferred-revenue schedule spreads an
  upfront amount over N months, auto-posting each installment via the hourly
  scheduler with idempotent catch-up (mirrors depreciation), the last period
  absorbing the rounding remainder so the holding account clears exactly. New
  3450 Deferred Revenue account; Accounting -> Amortisation UI (create / run /
  cancel). Verified live: a 120k/12-month prepaid posted 7 catch-up months of
  DR 7400 / CR 1400 10,000 each.

- ⬜ **P2 — Cash flow (indirect) relies on fixed account ranges + plug.**  
  Approximate vs auditor worksheet (`services/reporting.rs` cash flow). Custom
  COA misses ranges.

- 🟡 **P2 — Consolidation depth.** *(Deepened 2026-07-10, PR #90.)*
  Intercompany accounting (`services/intercompany.rs`, IC control accounts
  1250/3030/5180/7160) + group consolidation with **IC elimination**
  (`services/consolidation.rs`, `/consolidation/*`) + group-management UI now
  shipped. Still open: **ownership %, goodwill, full equity elimination** for
  partial/minority holdings.

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

- ✅ **P1 — Card payments.** *(Shipped — Paystack, replaces the Flutterwave stub.)*
  `payments/paystack.rs` initialises a transaction → hosted `authorization_url`
  → the payer pays → an HMAC-signed `charge.success` webhook
  (`verify_signature` over the raw body) records the money, with reusable
  authorization codes for re-charging. Routes `POST /payments/paystack/initialize`
  + public `POST /payments/paystack/webhook`; migration `053_paystack.sql`
  (PR #72). Settings now expose `paystack_enabled` + `paystack_public_key`
  (secret lives only in `PAYSTACK_SECRET_KEY`). A separate **subscription-billing**
  layer (`services/billing.rs`, migration `054`, PRs #73/#74) routes paid-plan
  signups through Paystack checkout and mirrors status onto `tenants.plan_key`.

- ⬜ **P1 — Bank auto-feeds (KCB / Equity / NCBA / open banking).**  
  Specs claim auto-import. Reality: **manual** statement import
  (CSV/MT940/OFX/PDF/Excel) only (`services/bank.rs`, Banking UI).

- ✅ **P1 — CBK FX rates auto-load.** *(Shipped 2026-07-10, PR #89.)*
  `services/fx.rs` `sync_cbk_rates` + a daily-guarded scheduler job
  (`sync_cbk_rates_all` on the hourly tick), `POST /fx-rates/sync-cbk`, and a
  "Load CBK rates" button on the Fx page. `FX_PROVIDER_URL` (default
  `https://api.frankfurter.dev`).

- ✅ **P1 — Public invoice / payment portal + `viewed_at`.** *(Shipped
  2026-07-10, PR #85/#86.)* A tokenised public `/pay/:token` page
  (`services/public_invoice.rs`, migration `058`) stamps `viewed_at` on open and
  initialises Paystack checkout (reuses `paystack_initialize`); the invoice
  detail can copy/send a pay-link. Closes the `viewed_at`, "invoice status
  viewed", and customer-portal-pay gaps together.

- ⬜ **P2 — Recurring invoice `auto_charge`.**  
  Field persisted (`recurring_invoices`, UI); **no** saved payment method or
  charge engine.

- 🟡 **P2 — Inventory FIFO.**  
  Specs/model mention FIFO layers (`inventory/mod.rs`); issue/receive use **WAC
  only** (`services/inventory.rs`). UI copy still mentions FIFO
  (`ProductsPage.tsx`). Now also the multi-warehouse follow-up (per-warehouse
  costing layers).

- ✅ **P2 — Multi-warehouse.** *(Shipped 2026-07-10, PR #92.)* Optional
  multi-warehouse + **3PL** layer on top of inventory (migration `060`:
  `warehouses` own/3PL, `warehouse_stock`, `warehouse_transfers`;
  `services/warehousing.rs`; non-breaking stock-delta hooks into
  receive/issue/adjust keeping `SUM(warehouse_stock)=on_hand`; `/warehouses` API
  + Warehouses UI). Follow-ups: per-warehouse costing/FIFO, 3PL storage-fee
  billing, per-warehouse reorder points, in-transit transfers.

- 🟡 **P2 — AI bank categorisation.**  
  Specs “AI suggestion engine”. Reality: history-based `AiSuggestion`
  (`services/bank.rs` `suggest_from_history`); not model-powered.

- ✅ **P2 — Invoice status "viewed".** *(Shipped 2026-07-10, PR #85.)* The
  public pay portal now stamps `viewed_at` on open (see the portal item above).

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
- ✅ **P1 — M-Pesa callback authenticity.** *(Fixed 2026-07-09.)* The callback
  now verifies a URL secret (`MPESA_CALLBACK_SECRET`, embedded in the registered
  callback URL as `?token=`) and an optional source-IP allowlist
  (`MPESA_CALLBACK_ALLOWED_IPS`) before touching the ledger. Verified: bad/absent
  token → 401, correct token → proceeds. **Set `MPESA_CALLBACK_SECRET` in prod.**  
- ⬜ **P1 — TLS termination** + secrets in managed store (startup secret
  validation already fails fast). Prod Caddy in `docker-compose.prod.yml` is
  the intended TLS edge — document/ops still incomplete.  
- ✅ **P2 — Rate limiting & body limits.** *(Fixed 2026-07-09.)* Per-IP
  fixed-window rate limit on the credential routes (login, register, signup,
  forgot-password, portal register/login) via a small in-memory limiter
  (`middleware/rate_limit.rs`; `LOGIN_RATE_LIMIT` / `_WINDOW_SECS`, 0 disables).
  Request bodies capped globally (`DefaultBodyLimit`, default 5 MiB,
  `MAX_BODY_BYTES`). Verified: 4th rapid login → 429. (Signup keeps its own
  Redis limiter.)  
- ✅ **P2 — Graceful API shutdown drain.** *(Fixed 2026-07-09.)* `axum::serve`
  now uses `.with_graceful_shutdown` on SIGINT/SIGTERM — in-flight requests
  drain (a mid-flight posting commits) before exit. Verified: SIGTERM logs the
  drain and exits cleanly.

---

## 6. UX / end-to-end failure modes

Things that **look complete** but **fail user journeys**. Cross-ref §1–§4 for
accounting/product roots of some UX failures.

### 6.1 P0 — Broken happy paths

- ✅ **Tracked goods product → post invoice** *(fixed 2026-07-09 — see §4.2:
  create AND edit now create/link the stock item).*  
- ✅ **Responsive shell.** *(Shipped 2026-07-10, PR #80/#91.)* Off-canvas
  sidebar drawer on mobile / fixed rail on desktop; `PageHeader` stacks
  (`flex-col … sm:flex-row`); all tab bars and wide rows use `overflow-x-auto` /
  `flex-wrap` — no page-level horizontal scroll at 390px.
- ✅ **Header search is decorative.** *(Fixed 2026-07-09.)* Now a working ⌘K
  command palette (`CommandPalette.tsx`): the header search and Cmd/Ctrl-K open
  it; type-to-filter over every app page (sourced from the sidebar nav),
  arrow-key + Enter to jump.  
- 🟡 **Silent mutation failures on critical actions.** *(Invoice/bill lists
  fixed 2026-07-09.)* Invoice post/delete and bill approve/post/delete list
  mutations now surface the server error (`onError` → alert), so credit-limit /
  stock / closed-period failures are visible. Other list pages still to sweep.  
- ✅ **RBAC UI vs API drift.** *(Largely closed 2026-07-10, PR #83; nav gated
  2026-07-09.)* Per-page ACTION buttons (Post/Send/Approve/Convert/Reverse/…)
  across invoices, bills, estimates, PRs/POs/tenders, expense claims, payroll,
  journals, products now gate on `usePermissions().can('<resource>.<action>')`
  instead of coarse `hasRole`; the sidebar hides destinations the user can't
  read; backend still enforces. Residual: a full sweep of any remaining
  `hasRole` call-sites, but the credit-limit/Bookkeeper mis-gating is resolved.

### 6.2 P1 — Misleading success / friction

- ✅ **Email/SMS/WhatsApp send without delivery config.** *(Shipped 2026-07-10,
  PR #84.)* The invoice/estimate send flow shows a provider pre-flight banner
  (`ProviderPreflight`) that warns when the channel's provider is unconfigured
  and links to Notification providers, instead of a silent neutral "sent".
- ⬜ **Nav overload (~50 items, 10 sections)** always shows Procurement/POS/
  Payroll/CRM for pure service SMEs (`Sidebar.tsx`).  
- ⬜ **Invoice/bill status tab counts are page-local**, not server totals
  (`InvoicesPage.tsx` filters `invoices` page slice).  
- ⬜ **Overdue tab depends on hourly scheduler** (`scheduler.rs` marks
  `overdue`); fresh past-due stay `posted` until tick.  
- ✅ **No “Pay” on bill/invoice list rows** *(Fixed 2026-07-09.)* Posted
  invoices/bills with a balance show a **Pay** button that opens Record Payment
  deep-linked to the party + document (`/payments?record=…&party=…&invoice|bill=…`).  
- ✅ **Apply unapplied payment requires pasting document UUID** *(Fixed
  2026-07-09.)* The allocate modal now shows a picker of the party's OPEN
  invoices/bills (number + balance), and defaults the amount to the document
  balance — no more UUID paste.  
- ⬜ **Credit note UI = full reverse only** (reason + `lines: []`)
  (`InvoiceDetailPage.tsx` CreditNoteModal) — no partial lines.  
- ✅ **Estimate convert does not open new invoice** *(Fixed 2026-07-09.)* The
  convert mutation now navigates to the created invoice (`?highlight=<id>`) and
  surfaces errors, instead of silently staying on the estimates list.  
- ✅ **Bill approve → post two-step** *(Fixed 2026-07-09.)* Draft bills gain an
  **Approve & Post** one-click (for users with both rights) alongside the
  separate Approve/Post buttons — the approval gate stays, but the common
  do-both path is one action.  
- ⬜ **Banking import vs Reconciliation vs Transactions** split across three
  nav items without guided flow.  
- ⬜ **Amos iframe blank if service down** (dev `localhost:8090`; prod
  `/amos-app`) — `AmosPage.tsx`.  
- ⬜ **Tax remittance UI requires raw liability/bank GL codes**
  (`TaxFilingsPage.tsx`) — weak labelling for non-accountants.  
- ✅ **Inconsistent feedback.** *(Shipped 2026-07-10, PR #81.)* A global
  `ToastProvider` (`useToast().success/error/info/fromError`) replaces
  `window.alert` across POS, products, FX, assets, banking, eTIMS,
  amortisation, etc.; list-mutation errors surface via `onError`.

### 6.3 P2 — Portal / secondary journeys

- ✅ Customer portal pay — *(Shipped 2026-07-10, PR #85.)* the public
  `/pay/:token` invoice page pays via Paystack. (In-portal linked-account pay
  and unlinked empty-state polish remain.)
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

- ✅ **P0 — Tool execution as service account bypasses human ERP RBAC.**
  *(Implemented 2026-07-10 — user-scoped MCP auth, cross-repo amos + mcp-erp.)*
  Amos now threads the session user's verified access token into every ERP tool
  call as a stripped `__user_token` arg (injected in `ScopedTool` AFTER the
  model turn + the confirm preview, so the model never sees it and it is never
  echoed); mcp-erp's manual `ServerHandler::call_tool` pulls it out before the
  typed input deserializes and binds it to a **task-local** (concurrency-safe
  across the one shared mcp-erp process), and `ZavoraBackend::request` uses it
  as the bearer for that call — **falling back to the service login on a 401**
  (expired token) or when absent (ambient routines, which run as the service
  account by design). **The ERP now records the human as the actor.** Verified
  live: a `create_customer` with a user token landed in that user's entity
  (`234537c2…`) while the same call without a token landed in the service
  account's entity (`d3aa0afa…`) — proving the per-call bearer. Unit tests pin
  extraction/stripping (mcp-erp) and injection (amos ERP-only, browser/routine
  skipped).
  **Remaining to fully close:** (1) **coordinated release** of amos + mcp-erp
  (`amos/Dockerfile` pins `MCP_ERP_REF`); (2) the embedded ERP shell should push
  refreshed access tokens over the existing `context` WS frame (server side
  already accepts + verifies them) so long sessions keep acting as the user
  rather than degrading to the service account after ~15 min.

### 7.2 P1 — Roles, persona, multi-tenant product, plans

- 🟡 **P1 — Coarse role → scope map vs RBAC v2.** *(Improved 2026-07-10,
  PR #87: Editor now gets `erp:write` — was mis-mapped read-only; **Customer**
  portal role now blocked like Vendor/Employee.)* `Principal::scopes`
  (`auth.rs`) still maps by role tier (Owner/Admin/Accountant → write+post;
  Approver/Editor → write; else read-only), not the user's *granular*
  permissions. Residual: derive scopes from `GET /auth/permissions` / JWT
  permission claims so bespoke roles (e.g. "Bookkeeper") are scoped exactly.

- ✅ **P1 — Hardcoded company persona.** *(Shipped 2026-07-10, PR #82.)*
  `build_runner` pulls the real tenant facts from `/settings` and
  `persona::company_context` injects `{company_name}` + `{company_context}`
  (name, base currency, VAT-registration flag, fiscal year) into the system
  instruction, with neutral fallbacks on error — Amos no longer claims to be a
  hardcoded company.

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

1. ✅ Complete `required_scopes` + regression test (P0) — done 2026-07-09.  
2. ✅ Hard confirm gate for ledger writes in live sessions (P0) — done 2026-07-09.  
3. ✅ User-scoped MCP auth / audit actor = human (P0/P1) — implemented
   2026-07-10 (amos + mcp-erp); pending coordinated release + client token
   refresh over the `context` frame.
4. 🟡 Scopes from permissions + block Customer; fix Editor/custom (P1) —
   Customer blocked + Editor fixed (#87); permissions-derived scopes still open.  
5. ✅ Dynamic `{company_context}` from settings (P1) — done 2026-07-10 (#82).  
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
- ✅ **P1 — Backups & migration safety.** *(Shipped 2026-07-10, PR #93.)*
  `docs/BACKUP_RUNBOOK.md`: what to back up (the `zavora_era` DB incl.
  `amos_sessions`/`amos_runs`/`amos_audit_events` + pgvector `memory_entries`),
  `pg_dump -Fc`/`pg_restore` procedures (manual/prod/scheduled + off-site +
  RPO/RTO), a verify-your-backup drill, and a destructive-migration review
  checklist. Round-trip verified live (119/119 tables, pgvector present).  
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

11. ✅ CORS + M-Pesa callback auth + rate limits on auth/webhooks (2026-07-09).  
12. ✅ PR CI (test/clippy/tsc + amos tests) before deploy (2026-07-09).  
13. ✅ Backup/restore runbook + drill (2026-07-10, #93).  
14. ✅ Payroll insurance relief field + nearest-shilling policy (2026-07-09; tax-pro review still advised).  
15. ✅ Permission-aware nav + `can()` on action buttons (2026-07-09/-10, #83).  
16. ✅ Global mutation error toast (#81); honest "email not configured" (#84) — 2026-07-10.  
17. ✅ Real search — ⌘K command palette (2026-07-09).  
18. ✅ Responsive shell (2026-07-10, #80/#91).  
19. ✅ AR/AP control recon report; VAT return all VAT accounts; FX reval scope (2026-07-09).  
20. **Amos:** 🟡 user-scoped MCP auth (**still open, the P0**); permission-based
    scopes (partial #87); ✅ dynamic company context (#82); refresh
    `docs/AMOS.md` (open) (§7.2).

### Wave C — Product honesty & depth (P2)

21. ~~Flutterwave: implement or strip from Specs/pricing.~~ ✅ Done — shipped as
    **Paystack** card payments + subscription billing (PRs #72/#73/#74).
22. ✅ Public invoice view/pay (Paystack) + `viewed_at` (2026-07-10, #85/#86).  
23. 🟡 Pay shortcuts + unapplied document picker + partial credit notes — pay
    shortcuts + picker done 2026-07-09; partial credit notes still open.  
24. 🟡 FIFO/warehouse — **multi-warehouse + 3PL shipped** (2026-07-10, #92);
    per-warehouse costing/FIFO still open.  
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
