# Implementation Plan: Tenant Signup

## Overview

This plan implements true multi-tenant signup for Zavora ERP and separates it from the existing invite flow, while shifting authenticated request scoping from the process-global `ENTITY_ID` to the tenant carried in each verified JWT.

Work proceeds bottom-up: schema migration and core types first, then the pure validation function, the transaction-aware `Tenant_Provisioner`, the public `Signup_Service` route and `Rate_Limiter`, the `Tenant_Scope_Resolver` middleware change, the invite-boundary and sole-Owner guard, and finally legacy-compat and end-to-end integration tests. Each step builds on the previous and ends wired into the running API.

All code is **Rust** (per the design), in the `zavora-erp-core` and `zavora-erp-api` crates. Property-based tests use **`proptest`** (added under `[dev-dependencies]`), run a minimum of 100 cases each, and are tagged with a comment in the format `// Feature: tenant-signup, Property {number}: {property text}`.

## Tasks

- [x] 1. Schema migration and core tenant module scaffolding
  - [x] 1.1 Add `organization_name` migration
    - Create `migrations/007_tenant_signup.sql` adding `entity_settings.organization_name TEXT NOT NULL DEFAULT 'My Company'` (idempotent `ADD COLUMN IF NOT EXISTS`)
    - Confirm the column is picked up by `sqlx::migrate!` on startup
    - _Requirements: 12.1_

  - [x] 1.2 Create the `tenant` module and request/result types
    - Create `zavora-erp-core/src/tenant/mod.rs` and register `pub mod tenant;` in `zavora-erp-core/src/lib.rs`
    - Define `SignupInput`, `ProvisionTenantRequest` (includes `seed_chart_of_accounts: bool`), and `ProvisionedTenant` structs as specified in the design
    - Add `proptest` to `[dev-dependencies]` in `zavora-erp-core/Cargo.toml`
    - _Requirements: 1.4, 2.1, 12.1, 12.2_

- [x] 2. Signup input validation (pure, pre-persistence)
  - [x] 2.1 Implement `validate_signup`
    - Implement `validate_signup(input: SignupInput) -> ErpResult<ProvisionTenantRequest>` in `zavora-erp-core/src/tenant/mod.rs`
    - Reject empty/whitespace organization name, empty/whitespace display name, syntactically invalid email, and passwords shorter than 8 chars; each error is `ErpError::ValidationFailed { message }` naming exactly one offending field and revealing no tenant/user identifiers
    - Normalise: trim organization name and display name; trim and lower-case email; leave password unchanged
    - _Requirements: 1.6, 7.1, 7.2, 7.3, 7.4, 7.5, 10.3_

  - [-]* 2.2 Write property test for `validate_signup`
    - **Property 1: Signup input validation is total and field-accurate**
    - **Validates: Requirements 1.6, 7.1, 7.2, 7.3, 7.5, 10.3**
    - Generators: organization/display names (including whitespace-only and unicode), valid and malformed emails, passwords straddling the 8-char boundary

- [ ] 3. Auth primitives property coverage (existing `auth` functions)
  - [-]* 3.1 Write property test for password hashing
    - **Property 6: Password is hashed, never stored in plaintext**
    - **Validates: Requirements 2.6, 2.3**
    - Assert `hash_password` output is Argon2id, differs from plaintext, verifies for the correct password, and fails for a different password

  - [-]* 3.2 Write property test for access-token tenant claim
    - **Property 7: Access token carries the owning tenant**
    - **Validates: Requirements 5.3**
    - For any `(user_id, entity_id, role)`, the token from `issue_token_pair` decodes to claims whose `entity_id` and `role` match the inputs

  - [-]* 3.3 Write property test for invalid-token rejection
    - **Property 8: Invalid tokens are rejected**
    - **Validates: Requirements 5.4**
    - Tampered bytes, wrong token type (refresh presented as access), and expired tokens all cause `decode_access_token` to error

