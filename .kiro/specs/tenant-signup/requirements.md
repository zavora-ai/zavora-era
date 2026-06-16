# Requirements Document

## Introduction

This feature introduces true multi-tenant **signup** to Zavora ERP and separates it cleanly from the existing **invite** flow. Today the platform conflates two distinct concerns:

- **Signup (tenant creation)** — a brand-new organization registers, which must create a *new* tenant, seed its baseline configuration, create its first Owner user, and return an authenticated session. This is public and unauthenticated.
- **Invite (add user to an existing tenant)** — already implemented via `POST /users`; an authenticated Owner or Admin adds a user to *their own* tenant.

The current codebase is single-tenant-per-process: a process-global served entity (an `OnceLock<Uuid>` set at startup from the `ENTITY_ID` environment variable) fixes the one tenant the process serves, and the authentication middleware rejects any token whose `entity_id` differs from that served entity. Data access throughout the services and routes scopes by `engine.entity_id()`, which returns the fixed startup entity. The existing `POST /auth/register` only bootstraps the first Owner for that fixed entity and is not true tenant creation.

The database schema is already multi-tenant-ready: every business table is keyed by `entity_id` (for example `accounts`, `customers`, `invoices`, `entity_settings`, `era_users`, `refresh_tokens`). The change required is at runtime: signup must create new tenants on demand, and authenticated requests must be scoped to the tenant carried in the verified JWT rather than to a single process-global entity.

This is a P0 architectural change. These requirements describe *what* the system must do; technical migration strategy is deferred to design. No application code is changed by this document.

## Glossary

- **Tenant**: An isolated organization and all data keyed by its `entity_id`. One tenant corresponds to exactly one `entity_id`.
- **Entity_Id**: The UUID primary key that scopes all of a tenant's data across every table.
- **Signup_Service**: The component that handles public, unauthenticated tenant creation requests.
- **Tenant_Provisioner**: The component that atomically creates a new tenant's `entity_id`, its `entity_settings` row, and its first Owner user.
- **Invite_Service**: The existing authenticated component (`POST /users`) that adds users to the caller's existing tenant.
- **Auth_Service**: The component that hashes passwords, issues JWT access tokens and server-side refresh tokens, and sets the refresh cookie.
- **Tenant_Scope_Resolver**: The component that determines, for each authenticated request, the `entity_id` whose data the request may access, derived from the verified JWT claims.
- **Access_Token**: A short-lived signed JWT carrying `sub` (user id), `entity_id`, and `role`, returned in the response body.
- **Refresh_Token**: A longer-lived JWT persisted server-side in `refresh_tokens` and delivered as an httpOnly, SameSite=Strict cookie.
- **Owner**: The highest-privilege role; the first user created for a tenant at signup is an Owner.
- **Role**: One of Owner, Admin, Accountant, Approver, Editor, Viewer.
- **Entity_Settings**: The per-tenant configuration row (`entity_settings`), including base currency, fiscal year end, chart-of-accounts template, sequences, and tax config.
- **Chart_Of_Accounts**: The set of `accounts` rows for a tenant. The default template is `KenyaStandard`.
- **Organization_Name**: The human-readable display name supplied for a tenant at signup.
- **Served_Entity**: The single process-global `entity_id` loaded at startup from the `ENTITY_ID` environment variable (legacy single-tenant mode).
- **Rate_Limiter**: The component that throttles public signup attempts to limit abuse.
- **Audit_Log**: The append-only `audit_events` store that records significant security and lifecycle events.
- **Password_Policy**: The set of rules a password must satisfy to be accepted.

## Requirements

### Requirement 1: Public Tenant Signup Endpoint

**User Story:** As a prospective customer with no existing account, I want to sign up my organization, so that I get a new isolated tenant and an authenticated session without an invitation.

#### Acceptance Criteria

