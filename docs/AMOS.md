# Amos — Program Reference

Amos is Zavora ERA's agentic layer: a realtime **voice + chat AI accountant**
for non-accountant business owners. Talk or type; Amos plans the work as a
visible task list, executes it against the live ledger through MCP tools,
drives a real browser through the ERP to **showcase** what it did, files
screenshot **evidence**, and **learns** across sessions. Every write to the
books is gated on the owner's explicit confirmation.

This document is the single reference for the whole Amos program: what exists,
how it fits together, and what's still planned. Setup lives in
[`amos/README.md`](../amos/README.md); operating rules in
[`amos/AGENTS.md`](../amos/AGENTS.md); changes in
[`CHANGELOG.md`](../CHANGELOG.md).

![Amos embedded in the Zavora ERP shell](assets/amos-embedded.png)

---

## 1. Architecture

```
Browser — ERP /amos page (iframe, mic-enabled) or standalone :8090
  │  WebSocket /ws  — binary = PCM audio both ways; JSON = chat, tasks,
  │                   evidence, skills, memory, routines
  ▼
amos crate (Rust, axum, standalone cargo workspace) ── RealtimeRunner ── Gemini Live
  │      tools bridged into the realtime session:
  ├── interactive MCP manager (secret-minimized environment)
  │     ├── mcp-erp delegated per verified session user → ERP API :8080
  │     └── @playwright/mcp → headed/headless Chrome → ERP UI :3000
  ├── ERP-only routine MCP manager (separate service identity)
  ├── native tools: plan_tasks · update_task · use_skill · erp_login ·
  │                 showcase_step · remember · recall · forget ·
  │                 analyze_attachment · web_search · current_datetime ·
  │                 ops_status · run_routine
  ├── AmosMemory (pgvector) ── Gemini embeddings ── Postgres (shared ERP DB)
  ├── SessionHistory (amos_sessions) — persisted transcripts
  └── Ambient Ops scheduler ── routines/*.toml ── one-shot sub-agents
        (Gemini Flash, adk-agent/adk-runner) ── amos_runs ops ledger
        triggers: cron · run_routine tool · Run-now button · ERP webhook
```

- **Runtime**: `adk-realtime` `RealtimeRunner` on Gemini Live (native audio).
  Mic PCM in at 16 kHz, model audio out at 24 kHz, live transcripts both ways;
  typing works mid-voice-session.
- **Deployment**: a standalone container (`amos/Dockerfile`) proxied by Caddy at
  `/amos-app/*`, same-origin with the ERP so the embedded iframe inherits mic
  permission. See §7.
- **Why a separate crate/workspace**: Amos path-depends on `../../adk-rust`,
  which can't exist in the ERP's Docker build context — so `amos` is its own
  cargo workspace and is *not* a member of the ERP workspace. Build it with
  `cd amos && cargo run`, never `cargo run -p amos` from the repo root.

---

## 2. Capabilities (what's built)

