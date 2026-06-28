# Posting Groups (GL Account Determination by Dimension)

> Status: **production-wired**. Control accounts (A/R, A/P), revenue/COGS/expense,
> and VAT output/input are all resolved through posting groups across invoicing,
> billing, credit notes, supplier credit notes, payments, and FX — with a safe
> fallback to the flat [`PostingSetup`](./POSTING_SETUP.md) so no existing tenant
> can mis-post.

This document is the full reference for Zavora's posting-group model: the data
model, the resolution rules, every posting site that uses them, the seeding and
backfill behaviour, the HTTP/UI surface, worked examples, and an operator runbook.

---

## 1. Why posting groups exist

Without posting groups, every invoice/bill line and every control posting has to
name a GL account explicitly, or fall back to a single hardcoded account for the
whole tenant. That breaks down the moment a business needs:

- **Different revenue/COGS accounts** for goods vs services, or domestic vs export.
- **Separate A/R or A/P control accounts** per customer/vendor segment (e.g. keep
  domestic debtors and export debtors on different balance-sheet lines).
- **Different VAT output/input accounts** per tax treatment or trading relationship.

Posting groups solve this the way Microsoft Dynamics 365 Business Central and
NetSuite do: you tag master data (customers, vendors, products) with *groups*, and
a small set of **matrices** map group combinations to GL accounts. Postings then
*derive* their accounts from the groups instead of carrying hardcoded codes.

The design rule, end to end:

> **Override → Matrix/Group → Flat default.** An explicit account on a line always
> wins. Otherwise the relevant matrix/group account is used. If neither is
> configured, the flat `PostingSetup` account is used. Turning groups on can never
> break a tenant that was working before.

---

## 2. The Business Central model, mapped to Zavora

| BC concept | What it drives | Zavora implementation |
|---|---|---|
| **General Business Posting Group** (who you trade with) | Revenue / COGS / expense by relationship, plus A/R & A/P control | `general_business_groups` (+ `receivables_account`, `payables_account`) |
| **General Product Posting Group** (what you trade) | Revenue / COGS / expense by item class | `general_product_groups` |
| **General Posting Setup** (Bus × Prod matrix) | The actual income/COGS/purchase accounts | `general_posting_matrix` |
| **VAT Business Posting Group** | Output/input VAT account + rate by relationship | `vat_business_groups` |
| **VAT Product Posting Group** | Output/input VAT account + rate by item | `vat_product_groups` |
| **VAT Posting Setup** (VAT Bus × VAT Prod matrix) | Rate + output/input VAT accounts | `vat_posting_matrix` |
| **Customer / Vendor Posting Group** (specific → A/R, A/P) | The receivables / payables control account | `receivables_account` / `payables_account` columns on `general_business_groups` |
| **Bank Account Posting Group** | Bank GL account | Per bank-account record (`bank_accounts.gl_account_code`) — not a group abstraction yet |
| **Inventory Posting Group** | Inventory asset + adjustment | Per-product accounts today (see §11 Deferred) |
| **Fixed Asset Posting Group** | Acquisition / depreciation / disposal | Per-asset / per-class accounts today (see §11 Deferred) |

Zavora keeps **A/R and A/P on the general business group** rather than a separate
"customer/vendor posting group" table. A customer's general business group
therefore answers two questions at once: which revenue/COGS matrix row applies,
and which receivables control account to debit. This is a deliberate
simplification — one dimension ("who you deal with") instead of two — that covers
the same use cases with less master-data overhead.

---

## 3. Coverage matrix

What is wired into real postings today:

| Area | Account(s) routed by group | Status |
|---|---|---|
| Invoice — revenue per line | `general_posting_matrix.sales_account` (customer biz × product general) | ✅ wired |
| Invoice — A/R | `general_business_groups.receivables_account` (customer biz) | ✅ wired |
| Invoice — VAT output per line | `vat_posting_matrix.vat_output_account` (customer VAT biz × product VAT) | ✅ wired |
| Credit note — A/R reversal | receivables control (customer biz) | ✅ wired |
| Credit note — VAT output reversal | VAT output (customer VAT biz × product VAT) | ✅ wired |
| Bad-debt write-off — A/R | receivables control (customer biz) | ✅ wired |
| Bill — expense/purchase per line | per-line account (matrix-derived via `resolve_bill_line`) | ✅ wired |
| Bill — A/P | `general_business_groups.payables_account` (vendor biz) | ✅ wired |
| Bill — VAT input | `vat_posting_matrix.vat_input_account` (vendor VAT biz) | ✅ wired |
| Supplier credit note — A/P reversal | payables control (vendor biz) | ✅ wired |
| Supplier credit note — VAT input reversal | VAT input (vendor VAT biz × product VAT) | ✅ wired |
| Payment — A/R / A/P settled | control account by party (customer/vendor biz) | ✅ wired |
| Payment — unapplied credit offset | control account by party | ✅ wired |
| FX gain/loss on settlement — A/R / A/P leg | control account by party | ✅ wired |

