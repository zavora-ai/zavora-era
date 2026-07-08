# RBAC v2 — Granular, Audit-Grade Authorization

Status: **Implemented & enforced.** The granular catalog (181 permissions),
rule-based SoD seed, declarative route→permission registry (295 routes),
central default-deny middleware, coverage test and per-role verification are all
live. The legacy coarse `require_role` groups and their ~233 call sites have been
removed — the declarative registry is the single, auditable authorization gate.
Supersedes the coarse 11-permission model in
`RBAC_AND_USER_MANAGEMENT.md`. Goal: an access-control model that passes a
rigorous ERP security audit — **resource × action** permissions (full CRUD +
workflow verbs), **declarative** route→permission mapping, **central
default-deny** enforcement, **segregation-of-duties (SoD)** aware, and fully
**auditable**.

## 1. Why the coarse model isn't enough

`records.manage` today authorizes creating an invoice, a bill, a product, a pay
run and a CRM deal — all with one grant. An auditor asks: *"Show me exactly who
can post a journal, void an invoice, approve a bill, or delete a customer, and
prove no one else can."* The coarse model can't answer that. We need a
permission per **(resource, action)** and a single declarative matrix mapping
every endpoint to the permission it requires.

## 2. Model

### 2.1 Permission key = `resource.action`
Lowercase, dotted, stable. Examples: `invoice.post`, `bill.approve`,
`customer.delete`, `journal.reverse`, `pay_run.pay`, `role.update`.

### 2.2 Action verbs (canonical)
| Verb | Meaning |
|---|---|
| `read` | view / list / export-preview of the resource |
| `create` | create a new record (draft) |
| `update` | edit an existing record |
| `delete` | delete/remove a record |
| `post` | commit to the ledger (irreversible; JE, invoice, bill, pay run) |
| `approve` | authorize (bill, pay run, requisition, expense, leave, tender award) |
| `send` | transmit externally (email invoice/statement, PO, eTIMS) |
| `void` | void/write-off/credit a posted document |
| `reverse` | book a reversing entry |
| `close` | close/reopen a fiscal period |
| `run` | execute a process (payroll, depreciation, recurring, reconciliation, FX reval) |
| `pay` | mark paid / disburse |
| `config` | manage the module's master data / settings |
| `manage` | umbrella for admin resources with no finer split (user, role, settings) |

Not every resource supports every verb; each resource declares its applicable
set (see §3). `read` is always present.

### 2.3 SoD (segregation of duties)
The verbs are split precisely so duties can be separated:
- **create ≠ approve ≠ post ≠ pay** — the classic four-eyes chain. A role can
  create bills but not approve them; approve but not post; post but not pay.
- **config ≠ transactional** — editing statutory rates / posting setup / COA is a
  separate grant from day-to-day posting.
- **read splits** — sensitive reads (`pay_run.read`, `employee.read`,
  `audit.read`) are distinct grants, not bundled into a generic read.
The workflow engine still enforces state transitions; RBAC provides the grants
that make SoD *assignable and auditable*.

## 3. Resource taxonomy (catalog)

~40 resources grouped by module. `[verbs]` lists the actions each supports.
The catalog is **generated** from this table in code (`resource × verb`), so it
stays consistent and the UI matrix renders it grouped by category.

**Sales**
- `invoice` [read, create, update, delete, post, send, void, reverse]
- `credit_note` [read, create]
- `estimate` [read, create, update, delete, send, convert]
- `recurring_invoice` [read, create, update, delete]

**Receivables / Customers**
- `customer` [read, create, update, delete]
- `customer_statement` [read, send]

**Purchases**
- `bill` [read, create, update, delete, approve, post, void]
- `supplier_credit` [read, create]
- `debit_note` [read, create]
- `expense_claim` [read, create, submit, approve]

**Vendors**
- `vendor` [read, create, update, delete]

