# Backup & Restore Runbook — Zavora ERP

**Audience:** whoever operates the production deployment.
**Scope:** what to back up, how to take and verify backups, how to restore, and
how to review a migration before it runs against live data.

> **Why this matters.** The ledger is the business. A posted journal entry is
> immutable by DB trigger (`migrations/002`, `026`) — which means a *bad* or
> *lost* database cannot be "edited back" into shape. The only safety net is a
> **known-good, restorable backup** plus a **reviewed migration path**. This
> runbook is that net. Treat an untested backup as no backup.

---

## 1. What has to be backed up

Everything lives in **one PostgreSQL database** (`zavora_era`, Postgres 17 +
`pgvector`). The container is `zavora-postgres`; the data volume is `pgdata`.
A single logical dump of that database captures the whole system of record.

| Data | Where | Back up? | Notes |
|---|---|---|---|
| **ERP ledger + masters** | `zavora_era` public schema (~119 tables: accounts, journal_lines, invoices, bills, payments, payroll, inventory, warehouses, …) | **Yes — critical** | The system of record. RPO target near-zero. |
| **Amos agent state** | same DB: `amos_sessions`, `amos_runs`, `amos_audit_events` | **Yes** | Session transcripts + the agent audit trail (who confirmed which posting). Needed for compliance/forensics. |
| **Amos semantic memory** | same DB: `memory_entries` (pgvector embeddings), `_adk_memory_migrations` | **Yes** | Learned profile/lessons. Loss = Amos forgets; not ledger-critical but painful. |
| **Platform / ops plane** | same DB: `platform_users`, `tenants`, `platform_audit_events` | **Yes (in the dump)** | Multi-tenant operator console. |
| **pgvector extension** | `CREATE EXTENSION vector` (v0.8.4) | Provisioned by image | The `pgvector/pgvector:pg17` image ships it; a plain `postgres:17` image will **fail to restore** the `memory_entries` vector column. Restore onto the same image. |
| **Redis** | `zavora-redis` (sessions, audit stream, signup rate-limit, caches) | **No (rebuildable)** | Ephemeral. Losing it logs users out and drops the in-flight audit stream tail; it does **not** lose committed ledger data (the audit *trail* is in Postgres). Do not treat Redis as a backup target. |
| **Object/showcase files** | `AMOS_SHOWCASE_DIR` screenshots; any receipt/OCR uploads on disk | Optional | Evidence cards + uploaded receipts if stored on the filesystem rather than re-derivable. Snapshot the volume if you rely on them. |
| **Secrets** | `.env.prod` (`JWT_*`, `PAYSTACK_SECRET_KEY`, `MPESA_*`, DB password, `GOOGLE_API_KEY`) | **Yes — separately** | Store in a managed secret store, **not** in the DB dump. A restore is useless if you can't decrypt sessions / re-auth integrations. |

**Bottom line:** one `pg_dump` of `zavora_era` + your secret store + (optionally)
the showcase/uploads volume = a complete, restorable system.

---

## 2. Taking a backup

Use **custom format** (`-Fc`): compressed, and restorable selectively with
`pg_restore`. Dump *as* a superuser role that owns all objects (`zavora`).

### 2.1 One-off / manual (local dev on port 5433)

```bash
# Dump straight out of the running container to a timestamped file on the host.
docker exec zavora-postgres \
  pg_dump -U zavora -d zavora_era -Fc \
  > "zavora_era_$(date +%Y%m%d_%H%M%S).dump"
```

Or via the host `psql`/`pg_dump` client against the published port:

```bash
PGPASSWORD=zavora pg_dump -h localhost -p 5433 -U zavora -d zavora_era -Fc \
  -f "zavora_era_$(date +%Y%m%d_%H%M%S).dump"
```

### 2.2 Production (docker-compose.prod.yml)

The prod DB is not port-published to the host; exec into the container:

```bash
# Uses the env already set inside the container.
docker exec zavora-postgres sh -c \
  'pg_dump -U "$POSTGRES_USER" -d "$POSTGRES_DB" -Fc' \
  > "/backups/zavora_era_$(date +%Y%m%d_%H%M%S).dump"
```

> **Consistency.** `pg_dump` runs in a single transaction snapshot, so the dump
> is internally consistent even while the app is live — no downtime needed. You
> do **not** need to stop the API to take a backup.

### 2.3 Scheduled (nightly), with retention

`pg_dump` is safe to run against a live database. Schedule it and prune old
copies. Example `cron` (host) keeping 14 daily + pushing off-box:

```bash
# /etc/cron.d/zavora-backup  — 02:15 daily
15 2 * * * root /usr/local/bin/zavora-backup.sh >> /var/log/zavora-backup.log 2>&1
```

```bash
#!/usr/bin/env bash
# /usr/local/bin/zavora-backup.sh
set -euo pipefail
BACKUP_DIR=/backups
STAMP=$(date +%Y%m%d_%H%M%S)
FILE="$BACKUP_DIR/zavora_era_${STAMP}.dump"

mkdir -p "$BACKUP_DIR"
docker exec zavora-postgres sh -c \
  'pg_dump -U "$POSTGRES_USER" -d "$POSTGRES_DB" -Fc' > "$FILE"

# Fail loudly if the dump is suspiciously small (empty/failed dump).
test "$(stat -f%z "$FILE" 2>/dev/null || stat -c%s "$FILE")" -gt 100000 \
  || { echo "BACKUP TOO SMALL — investigate"; exit 1; }

# Off-site copy (encrypt in transit + at rest). Example S3:
aws s3 cp "$FILE" "s3://zavora-backups/db/" --sse aws:kms

# Retention: keep 14 local daily dumps.
ls -1t "$BACKUP_DIR"/zavora_era_*.dump | tail -n +15 | xargs -r rm -f
```

**RPO/RTO.** Nightly dumps give a worst-case **RPO of ~24h**. For a tighter RPO,
add PostgreSQL **WAL archiving / PITR** (`archive_command` + base backups) so you
can roll forward to any point in time; the logical-dump procedure above remains
the simplest floor and the basis for the restore drill.

**Off-site rule.** A backup that lives only on the same host/volume as the
database does not survive the failure that matters (disk loss, volume delete).
Always ship a copy off the box, encrypted.

---

## 3. Restoring

### 3.1 Restore into a fresh database (the safe default)

Never restore over a live database you might still need. Restore into a new
database, verify, then cut over.

```bash
# 1. Create an empty target DB (on the same pgvector image!).
docker exec zavora-postgres psql -U zavora -d postgres \
  -c "CREATE DATABASE zavora_restore;"

# 2. Copy the dump into the container, then restore from the FILE.
#    (Parallel restore `-j` reads a file — it is NOT supported from stdin.)
#    --no-owner avoids role-mismatch noise; pgvector's CREATE EXTENSION replays
#    from the dump (the image already provides the .so).
docker cp zavora_era_20260710_020000.dump zavora-postgres:/tmp/restore.dump
docker exec zavora-postgres \
  pg_restore -U zavora -d zavora_restore --no-owner --clean --if-exists -j 4 /tmp/restore.dump
docker exec zavora-postgres rm -f /tmp/restore.dump

# (Piped alternative, single-threaded — drop `-j` when reading from stdin:)
#   cat zavora_era_20260710_020000.dump | docker exec -i zavora-postgres \
#     pg_restore -U zavora -d zavora_restore --no-owner --clean --if-exists

# 3. Verify (see §4) BEFORE pointing the app at it.
```

### 3.2 Full disaster recovery (rebuild from nothing)

```bash
# 1. Bring up just Postgres from the prod compose.
docker compose -f docker-compose.prod.yml up -d postgres
# 2. Restore into the app database (copy in, then file-based parallel restore).
docker cp /backups/zavora_era_LATEST.dump zavora-postgres:/tmp/restore.dump
docker exec zavora-postgres sh -c \
  'pg_restore -U "$POSTGRES_USER" -d "$POSTGRES_DB" --no-owner --clean --if-exists -j 4 /tmp/restore.dump'
docker exec zavora-postgres rm -f /tmp/restore.dump
# 3. Restore secrets (.env.prod) from the secret store.
# 4. Start the rest of the stack. Migrations that are already applied are
#    no-ops (idempotent tracking); newer ones apply on API boot.
docker compose -f docker-compose.prod.yml up -d
```

> **Extension gotcha.** If restore errors with `type "vector" does not exist`,
> the target image lacks pgvector. Use `pgvector/pgvector:pg17` (as both compose
> files already do) — do **not** restore onto a stock `postgres` image.

---

## 4. Verify every backup (a dump you haven't restored is a rumour)

Run this after taking a backup, and as a monthly **restore drill**. It restores
into a throwaway DB and sanity-checks it, then drops it.