Deferred (functional today via per-record accounts; see §11):

- COGS → inventory group wiring (COGS line currently uses the product/default).
- Dedicated **Sales Credit Memo** account (credit notes reverse to the same
  revenue account as the original line).
- Fixed-asset posting groups (acquisition / depreciation / disposal).
- Inventory and Bank posting-group abstraction.
- A standalone VAT-return report that aggregates *all* output/input VAT accounts
  (the return currently reads the flat `vat_output` / `vat_input`).

---

## 4. Data model

### 4.1 Group + matrix tables

Created in `migrations/008_production_readiness.sql`; A/R & A/P control columns
added in `migrations/030_business_group_control_accounts.sql`.

```
vat_business_groups (id, entity_id, code, name, description, UNIQUE(entity_id,code))
vat_product_groups  (id, entity_id, code, name, description, UNIQUE(entity_id,code))
vat_posting_matrix  (id, entity_id,
                     vat_biz_group_id  → vat_business_groups,
                     vat_prod_group_id → vat_product_groups,
                     vat_rate NUMERIC(5,2),
                     vat_output_account TEXT,
                     vat_input_account  TEXT,
                     UNIQUE(entity_id, vat_biz_group_id, vat_prod_group_id))

general_business_groups (id, entity_id, code, name,
                         receivables_account TEXT NULL,   -- migration 030
                         payables_account    TEXT NULL,   -- migration 030
                         UNIQUE(entity_id, code))
general_product_groups  (id, entity_id, code, name, UNIQUE(entity_id,code))
general_posting_matrix  (id, entity_id,
                         gen_biz_group_id  → general_business_groups,
                         gen_prod_group_id → general_product_groups,
                         sales_account    TEXT,
                         purchase_account TEXT,
                         cogs_account     TEXT NULL,
                         UNIQUE(entity_id, gen_biz_group_id, gen_prod_group_id))
```

All matrix lookups are entity-scoped and backed by composite indexes:
`idx_vat_posting_matrix_lookup (entity_id, vat_biz_group_id, vat_prod_group_id)`
and `idx_general_posting_matrix_lookup (entity_id, gen_biz_group_id, gen_prod_group_id)`.

### 4.2 Master-data assignment columns

Added to master tables so each record carries its group tags
(`migrations/008_production_readiness.sql`):

```
customers : vat_business_group_id, general_business_group_id
vendors   : vat_business_group_id, general_business_group_id
products  : vat_product_group_id,  general_product_group_id
```

These are nullable. A NULL group simply means "no matrix dimension" → the resolver
returns `None` → the caller falls back to the flat default.

### 4.3 Migration 030 (the A/R & A/P control columns)

```sql
ALTER TABLE general_business_groups
    ADD COLUMN IF NOT EXISTS receivables_account TEXT;
ALTER TABLE general_business_groups
    ADD COLUMN IF NOT EXISTS payables_account TEXT;
```

`NULL` (or empty) on these columns means "fall back to the per-record account,
then the flat `PostingSetup`." Migrations auto-apply on API startup
(`sqlx::migrate!("../migrations")`).

---

## 5. Resolution & precedence

The whole model is a **fallback chain**. For any account, resolution proceeds:

```
1. Explicit account on the line/request   (user override)        ── wins
2. Group/matrix account                    (derived dimension)
3. Flat PostingSetup default               (per-tenant baseline)  ── always exists
```

### 5.1 Revenue (sales) — example of all three layers

In `resolve_invoice_line` (`zavora-erp-core/src/services/invoicing.rs`):

```rust
account_code: req.account_code        // 1. explicit override
    .or(derived_sales)                // 2. general matrix (customer biz × product group)
    .unwrap_or(product.sales_account) // 3. product's own account (≈ flat default)
```

`derived_sales` is only computed when `req.account_code` is `None`, and is itself
`None` unless both the customer business group and the product general group are
set *and* a matching `general_posting_matrix` row exists.