**Procurement**
- `requisition` [read, create, submit, approve, reject, convert]
- `tender` [read, create, publish, award]
- `purchase_order` [read, create, send, receive]
- `goods_receipt` [read, create]
- `vendor_application` [read, approve, reject]
- `approval_limit` [read, config]

**Banking**
- `payment` [read, create, apply, delete]
- `bank_account` [read, create, delete]
- `bank_transaction` [read, categorise, reconcile, import]
- `reconciliation` [read, run, complete]

**Products & Inventory**
- `product` [read, create, update, delete]
- `inventory` [read, adjust, receive, issue]

**Fixed Assets**
- `asset` [read, create, run]  (`run` = depreciation)

**Accounting**
- `journal` [read, post, reverse]
- `account` [read, create, update]  (chart of accounts)
- `recurring_journal` [read, create, delete, run]
- `period` [read, close]
- `opening_balance` [read, create]
- `dimension` [read, create]

**Tax & Compliance**
- `tax_filing` [read, create, remit]
- `wht_rate` [read, config]

**FX**
- `fx_rate` [read, create, delete, run]  (`run` = revaluation)

**Reports & Analysis**
- `report` [read, export]
- `budget` [read, config]
- `custom_report` [read, create, delete]
- `report_schedule` [read, create, delete]
- `consolidation` [read]

**Payroll & HR** (sensitive — excluded from the generic Viewer read)
- `employee` [read, create, update]
- `pay_run` [read, create, approve, post, pay, delete]  (`create` = run payroll)
- `payroll_config` [read, config]  (earning/deduction types, departments, statutory, recurring, loans)
- `leave` [read, approve]
- `leave_type` [read, config]
- `holiday` [read, config]
- `onboarding` [read, create, update]

**CRM** (optional module)
- `crm` [read, config]  (enable/disable)
- `lead` [read, create, update, convert]
- `opportunity` [read, create, update, close]  (`close` = win/lose)
- `activity` [read, create, update]
- `ticket` [read, create, update]

**Point of Sale**
- `pos_sale` [read, create]
- `pos_session` [read, run]  (open/close shift, Z-report)
- `pos_stock` [read, adjust]

**Administration**
- `user` [read, manage]  (invite/edit/deactivate/resend)
- `role` [read, create, update, delete]
- `settings` [read, config]
- `notification_provider` [read, config]
- `audit` [read]
- `portal_invite` [create]  (customer/vendor/employee portal invites)

This yields ≈ 170 permission keys. `notification` (own inbox) and portal
self-service are **self-scoped** (no permission needed — enforced by principal +
row ownership); documented in the enforcement allowlist.

## 4. System-role seed (rule-based, behaviour-aware)

Seed grants are computed by **rules** over the catalog, not hand-typed, so they
stay consistent as the catalog grows. Rules per system role:

- **Owner / Admin** → **every** permission in the catalog.
- **Viewer** → `*.read` and `report.read`/`report.export` for **non-sensitive**
  resources; explicitly **excluded**: `pay_run.*`, `employee.*`, `payroll_config.*`,
  `audit.read` (sensitive reads).
- **Accountant** → all reads (incl. `pay_run.read`, `employee.read`); `create`,
  `update`, `post`, `send`, `void`, `reverse` on financial resources (invoice,
  bill, journal, payment, credit/debit notes, tax, fx, assets); `period.close`;
  `pay_run.post`/`pay_run.pay` (ledger side); reports/export. **Not**: `approve`
  (SoD — approval is the Approver), `*.config` admin of masters, `user/role/settings`.
- **Approver** → all reads + every `approve`/`reject`/`award`/`publish` verb
  (bill, pay_run, requisition, expense_claim, tender, vendor_application, leave).
  **No** create/post — pure authorization role (clean SoD).
- **Editor** → reads + `create`/`update` on operational resources (invoice,
  estimate, recurring_invoice, customer, product, inventory, vendor, bill draft,
  CRM lead/opportunity/activity/ticket, requisition create). **No** post/approve/
  delete/void/config/admin.