### 2.1 ERP toolset (mcp-erp, zavora backend)
A `zavora` backend was added to the shared `mcp-erp` server. Interactive calls
use caller delegation with no service-account fallback; a separate ERP-only
manager retains automatic service-token refresh for unattended routines. Amos
uses a filtered set of 70+ tools covering the full
accountant job: dashboard + **27 report types**, customers/vendors/products
(incl. masters), **AR invoices** (draft → VAT-verified post → **eTIMS
transmit/retry**), **AP bills**, **payments** (Kenyan WHT, director funding),
**manual journals**, **payroll** end-to-end, the **procurement (P2P)** suite,
**inventory** (levels/adjust/transfer), **bank reconciliation**
(compute → tick → complete-and-lock), **period close/reopen**, and
**statutory tax filings** (VAT/PAYE/WHT: report → file → remit). The backend's
own docs: the [mcp-erp repository](https://github.com/zavora-ai/mcp-erp).

### 2.2 Skills — teachable playbooks (agentskills.io standard)
Drop-in `SKILL.md` files under `amos/skills/` teach Amos consistent multi-step
procedures. **Progressive disclosure**: the system prompt carries a one-line
catalog per skill; the `use_skill` tool pulls a playbook's full body on demand.
A skill's `allowed-tools` also extends the MCP tool allowlist. Ships thirteen
(a regression test pins the pack — a malformed skill can't silently drop its
tools):

| Skill | Teaches |
|---|---|
| `record-vendor-bill` | AP: vendor lookup → duplicate check → FCY-correct draft → verify gross vs source → confirm → post → evidence |
| `record-customer-invoice` | AR: customer/product match → VAT-verified gross → confirm → post → **eTIMS transmission check + retry** |
| `record-payment` | Receipts & vendor payments, applications, KES-denominated WHT, director funding |
| `financial-reporting` | Which `run_report` type + params, trial-balance integrity, plain-language translation |
| `manual-journal` | Balanced entries, account verification, reversal-based corrections |
| `bank-reconciliation` | Compute → match → itemise every shilling of difference → complete-and-lock; never plug |
| `inventory-ops` | Stock levels, reasoned adjustments/write-offs, transfers, product/vendor masters, valuation impact |
| `tax-filing` | KRA calendar (VAT & WHT 20th, PAYE 9th), ledger-sourced figures, file + remit recording |
| `month-end-review` | Close ritual: TB proof, recon + filing status, unposted drafts, period close/reopen |
| `manage-procurement` | Requisition → approval → LPO → goods receipt → 3-way match → debit notes → expense claims |
| `hr-payroll` | Run/review/commit pay runs, adjustments, effective-dated statutory rates, plain-language PAYE/NSSF/SHA/Housing/HELB |
| `crm` | Flag-gated pipeline: leads, deals, activities, portal invites |
| `erp-showcase` | Browser evidence: snapshot-before-acting, the ERP route map, retry ladder |

`system.md` also carries a **coverage map** telling Amos what it handles
natively vs what still lives only in the ERP UI (AR credit notes, estimates,
asset/FX runs, budgets setup, POS shifts) so it routes honestly.

### 2.3 Showcase — browser evidence
`browser_navigate` to the ERP signs Amos in automatically (deterministic login
wrapped around the tool; the model never touches the login form). It navigates
the real UI, verifies content with snapshots, and files screenshot **evidence
cards** into the UI panel via `showcase_step`. Runs headed on dev machines,
headless in the container.

### 2.4 Memory — learning across sessions
Semantic long-term memory (adk-memory `PostgresMemoryService` + pgvector cosine
search + Gemini `gemini-embedding-001` at 768 dims), sharing the ERP database.

| Kind | What | How it's used |
|---|---|---|
| `profile` | Business facts & owner preferences | Injected into every session's prompt |
| `lesson` | Workflow gotchas, scoped per skill | Appended to a playbook when `use_skill` loads it |
| `session` | End-of-session summaries | Latest one rides in the prompt for continuity |

**The learning loop**: profile block injected at session start · `use_skill`
appends "Lessons learned from previous runs" · **failed workplan tasks auto-file
lessons** under the active skill · a **session-close distiller** (Gemini
`generateContent`) extracts durable knowledge from each transcript. Write via the
`remember` tool or these automatic paths; read via prompt injection, per-skill
enrichment, and the `recall` tool. Surfaced in the UI Memory panel and
`GET /api/memories`. Graceful in-memory fallback if the DB is unreachable.

**Hygiene (survives months of use)**: near-duplicate memories are skipped on
store (cosine ≥ 0.9 via pgvector; exact-normalized match on the in-memory
fallback, so keyword overlap can't false-dedup); a **`forget` tool** and a
per-row delete in the Memory panel make wrong facts correctable; the panel
lists by true recency (adk-memory `list_recent`), and the "last session" prompt
line is a real latest-lookup rather than a similarity guess.

**Session history**: on close, each transcript persists to `amos_sessions`
(best-effort, like the audit sink) — the dialogue in which a posting was
approved is compliance material, not throwaway. A **Past sessions** panel lists
them with a full-transcript viewer (`GET /api/sessions`).

### 2.5 UI & workflow contract
The web app (`amos/assets/index.html`) shows a live business snapshot (cash,
AR/AP, overdue, bank balances from the ledger), a **workplan** panel, **evidence**
cards, a **memory** panel (with per-row forget), **past sessions**, a
**routines** panel (schedule, last outcome, Run-now, pause/resume), and a
timestamped **activity** trail with "Posting" badges on ledger writes. Replies
render as sanitized markdown (tables, lists, code) streamed per chunk;
entity-aware openers and **contextual follow-up chips** are generated after
each answer. Workplan/evidence are **per-session** — concurrent sessions never
see each other's plans. Embedded in the ERP shell at `/amos` (sidebar button),
so the left nav and top branding stay consistent; `?embed=1` hides Amos's own
header. The workflow contract (restate → plan → confirm-before-write → execute
task-by-task → showcase → summarize) lives in `system.md` + `AGENTS.md`.

### 2.6 Specialist sub-agents
Two one-shot Gemini 3 Flash specialists run off the realtime loop:
`analyze_attachment` reads PDFs/images attached via the paperclip (invoices,
receipts, statements; context-cached so repeat questions about the same
document are cheap) and `web_search` answers external questions (KRA/CBK rates,
FX, suppliers) with Google-grounded, cited results. A **session clock** grounds
"today" and default posting dates in the user's timezone + work-as-of date, and
**plan tiers** gate voice and web-search by subscription (text-only sessions
force text modality so no billable audio is ever generated).

### 2.7 Ambient operations — the practice calendar
Amos runs the recurring accounting work on a schedule and reacts to events
(`amos/routines/*.toml`; see `amos/README.md` for the spec format):

| Routine | When | Does |
|---|---|---|
| `morning-briefing` | daily 07:00 | cash & bank, due/overdue today, health note |
| `etims-sweep` | daily 18:00 | untransmitted posted invoices → retry → ⚠ report |
| `recon-nudge` | Fri 16:00 | stale/unreconciled bank accounts |
| `ar-chase` | Mon 08:00 | overdue customers, invoices to chase, suggested action |
| `paye-prep` | 5th | last month's PAYE ahead of the 9th |
| `vat-prep` | 14th | last month's VAT return ahead of the 20th |
| `month-end-pack` | 3rd | TB proof, P&L/BS, recon/VAT/draft close checklist |

Each fire spawns a **one-shot sub-agent** (Gemini Flash text agent,
adk-agent/adk-runner) over the routine's declared tool subset, wrapped in the
same scope checks + audit trail as live sessions — never the browser, and write
scopes only where the spec opts in (the eTIMS retry is the only shipped write).
Results land in the **`amos_runs` ops ledger**, the ERP's **in-app notification
inbox**, and a live `routine_done` push. The live Amos knows the calendar
(`{ops}` prompt block + `ops_status`), triggers on demand (`run_routine`, the
panel's Run-now), and can pause/resume a schedule. **Reactive triggers**:
`POST /api/ops/run/{name}` accepts `{"context": ...}` — the ERP fires it
automatically when an eTIMS auto-transmit fails, so the sweep retries within
minutes instead of at 18:00 (`AMOS_WEBHOOK_URL`/`AMOS_WEBHOOK_SECRET`; the
secret opens exactly the trigger route, nothing that reads data). Prep routines
end at a report — posting, filing and closing stay human decisions made with
the live Amos (the approval gate is conversational, by design).

---

## 3. Configuration surface (edit files, restart — no recompiling)

| File | Controls | Env override |
|---|---|---|
| `amos/system.md` | System prompt template (`{ui_url}`, `{skills_catalog}`, `{agents_rules}`, `{memories}`) | `AMOS_SYSTEM_MD` |
| `amos/AGENTS.md` | Operating rules: financial + memory guardrails, skill protocol, escalation | `AMOS_AGENTS_MD` |
| `amos/mcp.json` | MCP servers (Kiro format, `${VAR}` expansion, secret-free) | `AMOS_MCP_JSON` |
| `amos/skills/` | Skill packs | `AMOS_SKILLS_DIR` |
| `amos/routines/` | Ambient-ops routine specs (cron, skill, tools, scopes, prompt) | `AMOS_ROUTINES_DIR` |

Secrets live only in gitignored `amos/.env` (prod: `deploy/.env.prod`, synced
from GitHub Actions secrets). Full env reference in `amos/README.md`.

---

## 4. HTTP & WebSocket surface

Every `/api/*` route and `/showcase/*` requires the user's ERP JWT
(`Authorization: Bearer …`, or `?token=` for image loads); `/` and `/ws` are
open (the WS runs its own auth-first handshake).

| Endpoint | Purpose |
|---|---|
| `GET /` | Amos web app (`?embed=1` for the ERP iframe) |
| `GET /health` | Unauthenticated liveness (deploy probes) |
| `GET /ws` | Realtime session (audio + JSON control) |
| `GET /api/snapshot` | Live business snapshot from the ledger |
| `GET /api/skills` · `/api/context` | Skills catalog · entity context |
| `GET /api/memories` · `DELETE /api/memories/forget?query=` | Memory listing (newest first) · correction |
| `GET /api/sessions` · `/api/sessions/{id}` | Past-session list · full transcript |
| `GET /api/ops` | Routine schedule + recent runs |
| `POST /api/ops/run/{name}` | Trigger a routine (optional `{"context"}`; user JWT or `X-Amos-Webhook-Secret`) |
| `PATCH /api/ops/routines/{name}` | Pause/resume a routine's schedule |
| `GET /showcase/<file>` | Evidence screenshots |

WS server→client message types: `connected`, `text_delta`, `transcript`,
`input_transcript`, `speech_started/stopped`, `response_done`, `tool_call`,
`tasks`, `showcase`, `skill`, `memory`, `memory_removed`, `suggestions`,
`routine_done`, `notice`, `error`. Binary frames = PCM audio.

---

## 5. Upstream adk-rust changes

Amos surfaced two framework bugs, both fixed on the pinned
`fix/gemini-batched-tool-calls` branch (which `amos/Dockerfile` builds from):

1. **Batched Gemini tool calls dropped** — the Gemini Live translator emitted
   only the first function call of a parallel batch, stalling sessions until the
   server aborted them. Now emits every call.
2. **`PostgresMemoryService::add_entry` unimplemented** — global (project-less)
   single-entry writes returned "not implemented" even though session and
   project writes worked. Now implemented.

Both are worth upstreaming to `adk-rust` `main` (they affect any Gemini Live /
pgvector-memory consumer, including the mia example).

---

## 5b. Security & tenant isolation

**One Amos = one tenant, enforced.** Each deployment serves exactly one entity
and refuses every session that isn't for it.

- **Identity gate** (`amos/src/auth.rs`, `routes.rs`): the embedded ERP page
  hands the user's access token to the iframe via `postMessage`; the iframe
  sends it as the first WebSocket frame. Amos verifies the token with the shared
  `JWT_ACCESS_SECRET` (same secret as the API) — signature, expiry, type, issuer
  — and requires `entity_id == the served entity`. A wrong-entity token, a
  forged/expired token, or no token ⇒ the session is **refused before the runner
  is built** (no tools, data, memory, or showcase). The served entity is derived
  at startup from the service account's own tenant (`AMOS_SERVED_ENTITY_ID`
  overrides). Dev standalone only: `AMOS_DEV_ALLOW_UNAUTH=1` permits an
  unauthenticated local session (never set in production).
- **Role scoping** (`amos/src/scope.rs`): the principal's ERP role grants scopes
  (`erp:read`/`erp:write`/`ledger:post`, mirroring the API's role gates). Every
  ERP/browser tool is wrapped so its required scope is checked before it runs —
  a Viewer's session cannot post to the ledger no matter what the model or a
  malicious prompt attempts.
- **Caller-bound MCP auth** (`amos/src/credential.rs`, `mcp.rs`): interactive
  calls carry only an opaque path to a private, per-session bearer file. Raw
  JWTs never enter MCP arguments. The interactive mcp-erp child has no service
  credentials and cannot fall back when a token is missing, expired, or
  rejected. Unattended routines use a separate ERP-only process.
- **Child-process secret isolation**: interactive MCP children start with a
  minimal environment, excluding the Gemini key, JWT signing secret, database
  URLs, and ERP service password. This applies to Playwright as well as mcp-erp.
- **Prompt-injection guardrails** (`amos/src/guard.rs`): inbound user turns are
  screened for instruction-override and secret/cross-tenant-exfiltration before
  reaching the model; a hit is refused, not forwarded. The `remember` tool
  rejects secret-shaped content.
- **Audit trail** (`amos/src/audit.rs`): session accept/deny and every tool
  access (allowed/denied) are written to `amos_audit_events` (its own table,
  separate from the ERP's `audit_events`), keyed by entity + user + session.
- **Memory** is keyed by the served entity id — a different deployment shares
  none of it.
- **REST surface auth**: every `/api/*` and `/showcase/*` route is behind the
  same token verification as the WebSocket (see §4) — panel data, memories and
  evidence screenshots are never readable unauthenticated. The only exception
  is the webhook secret, which opens exactly the routine-trigger route.
- **Honest tenant gate**: a valid user whose organisation this Amos doesn't
  serve gets a typed `wrong_tenant` refusal, and the UI shows "Amos is being
  set up for your workspace" instead of a dead "Offline".

To add a tenant: run another Amos instance with that tenant's service account
(and `AMOS_SERVED_ENTITY_ID`) — a completely separate system, as intended.

## 6. Known constraints & operational notes

- **One Amos serves one tenant** (see §5b). Interactive ERP calls are already
  bound to each verified user, but per-entity memory, evidence, and deployment
  isolation remain intentionally single-tenant; each tenant gets its own
  instance.
- **Gemini Live tool-count sensitivity** — the tool set is filtered
  aggressively; large sets degrade the model.
- **15-min JWT TTL** — the ERP shell refreshes the verified interactive token;
  the separate routine service client re-authenticates independently.
- **Process hygiene** — stop Amos with SIGTERM/Ctrl-C so it reaps MCP children
  (mcp-erp, Playwright, Chromium); SIGKILL leaks them.
- **sqlx pin** — `amos/Cargo.lock` pins sqlx 0.8.6; pgvector's version range
  otherwise resolves 0.9 and splits the sqlx trait impls.
- **Embedding latency** — ~100–300 ms per remember/recall; all writes are
  spawned off the realtime event loop.

---

## 7. Production deployment

Amos deploys with the ERP stack on every merge to `main`
(`.github/workflows/deploy.yml`).

- **Image** (`amos/Dockerfile`): clones pinned `adk-rust` and `mcp-erp` refs,
  builds both binaries on Rust 1.95, bakes Node + a
  pinned `@playwright/mcp` + headless Chromium.
- **Routing**: Caddy proxies `/amos-app/*` → `amos:8090` (prefix stripped; the
  frontend auto-detects it). Same-origin with the ERP → the iframe inherits mic
  permission, the CDN needs no changes, WebSockets proxy through.
- **Data**: memory shares the ERP Postgres (`pgvector/pgvector:pg17` image;
  `AMOS_MEMORY_DATABASE_URL`).
- **Secrets** (GitHub Actions → `deploy/.env.prod`): `GOOGLE_API_KEY`,
  `AMOS_ERP_EMAIL`, `AMOS_ERP_PASSWORD`, optional `AMOS_ERP_LOGIN_*`.
- **Health check**: the deploy waits on `amos:8090/api/skills` alongside the API.

Live at `https://erp.zavora.ai/amos-app/`.

---

## 8. Planned / pending work

### 8.1 Security & tenant isolation — ✅ done (2026-07-05)
Implemented — see §5b. Identity gate (verified JWT + served-entity binding),
role scoping, prompt-injection guardrails, entity-keyed memory, and an audit
trail. Remaining boundary: a single shared multi-tenant Amos (§8.4) if SaaS
multi-tenancy is wanted later.

### 8.3 Robustness & continuity — **P2**
- **Session resumption** when Gemini drops the ~15-min socket (workplan and
  evidence already survive; re-attach instead of cold-restart).
- **Skill success metrics** — wire `use_skill` outcomes to the registry's
  success-criteria for an audit view of playbook adherence.
- ~~**Memory hygiene**~~ — ✅ done (2026-07-08): dedup-on-store, `forget`
  tool + UI delete, true-recency listing. Remaining: a `delete_user` admin
  path (not a model tool) for GDPR-style wipe.

### 8.4 First real job (bookkeeping)
Not an Amos-platform task, but queued: the month-by-month re-recording of the
2025 vendor bills (roll back the 12 Google bills, re-record chronologically
across all vendors) is a natural first production assignment for Amos.
