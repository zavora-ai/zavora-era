# Platform Super Admin

Zavora ops plane for multi-tenant administration. **Separate from tenant ERP**
(`era_users` / RBAC). Operators manage tenants as objects; they do not receive a
tenant Owner role unless they open an audited support session.

## Planes

| Plane | Login | JWT role | Scope |
|-------|--------|----------|--------|
| Tenant ERP | `/login` | Owner, Admin, … | One `entity_id` |
| Platform | `/platform/login` | `PlatformSuperAdmin` | All tenants |

Platform tokens are **rejected** on tenant ERP routes (external principal bar).
Support sessions mint a normal tenant role JWT with an `impersonator_id` claim.

## Bootstrap

On API startup, if both env vars are set and no `platform_users` row exists for
the email, a Super Admin is created (idempotent):

```bash
PLATFORM_BOOTSTRAP_EMAIL=ops@zavora.ai
PLATFORM_BOOTSTRAP_PASSWORD=long-secret
# optional
PLATFORM_BOOTSTRAP_NAME=Platform Super Admin
```

Never reuse a tenant Owner password here. Leave unset in environments that
should not auto-create operators.

## Schema (migration 056)

- `platform_users` — ops identities
- `tenants` — directory (synced from `entity_settings` + signup)
- `platform_audit_events` — operator actions

## API (`/api/v1/platform`)

| Method | Path | Purpose |
|--------|------|---------|
| POST | `/auth/login` | Ops login |
| POST | `/auth/refresh` | Cookie `era_platform_refresh` |
| POST | `/auth/logout` | Revoke refresh |
| GET | `/me` | Current operator |
| GET | `/tenants` | Directory (`q`, `plan_status`, `hide_empty`, `hide_archived`, `limit`, `offset`) |
| GET | `/tenants/{id}` | Detail: summary + users + recent audit |
| PATCH | `/tenants/{id}` | Plan key/status (`active` \| `trial` \| `past_due`) |
| POST | `/tenants/{id}/suspend` | Suspend + revoke sessions |
| POST | `/tenants/{id}/unsuspend` | Restore access |
| POST | `/tenants/{id}/archive` | Soft-archive + revoke sessions |
| POST | `/tenants/{id}/unarchive` | Clear archive |
| POST | `/tenants/{id}/impersonate` | Support session (`{ user_id? }`) |
| GET | `/audit` | Global ops audit log |

### Suspension

- Sets `tenants.suspended_at`, `suspended_reason`, `plan_status = suspended`
- Revokes all `refresh_tokens` for that `entity_id`
- Tenant login / refresh return 401 with a clear message
- Support impersonation is still allowed so ops can diagnose

### Impersonation

- Default target: first active **Owner** (else first active user)
- Optional `user_id` for a specific active staff user
- Access ~30 min, refresh ~2 h (`PLATFORM_IMPERSONATE_*_TTL_SECS`)
- Sets tenant `era_refresh` cookie; UI shows amber support banner

## UI

- `/platform/login` — ops sign-in
- `/platform` — tenants table, filters, pagination, row drawer (users, plan, audit)
- **Audit log** tab — global recent actions
- Tenant ERP: support banner with **Exit to platform**

## Phases

| Phase | Status | Scope |
|-------|--------|--------|
| 0 | Shipped | Bootstrap, login, directory |
| 1 | Shipped | Suspend / restore, impersonate |
| 2 | This work | Detail drawer, plan, archive, audit UI, pagination, targeted Open |

## Ops checklist (production)

1. Set bootstrap env once; rotate password after first login if needed
2. Restrict `/platform` at the edge (IP allowlist / VPN) in addition to auth
3. Monitor `platform_audit_events` for `impersonate_tenant` / `suspend_tenant`
4. Do not share platform credentials with tenant customers