- **HrManager** → full HR/payroll: `employee.*`, `pay_run.*` (run/approve/post/
  pay), `payroll_config.*`, `leave.*`, `leave_type.*`, `holiday.*`, `onboarding.*`,
  plus their reads. **No** finance/sales/GL/admin.

The rules are encoded once; a **golden test** asserts (a) every catalog key is
granted to Owner, (b) Viewer has no sensitive read, (c) SoD invariants hold
(e.g. Editor lacks every `*.post`/`*.approve`; Approver lacks every `*.create`),
and (d) the union reproduces or intentionally refines today's behaviour.

## 5. Enforcement — declarative + central default-deny

The auditable core: a **single declarative registry** mapping **every** route
(method + path pattern) to the permission it requires — the access-control matrix
an auditor can read top-to-bottom.

```rust
// One row per route. This IS the audit artifact.
routes! {
  (GET,    "/api/v1/invoices",            "invoice.read"),
  (POST,   "/api/v1/invoices",            "invoice.create"),
  (POST,   "/api/v1/invoices/{id}/post",  "invoice.post"),
  (POST,   "/api/v1/invoices/{id}/void",  "invoice.void"),
  (POST,   "/api/v1/bills/{id}/approve",  "bill.approve"),
  (POST,   "/api/v1/payroll/{id}/pay",    "pay_run.pay"),
  // … every route …
  (POST,   "/api/v1/payments/mpesa-callback", PUBLIC),      // explicit, reviewed
  (GET,    "/api/v1/notifications",           SELF),        // self-scoped
}
```

A `MatchedPath` middleware on the protected router looks up `(method, matched
path)` and calls `require_permission`. **Default-deny:** a route with no registry
entry is **rejected** (500/deny + loud log) — so nothing is silently open.
Per-handler `require_permission` calls remain as defense-in-depth during
migration, then the registry is the single gate.

**Coverage test v2**: builds the registry, and asserts (a) every mutating *and
read* route in `main.rs` has a registry entry, (b) every entry's permission is a
real catalog key (or `PUBLIC`/`SELF` with justification), (c) no orphan entries.
CI fails on any unmapped route or unknown permission — the mechanical guarantee
that the audit matrix is complete and truthful.

## 6. Audit linkage

- Every denied request is logged (actor, entity, route, required permission).
- Sensitive grants (`*.post`, `*.approve`, `*.pay`, `role.*`, `user.manage`,
  `settings.config`) emit an audit event on **use**, tying the existing audit
  trail to the permission that authorized the action.
- The registry (§5) is exported as the documented access-control matrix; role→
  permission grants are queryable (`GET /roles/{id}`) — auditors can diff
  assigned vs. required.

## 7. Migration plan (incremental, non-breaking)

1. **Catalog + seed (core).** Generate the granular catalog; seed system roles by
   the §4 rules; keep the old coarse keys temporarily so existing `require_role`/
   `require_permission("records.manage")` checks still pass. Golden tests.
2. **Registry + central layer.** Add the declarative registry + `MatchedPath`
   default-deny middleware, mapping every route to its granular permission. Wire
   it on the protected router (per-handler checks stay as belt-and-suspenders).
3. **Coverage test v2.** Assert full route coverage + valid permissions.
4. **Retire coarse keys.** Once the registry is the gate and per-handler checks
   are aligned, remove the coarse permissions and the legacy `require_role`
   const groups.
5. **UI.** The Roles matrix already renders any catalog grouped by category, so it
   scales automatically; add search + per-resource “grant all CRUD” affordances.
6. **Verify.** Per-role curl matrix across representative endpoints; workspace +
   coverage tests; Playwright roles matrix.

Each step is independently shippable; steps 1–2 preserve current behaviour
(system roles keep effective access) while adding granularity underneath.

## 8. Backwards compatibility

- `era_users.role` and the JWT claim are unchanged (role keys).
- Custom roles already work (Phase 3); they now pick from ~170 granular perms.
- The `Viewer` sensitive-read tightening (Phase 4A) is retained and formalised.