```bash
#!/usr/bin/env bash
# verify-backup.sh <dumpfile>
set -euo pipefail
DUMP="$1"; TMP=zavora_verify_$$

docker exec zavora-postgres psql -U zavora -d postgres -c "CREATE DATABASE $TMP;"
trap 'docker exec zavora-postgres psql -U zavora -d postgres -c "DROP DATABASE IF EXISTS $TMP;"; docker exec zavora-postgres rm -f /tmp/verify.dump' EXIT

docker cp "$DUMP" zavora-postgres:/tmp/verify.dump
docker exec zavora-postgres \
  pg_restore -U zavora -d "$TMP" --no-owner --clean --if-exists -j 4 /tmp/verify.dump

# Table count should match live (±expected drift).
docker exec zavora-postgres psql -U zavora -d "$TMP" -t \
  -c "SELECT count(*) AS tables FROM pg_tables WHERE schemaname='public';"

# Spot-check the load-bearing tables exist and have rows.
docker exec zavora-postgres psql -U zavora -d "$TMP" -t -c "
  SELECT 'journal_lines', count(*) FROM journal_lines
  UNION ALL SELECT 'invoices', count(*) FROM invoices
  UNION ALL SELECT 'amos_audit_events', count(*) FROM amos_audit_events
  UNION ALL SELECT 'memory_entries', count(*) FROM memory_entries;"

echo "✅ restore verified from $DUMP"
```

A backup is "good" only once this script has exited 0 against it.

---

## 5. Migration safety review

Migrations in `migrations/` **auto-apply on API startup** (see README "migrations
auto-apply"). That is convenient and dangerous: a deploy can alter the ledger
schema the moment it boots. Gate every migration through this checklist.

### 5.1 Before merging a migration

- [ ] **Take a fresh backup first** if the target already holds real data
      (§2) — this is the rollback plan.
- [ ] **Read the SQL for destructive verbs:** `DROP TABLE`, `DROP COLUMN`,
      `TRUNCATE`, `ALTER … TYPE` (rewrites), `DELETE`/`UPDATE` without a tight
      `WHERE`, `NOT NULL` added to an existing column without a backfill+default.
      Grep the new file:

      ```bash
      grep -niE 'drop |truncate|delete |alter .*type|not null' migrations/0XX_*.sql
      ```

- [ ] **Additive-first.** Prefer add-column/add-table + backfill over rename or
      drop. The warehousing migration (`060`) is the pattern: new tables +
      **backfill** existing data (a default warehouse per entity, seeded from
      `on_hand`) — nothing dropped, invariant preserved.
- [ ] **Backfill preserves invariants.** If a column derives from existing data,
      the migration must populate it so aggregate checks still hold (e.g.
      `SUM(warehouse_stock.quantity) == inventory_items.on_hand`).
- [ ] **Immutability triggers unaffected.** Confirm you are not adding writes to
      posted journal entries or hard-closed periods (`migrations/002`, `026`
      enforce these at the DB and will reject the migration or later writes).
- [ ] **Idempotent / re-runnable** where feasible (`IF NOT EXISTS`,
      `CREATE OR REPLACE`), so a half-applied deploy can be retried.
- [ ] **Test on a restored copy of production**, not just an empty dev DB:

      ```bash
      # Restore prod into a scratch DB (see §3.1), then dry-run the migration.
      docker exec -i zavora-postgres psql -U zavora -d zavora_restore \
        < migrations/0XX_new.sql
      ```

### 5.2 If a migration must be destructive

Destructive schema change (dropping/rewriting a column that holds data) is a
**high-risk, hard-to-reverse** operation:

1. Announce a maintenance window; take a backup **immediately before**.
2. Apply during low traffic; keep the pre-migration dump for the retention
   window at minimum.
3. Have the **restore command staged** (§3) before you start.
4. Prefer a two-step deploy: (a) ship the additive change + backfill, (b) in a
   later release, once verified, remove the old column.

---

## 6. Quick reference

```bash
# Backup (prod)
docker exec zavora-postgres sh -c 'pg_dump -U "$POSTGRES_USER" -d "$POSTGRES_DB" -Fc' > backup.dump

# Verify (throwaway restore)
./verify-backup.sh backup.dump

# Restore into a fresh DB
docker exec zavora-postgres psql -U zavora -d postgres -c "CREATE DATABASE zavora_restore;"
docker cp backup.dump zavora-postgres:/tmp/restore.dump
docker exec zavora-postgres pg_restore -U zavora -d zavora_restore --no-owner --clean --if-exists -j 4 /tmp/restore.dump
```

- **Image:** always `pgvector/pgvector:pg17` (both compose files) — needed for `memory_entries`.
- **Redis:** not a backup target (ephemeral); the durable audit trail is in Postgres.
- **Secrets:** back up `.env.prod` separately in a managed store.
- **Golden rule:** a backup counts only after §4 has restored it clean.