- [x] 4. Transaction-aware chart-of-accounts seeding helper
  - [x] 4.1 Implement `seed_coa_in_tx`
    - Implement `seed_coa_in_tx(tx: &mut sqlx::Transaction<'_, sqlx::Postgres>, entity_id: Uuid, template: &CoaTemplate) -> ErpResult<u32>` in `zavora-erp-core/src/tenant/mod.rs`
    - Insert every `kenya_standard_coa()` account scoped to the supplied `entity_id`, running inside the caller's open transaction (not the auto-committing pool), and return the count seeded
    - _Requirements: 3.2, 3.4_

- [x] 5. Tenant_Provisioner (atomic, single transaction)
  - [x] 5.1 Implement `provision_tenant`
    - Implement `provision_tenant(pool: &PgPool, req: ProvisionTenantRequest) -> ErpResult<ProvisionedTenant>` in `zavora-erp-core/src/tenant/mod.rs`
    - Generate `entity_id = Uuid::new_v4()`; open one `sqlx::Transaction`; hash the password with `auth::hash_password`; insert the `entity_settings` row (`organization_name`, `base_currency='KES'`, `coa_template='KenyaStandard'`); insert the Owner `era_users` row (role `Owner`, active, Argon2id hash) mapping `UNIQUE(entity_id, email)` violations to a generic `Duplicate` error; when `seed_chart_of_accounts` is set, call `seed_coa_in_tx`; insert the `audit_events` tenant-creation row (no password/hash); commit
    - On any error, return `Err` before commit so the transaction rolls back and no rows reference the candidate `entity_id`
    - _Requirements: 1.2, 2.1, 2.2, 2.3, 2.4, 2.5, 2.6, 3.1, 3.2, 3.4, 3.5, 8.1, 8.3, 11.1, 11.2, 11.3, 12.1, 12.2, 12.3, 13.3, 14.1, 14.2_

  - [ ]* 5.2 Write property test for validation-persists-nothing
    - **Property 2: Validation failure persists nothing**
    - **Validates: Requirements 7.4**
    - Run against a transactional test database; assert row counts for `entity_settings`, `era_users`, `accounts`, `audit_events` are unchanged after invalid input

  - [ ]* 5.3 Write property test for provisioning postconditions
    - **Property 3: Successful provisioning postconditions**
    - **Validates: Requirements 1.2, 2.2, 2.3, 3.1, 11.1, 11.2, 12.1, 13.3**

  - [ ]* 5.4 Write property test for seeded chart of accounts
    - **Property 4: Seeded chart of accounts is complete and tenant-scoped**
    - **Validates: Requirements 3.2, 3.4**

  - [ ]* 5.5 Write property test for distinct tenant identifiers
    - **Property 5: Tenant identifiers are always distinct**
    - **Validates: Requirements 2.1, 12.2, 12.3, 14.3**
    - Include signups with identical organization names and retries of failed attempts

  - [ ]* 5.6 Write property test for cross-tenant email reuse
    - **Property 12: Cross-tenant email reuse is allowed**
    - **Validates: Requirements 8.2**

  - [ ]* 5.7 Write property test for audit secrecy
    - **Property 13: Audit records never contain secrets**
    - **Validates: Requirements 11.3**
    - Serialize the tenant-creation audit record and assert it contains neither the plaintext password nor any Argon2id hash substring

- [~] 6. Checkpoint - core provisioning
  - Ensure all tests pass, ask the user if questions arise.