### 5.2 Control accounts (A/R, A/P) and VAT — the resolver helpers

Each posting site calls a resolver that returns `Option<String>` and supplies its
own flat fallback via `unwrap_or_else`:

```rust
let ar_account = posting::groups::resolve_receivables(engine, entity_id, customer_id)
    .await
    .unwrap_or_else(|| posting.accounts_receivable.clone()); // flat fallback
```

`resolve_receivables` returns `Some` only when the customer has a general business
group **and** that group's `receivables_account` is set and non-empty. Otherwise
`None`, and the flat `accounts_receivable` is used.

### 5.3 The single most important invariant

> The A/R (or A/P) account **credited when a payment is applied** must equal the
> A/R (or A/P) account **debited when the invoice/bill was posted**.

If the invoice posts to a group-specific receivables account but the payment
clears the flat one, the customer subledger never nets to zero and the trial
balance carries phantom balances. This is why payment settlement, unapplied-credit
offset, and FX gain/loss all route A/R and A/P by the **same** party group as the
originating document (see §7.4). Because both sides resolve through
`resolve_receivables`/`resolve_payables` keyed on the same party, they are
guaranteed consistent.

---

## 6. Resolver API (`zavora-erp-core/src/posting/groups.rs`)

### 6.1 Matrix lookups (return the full row)

| Function | Returns | Notes |
|---|---|---|
| `resolve_general(engine, entity, biz: Option<Uuid>, prod: Option<Uuid>)` | `ErpResult<Option<GeneralPosting>>` | `None` if either group unset or no matrix row |
| `resolve_vat(engine, entity, biz: Option<Uuid>, prod: Option<Uuid>)` | `ErpResult<Option<VatPosting>>` | `VatPosting` carries `vat_rate`, `vat_output_account`, `vat_input_account` |

```rust
pub struct GeneralPosting { sales_account, purchase_account, cogs_account }
pub struct VatPosting     { vat_rate, vat_output_account, vat_input_account }
```

### 6.2 Master → group id

| Function | Reads |
|---|---|
| `customer_general_biz(engine, entity, customer_id)` | `customers.general_business_group_id` |
| `vendor_general_biz(engine, entity, vendor_id)` | `vendors.general_business_group_id` |
| `product_general_group(engine, entity, product_id)` | `products.general_product_group_id` |
| `customer_vat_biz(engine, entity, customer_id)` | `customers.vat_business_group_id` |
| `vendor_vat_biz(engine, entity, vendor_id)` | `vendors.vat_business_group_id` |
| `product_vat_group(engine, entity, product_id)` | `products.vat_product_group_id` |

All return `Option<Uuid>`; a missing master or NULL column yields `None`.

### 6.3 Account resolvers used by postings (return `Option<String>`)

| Function | Account | Keyed on |
|---|---|---|
| `resolve_receivables(engine, entity, customer_id)` | A/R control | customer's general business group |
| `resolve_payables(engine, entity, vendor_id)` | A/P control | vendor's general business group |
| `resolve_vat_output(engine, entity, customer_id, product_id: Option<Uuid>)` | Output VAT | customer VAT biz × product VAT group |
| `resolve_vat_input(engine, entity, vendor_id, product_id: Option<Uuid>)` | Input VAT | vendor VAT biz × product VAT group |

Each returns `Some(code)` only when the resolved account is present and non-empty;
empty strings are filtered out so a blank cell falls through to the flat default.
When `product_id` is `None` (e.g. an aggregate VAT line on a bill), VAT resolves on
the party's VAT business group alone.

### 6.4 Seeding / lifecycle

| Function | Purpose |
|---|---|
| `ensure_default_posting_groups(engine, entity)` | Idempotent: seed default groups+matrices if none, always backfill unassigned masters, always backfill DOMESTIC A/R & A/P. Safe to call on startup, after signup, and at the top of `create_invoice`/`create_bill`. |
| `seed_groups` (private) | First-time seed from the flat `PostingSetup` (see §8). |
| `assign_default_groups` (private) | Assign DOMESTIC/STD/GOODS/SERVICES/STD16 to any master with NULL groups. |

---

## 7. Posting sites — exactly where accounts route

Every site below resolves the account through a group helper and falls back to the
flat `PostingSetup`. DR/CR shown for the control/tax legs only.

### 7.1 Sales invoice — `services/invoicing.rs` (`post_invoice`)

