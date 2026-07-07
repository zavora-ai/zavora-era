# RBAC & User Management — Complete Solution Design

Status: **Proposed** · Owner: platform · Scope: `zavora-erp-core`, `zavora-erp-api`,
`zavora-erp-ui` · Related: `middleware/auth.rs`, `rbac/mod.rs`, `routes/users.rs`,
`routes/auth_tenants.rs`, `utils/roles.ts`, `pages/settings/UsersPage.tsx`.

## 1. Problem

The current authorization model is **hard-coded and opt-in**, and user
administration is **incomplete**. Concretely (evidence in the codebase):

- **Hard-coded roles/permissions.** 7 roles are a compile-time enum
  (`rbac::UserRole`); permission→role mappings are `const` arrays in
  `middleware/auth.rs` (`ROLES_CREATE`, `ROLES_APPROVE`, …) and **duplicated**
  in the UI (`utils/roles.ts`). Any change needs a code edit + redeploy, and the
  two copies have already **drifted** (UI omits `HrManager`).
- **No role management.** No custom roles, no per-permission granularity, no
  per-user overrides. `roles`/`permissions` tables do not exist; `era_users.role`
  is unconstrained `TEXT` (a bad value silently locks a user out at login).
- **Opt-in authorization (default-allow).** The global middleware only checks
  *authentication*; each handler must remember to call `require_role`. A new
  mutating route that forgets it is open to any authenticated user (incl.
  `Viewer`). ~233 manual checks today — broad, but structurally fragile.
- **Incomplete user admin.** The UI can only *invite*; it cannot change a role,
  deactivate/reactivate, or remove a user (despite `PUT /users/{id}` existing).
  `HrManager` is unassignable from the UI. Invited internal users **cannot
  activate** — `era_users` has no `set_token` and there is no set-password /
  forgot-password endpoint for internal users (the staff/vendor/customer portals
  all have one).
- **Coarse read scope.** `Viewer` reads everything, including payroll salaries.

## 2. Goals / non-goals

**Goals**
1. Data-driven RBAC: roles and their permissions live in the DB and are editable
   at runtime, per tenant, without a deploy.
2. **Default-deny** on all state-changing endpoints, with a mechanical guarantee
   that new routes are covered.
3. Complete internal-user lifecycle: invite → activate (set password) → manage
   (edit role, deactivate, remove) → recover (forgot password).
4. A single source of truth for permissions shared by API and UI (no drift).
5. **Behaviour-preserving migration** — the existing 7 roles keep exactly their
   current effective permissions on day one.

**Non-goals**
- Per-record ACLs / row-level sharing (overkill for an SME ERP; revisit only on
  concrete demand).
- Replacing the multi-tenant membership model (email-based, one `era_users` row
  per entity, per-membership role) — it is sound and is retained.
- ABAC / attribute policies. We stay with role→permission RBAC.

## 3. Target model

### 3.1 Permission catalog (code-defined, DB-synced)

Permissions are **stable string keys** `module.action`. The catalog of *what
permissions exist* is defined once in Rust (versioned with code) and synced into
a `permissions` table on startup. *Which role has which permission* is data.

Initial catalog (grouped; extend without schema change):