- [x] 7. Signup_Service route and Rate_Limiter
  - [x] 7.1 Implement `check_signup_rate`
    - Create `zavora-erp-api/src/routes/auth_signup.rs` with `check_signup_rate(redis, client_key) -> ErpResult<()>` as a Redis fixed-window counter (`INCR` + `EXPIRE`), threshold/window from `SIGNUP_RATE_MAX`/`SIGNUP_RATE_WINDOW_SECS` with safe defaults
    - Fail open with a warning log when Redis is unavailable
    - Add `proptest` to `[dev-dependencies]` in `zavora-erp-api/Cargo.toml`
    - _Requirements: 10.1_

  - [ ]* 7.2 Write property test for rate limiting
    - **Property 17: Rate limiting on public signup**
    - **Validates: Requirements 10.1**
    - For threshold N and window W, exactly the first N requests in the window are admitted and every subsequent request is rejected

  - [x] 7.3 Implement the `signup` handler
    - In `auth_signup.rs`, implement `POST /api/v1/auth/signup`: derive client key and call `check_signup_rate` (429 over limit); `validate_signup` (400 with field name); `tenant::provision_tenant` (duplicate Owner email returns a generic non-enumerating 409); `auth::issue_token_pair` and `store_refresh_token`; respond via the existing `auth_success` shape with access token + owner identity in the body and the refresh token only in the `era_refresh` httpOnly `SameSite=Strict` cookie
    - _Requirements: 1.1, 1.2, 1.3, 1.4, 1.5, 5.3, 8.3, 10.2_

  - [x] 7.4 Wire the signup route into the public router
    - Register `/api/v1/auth/signup` on the public (unauthenticated) router in `zavora-erp-api/src/main.rs`
    - _Requirements: 1.1_

  - [ ]* 7.5 Write property test for signup success response shape
    - **Property 11: Signup success response shape**
    - **Validates: Requirements 1.3, 1.4, 1.5**
    - Assert the body carries the access token and owner identity and never the refresh token value; the refresh token appears only as the httpOnly `SameSite=Strict` `era_refresh` cookie

  - [ ]* 7.6 Write property test for non-enumerating duplicate-email response
    - **Property 18: Non-enumerating duplicate-email response**
    - **Validates: Requirements 10.2**
    - The rejection response is identical whether or not the email exists in another tenant

  - [ ]* 7.7 Write unit test for unauthenticated reachability
    - Assert the signup route is reachable with no `Authorization` header
    - _Requirements: 1.1_

- [x] 8. Tenant_Scope_Resolver (per-request scoping)
  - [x] 8.1 Remove the served-entity gate in `verify_bearer`
    - In `zavora-erp-api/src/middleware/auth.rs`, remove the `claims.entity_id != served_entity()` rejection while still decoding/verifying the access token and building `AuthContext { user_id, entity_id, role }` from the verified claims; retain `served_entity()` only for the legacy `register` bootstrap path
    - _Requirements: 4.1, 4.2, 4.3, 4.4, 5.1, 5.4, 9.1, 9.4_

  - [x] 8.2 Add a request-scoped `TenantScope` handle
    - Add `ErpEngine::scoped(&self, entity_id: Uuid) -> TenantScope<'_>` and the `TenantScope` type in `zavora-erp-core/src/engine.rs`, exposing `entity_id()` plus `pool()`/`redis()` forwarding, so handlers scope by the per-request `entity_id` from `AuthContext` instead of `engine.entity_id()`
    - _Requirements: 4.2, 5.1, 9.4_

  - [ ]* 8.3 Write property test for request scope resolution
    - **Property 9: Request scope equals the verified token's tenant**
    - **Validates: Requirements 4.1, 4.2, 4.3, 4.4, 9.1, 9.4**

  - [ ]* 8.4 Write property test for cross-tenant isolation
    - **Property 10: Cross-tenant isolation**
    - **Validates: Requirements 5.1, 5.2**
    - For a multi-tenant data set, a scope to tenant A returns only A's rows and a request for another tenant's resource resolves to not-found

- [~] 9. Checkpoint - signup endpoint and scoping live
  - Ensure all tests pass, ask the user if questions arise.