| Leg | Account source |
|---|---|
| **DR A/R** (gross) | `resolve_receivables(customer_id)` → flat `accounts_receivable` |
| CR Revenue (per line) | line `account_code` (matrix-derived in `resolve_invoice_line`) |
| **CR VAT Output** (per line) | `resolve_vat_output(customer_id, line.product_id)` → flat `vat_output` |

### 7.2 Sales credit note — `services/invoicing.rs`

| Leg | Account source |
|---|---|
| **CR A/R** (reduce) | `resolve_receivables(original.customer_id)` → flat |
| DR Revenue (per line) | line `account_code` |
| **DR VAT Output** (per line) | `resolve_vat_output(original.customer_id, line.product_id)` → flat |

### 7.3 Bad-debt write-off — `services/invoicing.rs`

| Leg | Account source |
|---|---|
| DR Bad-debt expense | caller-supplied `expense_account` |
| **CR A/R** | `resolve_receivables(invoice.customer_id)` → flat |

### 7.4 Bill — `routes/bills.rs` (`post_bill` handler)

| Leg | Account source |
|---|---|
| DR Expense/Purchase (per line) | bill line `account_code` (matrix-derived in `resolve_bill_line`) |
| **DR VAT Input** (aggregate) | `resolve_vat_input(vendor_id, None)` → flat `vat_input` |
| **CR A/P** (net of WHT) | `resolve_payables(vendor_id)` → flat `accounts_payable` |
| CR WHT Payable | flat `wht_payable` |

### 7.5 Supplier credit note — `services/supplier_credit_notes.rs`

| Leg | Account source |
|---|---|
| **DR A/P** (reduce) | `resolve_payables(req.vendor_id)` → flat |
| CR Expense (per line) | line `account_code` |
| **CR VAT Input** (per line) | `resolve_vat_input(req.vendor_id, line.product_id)` → flat |

### 7.6 Payments & FX — `services/payments.rs`

`PaymentAccounts::resolve(engine, entity_id, party_id: Option<Uuid>, payment_type)`
routes the control account by party:

- **Customer payment** → A/R from `resolve_receivables(party_id)`, else flat.
- **Vendor payment** → A/P from `resolve_payables(party_id)`, else flat.

Used by all three posting paths so they stay consistent with the source document:

| Function | Uses party | Legs affected |
|---|---|---|
| `post_payment_journal_entry(... party_id ...)` | `req.party_id` | DR/CR A/R or A/P, unapplied excess |
| `apply_unapplied_payment` | `row.party_id` | DR Unapplied / CR A/R or A/P |
| `post_fx_gain_loss_entry(... party_id ...)` | `req.party_id` | the A/R/A/P offsetting leg of realised FX gain/loss |

---

## 8. Seeding & backfill behaviour

`ensure_default_posting_groups` is the single entry point and is **idempotent**:

1. **First-time seed** (`seed_groups`, only when the tenant has zero general
   business groups) creates, from the flat `PostingSetup`:
   - General business group **DOMESTIC** with `receivables_account = ps.accounts_receivable`
     and `payables_account = ps.accounts_payable`.
   - General product groups **GOODS**, **SERVICES**.
   - `general_posting_matrix` rows for DOMESTIC×{GOODS,SERVICES} →
     `sales = ps.default_sales`, `purchase = cogs = ps.default_purchase`.
   - VAT business group **STD**; VAT product groups **STD16 / ZERO / EXEMPT**.
   - `vat_posting_matrix` rows: STD×STD16 @ 16%, STD×ZERO @ 0%, STD×EXEMPT @ 0%,
     all → `ps.vat_output` / `ps.vat_input`.

2. **Always: assign defaults** (`assign_default_groups`) — any customer/vendor with
   NULL general/VAT business group gets DOMESTIC/STD; any product gets GOODS or
   SERVICES (by `product_type`) and STD16. Only touches NULL rows, so manual
   assignments are never overwritten. This covers masters created *after* the seed.

3. **Always: backfill control accounts** — sets the DOMESTIC group's
   `receivables_account` / `payables_account` from the flat setup **only where they
   are NULL** (`COALESCE`). This is what upgrades tenants that were seeded before
   migration 030 added the columns. A tenant that customised these via the UI is
   never clobbered.

Net effect: a brand-new tenant and a long-lived pre-030 tenant both converge to a
fully-populated, override-safe configuration with no manual step.

---

## 9. HTTP API