| Key | Category | Guards (today's constant) |
|---|---|---|
| `sales.manage` | Sales | invoices/estimates/recurring create+edit (`ROLES_CREATE`) |
| `sales.send` | Sales | send invoice/statement/estimate (`ROLES_SEND`) |
| `purchases.manage` | Purchases | bills/credit-notes/expense create+edit (`ROLES_CREATE`) |
| `purchases.approve` | Purchases | approve bills (`ROLES_APPROVE`) |
| `procurement.manage` | Procurement | requisitions/tenders/PO/GRN (`ROLES_CREATE`) |
| `procurement.approve` | Procurement | approve requisition/tender award (`ROLES_APPROVE`) |
| `banking.manage` | Banking | payments/bank import/reconcile/transactions |
| `journal.post` | Accounting | post/reverse journals (`ROLES_POST_JOURNAL`) |
| `accounting.manage` | Accounting | COA, recurring journals, FX, dimensions, budgets |
| `period.manage` | Accounting | close/reopen/year-end (`ROLES_CLOSE_PERIOD`) |
| `tax.manage` | Compliance | tax filings, WHT rates |
| `inventory.manage` | Inventory | items, receive/issue/adjust |
| `assets.manage` | Assets | asset register, depreciation |
| `crm.manage` | CRM | CRM writes (enable, pipeline, leads, tickets) |
| `hr.manage` | HR | employees, leave config, onboarding (`ROLES_HR_MANAGE`) |
| `leave.approve` | HR | approve/decline leave (`ROLES_LEAVE_APPROVE`) |
| `payroll.manage` | HR | run/approve/post/pay payroll |
| `payroll.read` | HR | view payslips/salaries (**new read split**) |
| `hr.read` | HR | view employee master (**new read split**) |
| `reports.read` | Reports | run reports |
| `settings.manage` | Admin | entity settings, posting groups, notifications |
| `users.manage` | Admin | invite/edit/deactivate users (`ROLES_MANAGE`) |
| `roles.manage` | Admin | edit roles & permissions (**new**) |
| `read.all` | Global | general read of finance/sales/ops data |

> Notes: keys map ≥1:1 onto today's `const` arrays so seeding is
> behaviour-preserving. `payroll.read`/`hr.read` are introduced now but, in the
> behaviour-preserving seed, `Viewer` **keeps** them (see §7) — tightening is an
> explicit, opt-in Phase C change so we never silently break an existing tenant.

### 3.2 Schema (new migration `048_rbac.sql`)

```sql
-- Catalog of all known permissions (synced from code on startup; app-owned).
CREATE TABLE permissions (
    key         TEXT PRIMARY KEY,           -- 'journal.post'
    category    TEXT NOT NULL,
    label       TEXT NOT NULL,
    description TEXT
);

-- Roles: system roles (entity_id IS NULL, immutable) + per-tenant custom roles.
CREATE TABLE roles (
    id           UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    entity_id    UUID NULL,                 -- NULL = built-in/system role
    key          TEXT NOT NULL,             -- 'owner','admin',... or custom slug
    name         TEXT NOT NULL,
    description  TEXT,
    is_system    BOOLEAN NOT NULL DEFAULT false,
    is_assignable BOOLEAN NOT NULL DEFAULT true,
    created_at   TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at   TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (entity_id, key)                 -- system keys unique globally (entity_id NULL)
);

CREATE TABLE role_permissions (
    role_id        UUID NOT NULL REFERENCES roles(id) ON DELETE CASCADE,
    permission_key TEXT NOT NULL REFERENCES permissions(key) ON DELETE CASCADE,
    PRIMARY KEY (role_id, permission_key)
);

-- era_users keeps `role TEXT` as the role KEY (no destructive migration; the JWT
-- role claim stays a string). Add an activation token like the other portals.
ALTER TABLE era_users
    ADD COLUMN IF NOT EXISTS set_token         TEXT,
    ADD COLUMN IF NOT EXISTS set_token_expires TIMESTAMPTZ;
CREATE INDEX IF NOT EXISTS idx_era_users_set_token ON era_users(set_token);
```

**Role resolution** for a request with `(entity_id, role_key)`:
```sql
SELECT r.id FROM roles r
WHERE r.key = $role_key
  AND (r.entity_id = $entity_id OR (r.entity_id IS NULL AND r.is_system))
ORDER BY r.entity_id NULLS LAST      -- prefer a tenant's own role over system
LIMIT 1;
```
Custom roles must use keys distinct from system keys, so resolution is
unambiguous. The effective permission set is the `role_permissions` for that
role id.

### 3.3 Enforcement — `require_permission`

Introduce in `middleware/auth.rs`:

```rust
pub async fn require_permission(state: &AppState, ctx: &AuthContext, perm: &str)
    -> Result<(), ErpError>;   // Ok or ErpError::PermissionDenied
```

- Resolves `ctx.role` (key) + `ctx.entity_id` → permission set via a **cache**
  (in-memory `HashMap<(Uuid entity, String role_key), Arc<HashSet<String>>>`
  behind an `RwLock`, invalidated whenever a role's permissions change; cheap and
  correct for a single-node API).
- `require_role(...)` is kept as a thin, deprecated shim during transition (it
  can be re-expressed as membership in the roles that carry the mapped
  permission) so call sites migrate incrementally with zero behaviour change.

**Default-deny coverage.** Add a **route→permission registry** and enforce it
centrally rather than per-handler:

```rust
// One entry per mutating route. `read.all` for read-only, an explicit permission
// for writes, `Public` for the auth/portal endpoints.
static ROUTE_PERMS: &[(Method, &str, Access)] = &[
    (POST, "/api/v1/journal-entries", Perm("journal.post")),
    (POST, "/api/v1/invoices",        Perm("sales.manage")),
    // ...
];
```
- A tower layer looks up `(method, matched_path)` and calls `require_permission`
  before the handler; handlers keep their inline checks as defence-in-depth
  during the transition, then rely on the layer.
- A **unit test** walks the Axum router and asserts every `POST/PUT/PATCH/DELETE`
  route has a `ROUTE_PERMS` entry — CI fails if a new mutating route is
  undeclared. This is the mechanical guarantee that closes the opt-in gap.

### 3.4 Frontend single source of truth

- New endpoint `GET /api/v1/auth/permissions` → `{ role, permissions: [keys] }`
  for the current user (from the same cache).
- Replace the hard-coded arrays in `utils/roles.ts` with a `PermissionsProvider`
  + `can(key)` / `useCan()` hook fed by that endpoint. Buttons gate on `can(...)`.
  Drift becomes impossible — the UI reads the server's truth.

## 4. User lifecycle (internal `era_users`)

Mirror the staff portal exactly (`routes/staff_auth.rs`):

- **Invite** (`POST /users`, `users.manage`): create `status='invited'`; if no
  password, generate `set_token` (7-day expiry) and email a set-password link
  via the existing notifications service.
- **Activate / reset** — new public endpoints:
  - `POST /api/v1/auth/set-password` `{token, password}` → sets hash, `status='active'`.
  - `POST /api/v1/auth/forgot-password` `{email}` → issues token, emails link
    (no account enumeration).
- **Manage** (`PUT /users/{id}`, `users.manage`): change role, activate/
  deactivate — already implemented server-side incl. **sole-Owner protection**;
  just needs UI. Add **resend invite** and **remove/deactivate self-guard**.

## 5. Admin UX specification

Conventions reused from the existing app: `PageHeader`, `DataTable`, `Modal`,
`StatCard`, `.btn-primary/.btn-secondary`, `.input/.label`, lucide icons,
react-query. All actions gate on `can(key)` (§3.4); the server remains the real
authority. Every list has explicit **loading / empty / error / permission-denied**
states (empty = friendly guidance, not a blank table).

### 5.1 Information architecture
- Sidebar **ADMIN** group: rename "Users & Roles" to two entries —
  **Users** (`/users`, needs `users.manage`) and **Roles** (`/roles`, needs
  `roles.manage`). Both hidden from the nav when the user lacks the permission.
- Public (no-auth) routes: `/set-password?token=…` and `/forgot-password`.

### 5.2 Users page (`/users`) — `pages/settings/UsersPage.tsx`
Layout: `PageHeader("Users", subtitle, actions=[Invite user])` over a full-width
`DataTable`. A small stat strip on top: Active · Invited · Deactivated counts.

Table columns: **Name** (display_name + "you" chip on self) · **Email** ·
**Role** (badge; system roles indigo, custom roles slate) · **Status**
(active=green, invited=amber, deactivated=gray) · **Last login** (relative) ·
**⋯ actions**.

Row actions (kebab): **Edit** · **Resend invite** (only `status='invited'`) ·
**Deactivate**/**Reactivate**. No hard delete in v1 (deactivate is the safe
default; hard-delete only for `invited` stubs with a confirm).

**Invite user** modal:
- Fields: Full name, Email, **Role** (`<select>` from `GET /roles` where
  `is_assignable`, grouped: *System roles* then *Custom roles*; shows the role's
  one-line description under the select), and **Activation** radio:
  ① *Send set-password email* (default) or ② *Set a temporary password now*
  (reveals a password field, min 8).
- On submit → `POST /users`. Success toast differs per path
  ("Invite emailed to X" vs "X can sign in now"). Inline field errors; duplicate
  email → "A user with this email already exists."

**Edit user** modal (wires the missing `updateUser` client fn → `PUT /users/{id}`):
- Editable: Role, Active toggle. Read-only: email.
- **Sole-Owner guard** (server-enforced) surfaced *proactively*: if the target is
  the only active Owner, the Role select and Active toggle are disabled with a
  tooltip "This is the workspace's only Owner — add another Owner first." Mirror
  the server 422 as an inline error if it still occurs.
- **Self-guard:** you cannot deactivate yourself or drop your own last admin
  right; those options are disabled with an explanatory tooltip.
- Save shows a busy state; on success, optimistic row update + `refreshCrm`-style
  cancel-then-invalidate of `['users']`.

### 5.3 Roles page (`/roles`) — new `pages/settings/RolesPage.tsx`
Two-pane master–detail (stacks on mobile):

- **Left — role list:** system roles (lock icon, "Built-in" chip) then custom
  roles; each shows name + assigned-user count. Actions: **New role**, and per
  custom role a kebab (**Rename**, **Duplicate**, **Delete**). Selecting a role
  loads the matrix on the right.
- **Right — permission matrix editor:** the permission catalog grouped by
  category (Sales, Purchases, Accounting, HR, CRM, Reports, Admin …). Each group
  is a collapsible section with a **group master checkbox** (checked / indeterminate /
  unchecked → toggles all in the group) and per-permission rows: checkbox +
  label + description tooltip.
  - **System roles:** matrix is **read-only** (checkboxes disabled, "Built-in role — duplicate to customise" banner + a **Duplicate to edit** button).
  - **Custom roles:** editable. A sticky footer shows **"N changes"** with
    **Save** / **Discard**; navigating away while dirty prompts. Save →
    `PUT /roles/{id}` then invalidates `['roles']` and `['auth','permissions']`
    (so the current user's own `can()` refreshes if their role changed).
  - Search box filters permissions by key/label across groups.

**New / Duplicate role** modal: Name, optional description, and *"Start from"*
(Blank or an existing role to clone its permissions). Key is a server-generated
slug; blocked from colliding with system keys.

**Delete role**: allowed only when **no active users** hold it; otherwise the
dialog lists affected users and offers **"Reassign to …"** before deleting.
Confirmation is type-to-confirm for destructive delete.

### 5.4 Activation & recovery screens (public)
Reuse the staff-portal component shape (`StaffSetPasswordPage`,
`StaffLoginPage`'s forgot flow), branded for internal users:
- **Set password** (`/set-password?token=…`): password + confirm, strength hint,
  min 8; consumes the token → success → redirect to `/login`. Invalid/expired
  token → "This link has expired — ask an admin to resend your invite."
- **Forgot password** (`/forgot-password`, linked from `/login`): email field →
  always shows the same neutral confirmation (no account enumeration).

### 5.5 Cross-cutting UX
- **Gating:** the whole Users/Roles nav + actions use `can('users.manage')` /
  `can('roles.manage')`. A user who deep-links without permission sees a
  "You don't have access to manage users/roles" card (not a redirect loop).
- **Accessibility:** modals trap focus + ESC to close (existing `Modal`);
  matrix checkboxes are real `<input type=checkbox>` with `<label>`; status/role
  conveyed by text, not colour alone.
- **Responsive:** Roles master–detail collapses to a role `<select>` + matrix on
  small screens; Users table becomes stacked cards.
- **Microcopy:** prefer plain language ("Can post journal entries") over keys in
  end-user surfaces; show the raw `permission.key` only in a subtle monospace
  hint for admins.

## 6. Multi-tenant behaviour

Unchanged model: role is per-`era_users` row (per membership). Custom roles are
**per tenant** (`roles.entity_id = entity`); system roles are shared. On
`switch-tenant`, the re-issued token carries that membership's `role` key, and
`require_permission` resolves it within the target entity — so a person can be
`Owner` in tenant A and a custom `Auditor` in tenant B.

## 7. Rollout plan (non-breaking, phased)

**Phase 0 — model + seed (behaviour-identical).**
`047_rbac.sql`; startup syncs the permission catalog; seed the 7 system roles and
their `role_permissions` to reproduce today's `const` arrays **exactly**
(including `Viewer` getting `payroll.read`/`hr.read` for now). Add resolution +
cache. No enforcement change yet. *Verify: a test asserts each system role's
effective set == the current arrays.*

**Phase 1 — enforce by permission.**
Add `require_permission` + `GET /auth/permissions`; convert `require_role` call
sites (mechanical, behaviour-preserving); frontend switches to `can()`.

**Phase 2 — user lifecycle.**
`set_token` columns + set-password/forgot-password; Users page gains edit/
deactivate/resend; add DB `CHECK`/self-lockout guards.

**Phase 3 — role admin + custom roles.**
`GET/POST/PUT/DELETE /roles`, `GET /permissions`; Roles admin UI.

**Phase 4 — default-deny + read tightening.**
**Phase 4 — default-deny + read tightening.**
Split into two parts:
- **4A · sensitive-read segregation — ✅ DONE.** Added `payroll.read` + `hr.read`
  to the catalog, seeded to every role **except `Viewer`** (Approver still views a
  pay run to approve it, Editor to run one — only the read-only Viewer loses
  salary/employee visibility). Gated the previously **unguarded** reads
  (`GET /payroll`, `/payroll/{id}`, `/payroll/{id}/inputs`, payslip PDF,
  `GET /employees`, `/employees/{id}`, and payroll `report_type`s on `POST /reports`)
  with `require_permission`. Verified: Viewer → 403 on all of these while
  non-payroll reports and every other role are unaffected; golden test
  `sensitive_reads_exclude_only_viewer` locks it in.
- **4B · authorization-coverage guarantee — ✅ DONE.** Rather than a risky
  big-bang runtime layer over 270+ routes, the opt-in gap is closed by a
  **static-analysis CI test** (`tests/authz_coverage.rs`): it parses `main.rs`,
  finds every mutating route (`post`/`put`/`patch`/`delete`), resolves each to its
  handler, and asserts the handler body performs a `require_role`/
  `require_permission` check — or is on a reviewed `ALLOWLIST` of intentionally
  public / principal-extractor-guarded (Customer/Vendor/Staff) / self-scoped /
  read-only handlers. A new mutating route without a check now fails CI, so
  authorization can never silently regress to default-allow. The one-time audit
  surfaced 6 handlers, all confirmed legitimately guarded (per-entity membership,
  self-scoped notifications, vendor portal, read-only compute/validate) — **no
  genuine gap found**. An optional future hardening is a runtime central layer
  built from a declarative router table (single source for routing + a
  permission registry); not required now that coverage is guaranteed.

Each phase is independently shippable and reversible; Phases 0–2 are pure
additions with no behaviour change.

## 8. Security considerations

- Enforcement stays server-side; the UI `can()` layer is UX only (documented).
- Cache invalidation on any `roles`/`role_permissions` write (and on tenant
  switch the token already rebinds).
- `roles.manage` is powerful — restrict to Owner/Admin; forbid editing/deleting
  system roles and deleting a role still assigned to active users (reassign
  first). Keep the existing sole-Owner protection.
- `era_users.role` gains integrity: it must resolve to a role row; add a guard at
  write time and a startup check.

## 9. Verification

- **Behaviour-preserving tests:** each of the 7 system roles' effective
  permissions equals the pre-migration `const` arrays (golden test).
- **Coverage test:** every mutating route has a `ROUTE_PERMS` entry.
- **Migration:** `047_rbac.sql` validated in a `BEGIN/ROLLBACK` tx, then applied
  on startup; idempotent catalog sync.
- **Workspace tests** green; **`tsc --noEmit`** clean; **Playwright**: invite →
  set-password → login; edit role/deactivate; create a custom role and confirm a
  gated action toggles.

## 10. Estimated effort

| Phase | Backend | Frontend | Risk |
|---|---|---|---|
| 0 model+seed | migration, catalog, resolver+cache, golden test | – | low |
| 1 enforce | `require_permission`, `/auth/permissions`, convert sites | `can()` provider | low |
| 2 lifecycle | set-token cols, 2 endpoints, guards | edit/deactivate/resend + set-pw screens | low |
| 3 role admin | roles CRUD | Roles matrix UI | med |
| 4 default-deny | route registry + layer + test; read split | minor | med |

Recommended order to ship value fastest: **0 → 2 → 1 → 4 → 3** (get real user
admin + activation out early, harden coverage, custom-role UI last).