- [x] 10. Invite boundary and sole-Owner protection
  - [x] 10.1 Implement first-Owner protection in the user update/deactivate path
    - In `zavora-erp-api/src/routes/users.rs`, before deactivating an Owner or changing an Owner's role away from Owner, count active Owners for the caller's tenant and reject when the count is 1; confirm invite (`create`) sets the new user's `entity_id` to `ctx.entity_id` and creates no tenant
    - _Requirements: 6.1, 6.2, 6.4, 13.1, 13.2_

  - [ ]* 10.2 Write property test for invite tenant targeting
    - **Property 14: Invite targets the caller's tenant and never creates one**
    - **Validates: Requirements 6.1, 6.2, 6.4**

  - [ ]* 10.3 Write property test for invite authorization by role
    - **Property 15: Invite authorization by role**
    - **Validates: Requirements 6.3, 6.6**
    - Permitted exactly when role is Owner or Admin; denied with a permission error otherwise

  - [ ]* 10.4 Write property test for sole-Owner protection
    - **Property 16: Sole-Owner protection**
    - **Validates: Requirements 13.1, 13.2**

  - [ ]* 10.5 Write unit test for unauthenticated invite
    - Invite with no token returns 401
    - _Requirements: 6.5_

- [x] 11. Legacy compatibility
  - [~] 11.1 Deprecate `register` and document `signup` as the supported path
    - Annotate the legacy `POST /api/v1/auth/register` handler as deprecated (doc comment / OpenAPI note) and document `/api/v1/auth/signup` as the supported tenant-creation path, leaving register's bootstrap behaviour unchanged
    - _Requirements: 9.2, 9.3_

- [ ] 12. End-to-end and integration tests (against Postgres)
  - [ ]* 12.1 Write integration test for atomic rollback / abandoned signups
    - Inject a failure after partial inserts (colliding Owner email under `UNIQUE(entity_id, email)` or a forced seeding error) and assert no `entity_settings`, `era_users`, `accounts`, or `audit_events` row references the candidate `entity_id`
    - _Requirements: 2.4, 2.5, 3.5, 14.1, 14.2_

  - [ ]* 12.2 Write integration test for within-tenant duplicate Owner email
    - A duplicate Owner email within the tenant being created is rejected with a duplicate error
    - _Requirements: 8.1, 8.3_

  - [ ]* 12.3 Write integration test for multi-tenant login
    - The same email in two tenants authenticates against the intended tenant's credentials
    - _Requirements: 8.4_

  - [ ]* 12.4 Write end-to-end signup → authenticated request test
    - Sign up, use the returned access token to read tenant-scoped data, and confirm isolation from a second tenant created the same way
    - _Requirements: 1.1, 4.2, 5.1, 5.2_

  - [ ]* 12.5 Write unit test for deferred COA seeding
    - Auto-seed disabled yields zero accounts; a later authenticated `/accounts/seed` populates them
    - _Requirements: 3.3_

- [~] 13. Final checkpoint - full suite green
  - Ensure all tests pass, ask the user if questions arise.

## Notes

- Tasks marked with `*` are optional test tasks and can be skipped for a faster MVP; core implementation tasks are never optional.
- Each task references specific requirements clauses for traceability, and every property test task references its design property by number.
- Property tests use `proptest` with a minimum of 100 cases each and the required `// Feature: tenant-signup, Property {n}: ...` tag; one property maps to exactly one property test.
- Pure-logic properties (1, 6, 7, 8, 9, 15, 17, 18) run in memory; DB-backed properties (2, 3, 4, 5, 10, 11, 12, 13, 14, 16) run against a transactional test database.
- Checkpoints provide incremental validation at natural boundaries.

## Task Dependency Graph

```json
{
  "waves": [
    { "id": 0, "tasks": ["1.1", "1.2"] },
    { "id": 1, "tasks": ["2.1", "3.1", "3.2", "3.3", "7.1", "8.1", "8.2"] },
    { "id": 2, "tasks": ["2.2", "4.1", "7.2"] },
    { "id": 3, "tasks": ["5.1", "8.3", "8.4"] },
    { "id": 4, "tasks": ["5.2", "5.3", "5.4", "5.5", "5.6", "5.7", "7.3"] },
    { "id": 5, "tasks": ["7.4", "10.1"] },
    { "id": 6, "tasks": ["7.5", "7.6", "7.7", "10.2", "10.3", "10.4", "10.5", "11.1"] },
    { "id": 7, "tasks": ["12.1", "12.2", "12.3", "12.4", "12.5"] }
  ]
}
```