All under `/api/v1/posting-groups`, defined in
`zavora-erp-api/src/routes/posting_groups.rs`, registered in `main.rs`. Mutations
require a create-capable role (`ROLES_CREATE`).

### `GET /posting-groups`
Returns the full configuration (and calls `ensure_default_posting_groups` first, so
the editor is never empty):

```jsonc
{
  "vat_business":   [{ "id","code","name" }],
  "vat_product":    [{ "id","code","name" }],
  "vat_matrix":     [{ "id","vat_biz_group_id","vat_prod_group_id","vat_rate","vat_output_account","vat_input_account" }],
  "general_business":[{ "id","code","name","receivables_account","payables_account" }],
  "general_product":[{ "id","code","name" }],
  "general_matrix": [{ "id","gen_biz_group_id","gen_prod_group_id","sales_account","purchase_account","cogs_account" }]
}
```

### `POST /posting-groups/group`
Create a group. `kind` ∈ `vat_business | vat_product | general_business | general_product`.
```json
{ "kind": "general_business", "code": "EXPORT", "name": "Export customers" }
```

### `POST /posting-groups/assign`
Tag a master record with its groups. `kind` ∈ `customer | vendor | product`.
```json
{ "kind": "customer", "id": "<uuid>", "general_group_id": "<uuid>", "vat_group_id": "<uuid>" }
```
(Maps to `general_business_group_id`/`vat_business_group_id` for parties,
`general_product_group_id`/`vat_product_group_id` for products.)

### `POST /posting-groups/business-control`  *(new in this work)*
Set the A/R and A/P control accounts on a general business group. Empty string
clears the account (→ fall back to flat).
```json
{ "gen_biz_group_id": "<uuid>", "receivables_account": "1210", "payables_account": "3015" }
```

### `POST /posting-groups/general-matrix`
Upsert one matrix cell (delete-then-insert by the unique key).
```json
{ "gen_biz_group_id":"<uuid>", "gen_prod_group_id":"<uuid>",
  "sales_account":"4500", "purchase_account":"6000", "cogs_account":"6000" }
```

### `POST /posting-groups/vat-matrix`
```json
{ "vat_biz_group_id":"<uuid>", "vat_prod_group_id":"<uuid>",
  "vat_rate":0.16, "vat_output_account":"3100", "vat_input_account":"1300" }
```

---

## 10. UI

### Settings → Posting Groups (`zavora-erp-ui/src/pages/settings/PostingGroupsTab.tsx`)
Three sections, all inline-save (dropdowns populated from the chart of accounts):

1. **Control Accounts** — A/R & A/P per business group (calls `business-control`).
   *new in this work.*
2. **General Posting Matrix** — sales / purchase / COGS per business × product cell.
3. **VAT Posting Matrix** — rate / output VAT / input VAT per VAT business × product
   cell. "Add group" controls create new business/product groups in place.

### Master-data assignment (`components/shared/PostingGroupFields.tsx`)
A reusable selector embedded in the Customer, Vendor, and Product create modals:
- `scope="party"` shows general + VAT **business** groups (customers/vendors).
- `scope="product"` shows general + VAT **product** groups.
On save the page calls `assignPostingGroups(...)` so the new record is tagged
immediately. Leaving the selectors blank lets `assign_default_groups` apply the
DOMESTIC/STD defaults.

### API client (`zavora-erp-ui/src/api/client.ts`)
`getPostingGroups`, `createPostingGroup`, `assignPostingGroups`,
`upsertGeneralMatrix`, `upsertVatMatrix`, `upsertBusinessControl`.

---

## 11. Worked example — domestic vs export A/R

Goal: domestic debtors on `1200`, export debtors on `1210`; export sales to `4500`.

1. **Create an EXPORT business group** — Settings → Posting Groups → *Add business
   group* → code `EXPORT`. (`POST /group`.)
2. **Set its control accounts** — Control Accounts row EXPORT → Receivables `1210`,
   Payables as needed. (`POST /business-control`.)
3. **Set its revenue cell(s)** — General Matrix row EXPORT × GOODS → Sales `4500`.
   (`POST /general-matrix`.)
4. **Tag the customer** — on the customer, choose business group EXPORT.
   (`POST /assign`.)
5. **Post an invoice** for that customer:
   - DR **1210** (export A/R, from the group) — not the flat 1200.
   - CR **4500** (export revenue, from the matrix).
   - CR VAT output from the VAT matrix.