1. THE Signup_Service SHALL expose a public endpoint that requires no `Authorization` header and no existing session.
2. WHEN a signup request is received with an Organization_Name, an Owner email, an Owner display name, and a password, THE Signup_Service SHALL invoke the Tenant_Provisioner to create a new tenant.
3. WHEN tenant provisioning succeeds, THE Auth_Service SHALL return an Access_Token in the response body and set a Refresh_Token as an httpOnly SameSite=Strict cookie.
4. WHEN tenant provisioning succeeds, THE Signup_Service SHALL return the new Entity_Id, the Owner user id, the Owner email, the Owner display name, and the role `Owner` in the response body.
5. THE Signup_Service SHALL NOT include the Refresh_Token in the response body.
6. IF a required field (Organization_Name, Owner email, Owner display name, or password) is missing or empty, THEN THE Signup_Service SHALL reject the request with a validation error identifying the missing field.

### Requirement 2: Atomic Tenant Provisioning

**User Story:** As a platform operator, I want tenant creation to be all-or-nothing, so that a failed signup never leaves an orphaned tenant, settings row, or user behind.

#### Acceptance Criteria

1. WHEN the Tenant_Provisioner creates a tenant, THE Tenant_Provisioner SHALL generate a new unique Entity_Id that does not collide with any existing tenant.
2. WHEN the Tenant_Provisioner creates a tenant, THE Tenant_Provisioner SHALL create exactly one Entity_Settings row for the new Entity_Id with the platform default base currency `KES`.
3. WHEN the Tenant_Provisioner creates a tenant, THE Tenant_Provisioner SHALL create exactly one Owner user for the new Entity_Id with an Argon2id-hashed password and an active status.
4. THE Tenant_Provisioner SHALL persist the new Entity_Id, the Entity_Settings row, and the Owner user within a single database transaction.
5. IF any step of tenant provisioning fails, THEN THE Tenant_Provisioner SHALL roll back the transaction so that no Entity_Settings row, Owner user, or Chart_Of_Accounts rows are persisted for the new Entity_Id.
6. WHEN the Tenant_Provisioner stores the password, THE Tenant_Provisioner SHALL store only the Argon2id hash and SHALL NOT store the plaintext password.

### Requirement 3: Chart of Accounts Provisioning at Signup

**User Story:** As a new Owner, I want my tenant's chart of accounts to be ready, so that I can start bookkeeping without manual setup.

#### Acceptance Criteria

1. THE Entity_Settings row created at signup SHALL record the chart-of-accounts template `KenyaStandard`.
2. WHERE automatic seeding is enabled, THE Tenant_Provisioner SHALL seed the new tenant's Chart_Of_Accounts from the `KenyaStandard` template within the same transaction that creates the tenant.
3. WHERE automatic seeding is disabled, THE Tenant_Provisioner SHALL create the tenant without Chart_Of_Accounts rows and the Owner SHALL be able to seed the Chart_Of_Accounts later through the authenticated accounts-seed endpoint.
4. WHEN the Chart_Of_Accounts is seeded for a tenant, THE Tenant_Provisioner SHALL associate every seeded account row with the new tenant's Entity_Id.
5. IF Chart_Of_Accounts seeding fails during signup while automatic seeding is enabled, THEN THE Tenant_Provisioner SHALL roll back the entire tenant-creation transaction.

### Requirement 4: Per-Request Tenant Scoping

**User Story:** As a tenant user, I want every authenticated request to operate on my own tenant, so that the platform can serve many tenants from one deployment.

#### Acceptance Criteria

1. WHEN an authenticated request is processed, THE Tenant_Scope_Resolver SHALL determine the request's Entity_Id from the verified Access_Token claims.
2. WHILE a request is being processed, THE Tenant_Scope_Resolver SHALL scope all data reads and writes to the Entity_Id derived from the verified Access_Token.
3. THE Tenant_Scope_Resolver SHALL derive the request Entity_Id from the verified Access_Token rather than from a process-global Served_Entity.
4. WHEN two authenticated requests carry Access_Tokens for different tenants, THE Tenant_Scope_Resolver SHALL scope each request to its own token's Entity_Id independently.

### Requirement 5: Cross-Tenant Isolation

**User Story:** As a tenant Owner, I want it to be impossible for another tenant to read or change my data, so that my organization's records stay confidential and intact.

#### Acceptance Criteria

1. WHEN a request carries an Access_Token for tenant A, THE Tenant_Scope_Resolver SHALL restrict the request to data whose Entity_Id equals tenant A's Entity_Id.
2. IF a request attempts to read or modify a resource whose Entity_Id differs from the Access_Token's Entity_Id, THEN THE Tenant_Scope_Resolver SHALL deny the request with a not-found or forbidden result and SHALL NOT return the other tenant's data.
3. WHEN a user authenticates, THE Auth_Service SHALL set the Access_Token `entity_id` claim to the Entity_Id of the tenant that owns the authenticating user.
4. THE Auth_Service SHALL reject an Access_Token whose signature is invalid, whose type is not `access`, or whose expiry has passed.

### Requirement 6: Separation of Signup from Invite

**User Story:** As an Owner, I want signup and inviting teammates to be distinct operations, so that creating an organization and adding users to it cannot be confused or misused.

#### Acceptance Criteria

1. THE Signup_Service SHALL create a new tenant and SHALL NOT add a user to any existing tenant.
2. THE Invite_Service SHALL add a user to the caller's existing tenant and SHALL NOT create a new tenant.
3. WHEN the Invite_Service receives a request, THE Invite_Service SHALL require a valid Access_Token whose role is Owner or Admin.
4. WHEN the Invite_Service creates a user, THE Invite_Service SHALL set the new user's Entity_Id to the caller's token Entity_Id.
5. IF a caller invokes the Invite_Service without a valid Access_Token, THEN THE Auth_Service SHALL reject the request as unauthenticated.
6. IF an authenticated caller whose role is not Owner or Admin invokes the Invite_Service, THEN THE Invite_Service SHALL reject the request with a permission-denied error.

### Requirement 7: Signup Input Validation

**User Story:** As a platform operator, I want signup inputs validated, so that only well-formed credentials and identities create tenants.

#### Acceptance Criteria

1. IF the supplied Owner email is not a syntactically valid email address, THEN THE Signup_Service SHALL reject the request with a validation error identifying the email field.
2. IF the supplied password is shorter than 8 characters, THEN THE Signup_Service SHALL reject the request with a validation error identifying the password field.
3. THE Signup_Service SHALL enforce the Password_Policy that a password contains at least 8 characters.
4. WHEN the Signup_Service validates the password, THE Signup_Service SHALL reject the request before any tenant data is persisted if the Password_Policy is not satisfied.
5. IF the Organization_Name consists only of whitespace, THEN THE Signup_Service SHALL reject the request with a validation error identifying the Organization_Name field.

### Requirement 8: Email Uniqueness Semantics

**User Story:** As a user, I want my email handled consistently across tenants, so that the same address can belong to more than one organization without breaking sign-in.

#### Acceptance Criteria

1. THE Tenant_Provisioner SHALL treat an Owner email as unique within a single tenant, matching the existing per-tenant uniqueness constraint `UNIQUE(entity_id, email)`.
2. WHERE the same email address is already associated with a different tenant, THE Signup_Service SHALL allow the new signup to proceed and create the new tenant.
3. IF a signup supplies an Owner email that already exists as an active user within the tenant being created, THEN THE Tenant_Provisioner SHALL reject the request with a duplicate-email error.
4. WHEN an email address is associated with users in more than one tenant, THE Auth_Service SHALL authenticate the user against the credentials of the tenant identified for that sign-in.

### Requirement 9: Deployment and Backward Compatibility

**User Story:** As a platform operator running the current single-tenant deployment, I want the new signup capability to coexist with the existing `ENTITY_ID` deployment, so that upgrading does not break running installations.