6. **Receive payment** → DR Bank / CR **1210** — same account as the invoice debit,
   so the export-debtors balance nets to zero correctly.

A domestic customer (group DOMESTIC, or untagged → defaulted to DOMESTIC) keeps
posting to 1200/`default_sales` exactly as before.

---

## 12. Operator runbook

- **New tenant**: nothing to do — signup seeds DOMESTIC/STD groups, the standard
  matrices, and assigns every master a sensible default.
- **Existing tenant upgrading past migration 030**: nothing to do — the DOMESTIC
  group's A/R/A/P columns auto-backfill from the flat setup on the next
  `ensure_default_posting_groups` call (opening the editor, or creating an
  invoice/bill). The API must be running the post-030 binary.
- **Add a segment** (export, intercompany, etc.): create a business group, set its
  control accounts and matrix cells, then tag the relevant masters. See §11.
- **Change a mapping**: edit the cell/control account in the UI; it takes effect on
  the *next* posting. Historical journal entries are immutable and are **not**
  rewritten.
- **Undo a customisation**: blank a matrix cell or control account → that account
  falls back to the flat default again.

> Changing a posting-group account does not restate posted journals. If a control
> account had balances under the old mapping, move them with a manual journal —
> the group only governs *future* postings.

---

## 13. Invariants & edge cases

- **Subledger consistency** (§5.3): document and its settlement always resolve the
  control account through the same party group → they match by construction.
- **Empty ≠ set**: empty-string accounts are filtered to `None` so a half-filled
  cell never posts to `""`.
- **Negative/contra lines**: revenue contra lines are booked as positive debits
  (no negative journal amounts); group resolution is unaffected.
- **Aggregate bill VAT**: bills post one VAT-input line, resolved on the vendor's
  VAT business group with `product_id = None` (no per-line product dimension).
- **Immutability**: the ledger is append-only; re-mapping never edits history.
- **Multi-tenant isolation**: every query is `entity_id`-scoped and the unique keys
  include `entity_id`, so groups never leak across tenants.

---

## 14. Verification

- `cargo build` (workspace) — clean.
- `cargo test -p zavora-erp-core --lib` — 65/65 pass.
- `tsc --noEmit` (UI) — clean.
- End-to-end (matrix path, verified in development): invoice with no account →
  matrix account; explicit account → override; customer reassigned to a new EXPORT
  group → routes to the EXPORT sales/AR accounts.

---

## 15. File reference

| Concern | Path |
|---|---|
| Flat baseline accounts | `zavora-erp-core/src/posting/mod.rs` (`PostingSetup`) |
| Resolvers + seeding | `zavora-erp-core/src/posting/groups.rs` |
| Invoice/credit-note/write-off postings | `zavora-erp-core/src/services/invoicing.rs` |
| Bill posting | `zavora-erp-api/src/routes/bills.rs` |
| Supplier credit note | `zavora-erp-core/src/services/supplier_credit_notes.rs` |
| Payments & FX | `zavora-erp-core/src/services/payments.rs` |
| HTTP routes | `zavora-erp-api/src/routes/posting_groups.rs`, `main.rs` |
| Settings editor | `zavora-erp-ui/src/pages/settings/PostingGroupsTab.tsx` |
| Master-data selector | `zavora-erp-ui/src/components/shared/PostingGroupFields.tsx` |
| API client | `zavora-erp-ui/src/api/client.ts` |
| Group/matrix tables | `migrations/008_production_readiness.sql` |
| A/R & A/P control columns | `migrations/030_business_group_control_accounts.sql` |
| Flat setup doc | `docs/POSTING_SETUP.md` |

---

## 16. Future work (deferred)

1. **COGS → inventory posting group** — route the COGS leg of inventory issues
   through the matrix `cogs_account` / an inventory group, not the product default.
2. **Sales Credit Memo account** — a dedicated contra-revenue account for credit
   notes instead of reversing into the original revenue account.
3. **Fixed-asset posting groups** — acquisition / accumulated depreciation /
   depreciation expense / disposal gain-loss per asset class.
4. **Inventory & Bank posting-group abstraction** — replace per-record accounts
   with group-driven resolution to match the BC model fully.
5. **VAT-return aggregation** — sum across all output/input VAT accounts defined in
   the matrix rather than reading the flat `vat_output` / `vat_input`.
6. **Per-line bill VAT** — resolve input VAT per bill line (product VAT group)
   instead of one aggregate line.