#### Acceptance Criteria

1. WHERE a deployment sets the `ENTITY_ID` environment variable, THE Auth_Service SHALL continue to accept valid Access_Tokens issued for that Served_Entity.
2. WHEN the legacy `register` endpoint is invoked for the Served_Entity that has no active users, THE Auth_Service SHALL continue to bootstrap the first Owner for that Served_Entity.
3. WHEN the new Signup_Service is available, THE platform SHALL designate the Signup_Service as the supported path for creating new tenants and SHALL mark the legacy `register` endpoint as deprecated.
4. WHILE a deployment operates in legacy single-tenant mode, THE Tenant_Scope_Resolver SHALL scope authenticated requests to the verified Access_Token Entity_Id, which equals the Served_Entity for tokens issued by that deployment.

### Requirement 10: Abuse Protection on Public Signup

**User Story:** As a platform operator, I want the public signup endpoint protected against abuse, so that automated clients cannot create tenants without limit or probe for existing accounts.

#### Acceptance Criteria

1. WHEN signup requests from a single client source exceed the configured rate threshold within the configured time window, THE Rate_Limiter SHALL reject further signup requests from that source with a rate-limited error.
2. WHEN a signup is rejected because the Owner email already exists within the tenant being created, THE Signup_Service SHALL return a response that does not reveal whether the email exists in any other tenant.
3. THE Signup_Service SHALL return validation errors that do not enumerate existing tenants or existing users in other tenants.

### Requirement 11: Audit of Tenant Creation

**User Story:** As a compliance reviewer, I want tenant creation recorded, so that I can trace when and by whom each organization was provisioned.

#### Acceptance Criteria

1. WHEN a tenant is successfully provisioned, THE Audit_Log SHALL record an event capturing the new Entity_Id, the Owner user id, the Organization_Name, and the creation timestamp.
2. THE Audit_Log SHALL scope each tenant-creation event to the new tenant's Entity_Id.
3. WHEN a tenant-creation audit event is recorded, THE Audit_Log SHALL NOT record the plaintext password or the Argon2id password hash.

### Requirement 12: Tenant Naming and Identification

**User Story:** As an Owner, I want my organization to have a stored name, so that the tenant is identifiable in the UI and in administrative views.

#### Acceptance Criteria

1. WHEN a tenant is provisioned, THE Tenant_Provisioner SHALL persist the supplied Organization_Name as part of the tenant's stored configuration.
2. THE Tenant_Provisioner SHALL identify each tenant by its Entity_Id as the stable unique key.
3. WHERE two tenants supply the same Organization_Name, THE Tenant_Provisioner SHALL still create distinct tenants with distinct Entity_Id values.

### Requirement 13: First Owner Protection

**User Story:** As an organization, I want the first Owner to remain present, so that a tenant can never be left without an Owner who can administer it.

#### Acceptance Criteria

1. WHILE a tenant has exactly one active Owner, THE Invite_Service SHALL reject any request that would remove or deactivate that Owner.
2. WHILE a tenant has exactly one active Owner, THE Invite_Service SHALL reject any request that would change that Owner's role to a non-Owner role.
3. THE Tenant_Provisioner SHALL assign the first user created at signup the role Owner.

### Requirement 14: Abandoned and Partial Signups

**User Story:** As a platform operator, I want incomplete signups to leave no usable tenant, so that abandoned attempts do not accumulate as half-created organizations.

#### Acceptance Criteria

1. IF a signup transaction does not commit, THEN THE Tenant_Provisioner SHALL leave no Entity_Settings row, Owner user, or Chart_Of_Accounts rows for the attempted Entity_Id.
2. WHEN a signup fails after generating a candidate Entity_Id, THE Tenant_Provisioner SHALL ensure that candidate Entity_Id is not referenced by any persisted tenant data.
3. IF the same client retries a failed signup, THEN THE Signup_Service SHALL process the retry as a new signup attempt without reusing data from the failed attempt.
