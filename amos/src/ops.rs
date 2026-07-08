//! Ambient operations — Amos's practice calendar.
//!
//! Routines (`routines/*.toml`) are scheduled accounting jobs run by one-shot,
//! NON-realtime sub-agents (Gemini Flash text agents): a cron fires (or the
//! user triggers manually), the sub-agent runs the routine's playbook against
//! the same scoped + audited ERP toolset the live session uses, and the result
//! lands in the ops ledger (`amos_runs`), the in-app notification inbox, and a
//! live push to any open session. The live Amos reads the ledger + schedule, so
//! it always knows what ran, what's due, and what failed.
//!
//! Safety: routines are read-only unless their spec grants write scopes, they
//! never drive the browser, and Skip concurrency means a slow run never
//! double-fires. Anything requiring judgement stops at a report — posting and
//! closing stay with the human in the live session.

use crate::state::AppState;
use anyhow::{Result, anyhow};
use chrono::Utc;
use serde::Deserialize;
use sqlx::{PgPool, Row};
use std::collections::HashSet;
use std::str::FromStr;
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::{info, warn};

/// One scheduled routine, loaded from `routines/<name>.toml`.
#[derive(Debug, Clone, Deserialize)]
pub struct RoutineSpec {
    pub name: String,
    pub title: String,
    /// 6-field cron (sec min hour dom month dow), evaluated in the business tz.
    pub cron: String,
    /// Optional skill whose playbook (and allowed-tools) the sub-agent runs with.
    #[serde(default)]
    pub skill: Option<String>,
    /// Explicit extra tools (beyond the skill's) the sub-agent may call.
    #[serde(default)]
    pub tools: Vec<String>,
    /// Scopes granted to the run (e.g. ["erp:read"]). Write scopes are opt-in.
    #[serde(default)]
    pub scopes: Vec<String>,
    /// The sub-agent's task, appended after the playbook.
    pub prompt: String,
    /// Deliver the result to the ERP's in-app notification inbox.
    #[serde(default)]
    pub notify: bool,
    #[serde(default = "default_true")]
    pub enabled: bool,
}

fn default_true() -> bool {
    true
}

pub struct Ops {
    routines: Vec<RoutineSpec>,
    tz: chrono_tz::Tz,
    /// Ops ledger + notification inbox (the shared ERP database). Best-effort:
    /// without it, routines still run and push live, they just aren't recorded.
    pool: Option<PgPool>,
    entity: String,
    /// Skip-concurrency guard: a routine never runs twice at once.
    running: Mutex<HashSet<String>>,
    /// Runtime pause overlay: paused routines skip their cron (manual runs
    /// still work — pausing expresses "stop the schedule", not "forbid it").
    paused: tokio::sync::RwLock<HashSet<String>>,
}

impl Ops {
    /// Load the routine registry and connect the ledger. Returns `None` when no
    /// routines are configured (ambient ops fully off).
    pub async fn init(entity: uuid::Uuid) -> Option<Arc<Self>> {
        let dir = std::env::var("AMOS_ROUTINES_DIR")
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|_| std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("routines"));
        let mut routines = Vec::new();
        for entry in std::fs::read_dir(&dir).ok()?.flatten() {
            let path = entry.path();
            if path.extension().is_none_or(|e| e != "toml") {
                continue;
            }
            match std::fs::read_to_string(&path).map_err(anyhow::Error::from).and_then(|s| Ok(toml::from_str::<RoutineSpec>(&s)?)) {
                Ok(spec) => {
                    if cron::Schedule::from_str(&spec.cron).is_err() {
                        warn!("ops: routine {} has an invalid cron '{}' — skipped", spec.name, spec.cron);
                        continue;
                    }
                    routines.push(spec);
                }
                Err(e) => warn!("ops: failed to load {}: {e}", path.display()),
            }
        }
        if routines.is_empty() {
            return None;
        }
        routines.sort_by(|a, b| a.name.cmp(&b.name));

        let tz: chrono_tz::Tz = std::env::var("AMOS_DEFAULT_TIMEZONE")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(chrono_tz::Africa::Nairobi);

        let url = std::env::var("AMOS_MEMORY_DATABASE_URL")
            .or_else(|_| std::env::var("AMOS_AUDIT_DATABASE_URL"))
            .unwrap_or_else(|_| "postgres://zavora:zavora@localhost:5433/zavora_era".to_string());
        let pool = match PgPool::connect(&url).await {
            Ok(p) => {
                let ddl = r#"
                    CREATE TABLE IF NOT EXISTS amos_runs (
                        id          UUID PRIMARY KEY,
                        entity_id   TEXT NOT NULL,
                        routine     TEXT NOT NULL,
                        title       TEXT NOT NULL,
                        fired_by    TEXT NOT NULL,
                        status      TEXT NOT NULL,
                        started_at  TIMESTAMPTZ NOT NULL DEFAULT now(),
                        finished_at TIMESTAMPTZ,
                        summary     TEXT
                    );
                    CREATE INDEX IF NOT EXISTS idx_amos_runs_entity ON amos_runs (entity_id, started_at DESC);
                "#;
                match sqlx::raw_sql(ddl).execute(&p).await {
                    Ok(_) => Some(p),
                    Err(e) => {
                        warn!("ops: ledger setup failed ({e}); runs won't be recorded");
                        None
                    }
                }
            }
            Err(e) => {
                warn!("ops: db unavailable ({e}); runs won't be recorded");
                None
            }
        };

        info!(
            "🗓️ ambient ops: {} routine(s) loaded ({}), tz {}, ledger {}",
            routines.len(),
            routines.iter().map(|r| r.name.as_str()).collect::<Vec<_>>().join(", "),
            tz.name(),
            if pool.is_some() { "on" } else { "off" },
        );
        Some(Arc::new(Self {
            routines,
            tz,
            pool,
            entity: entity.to_string(),
            running: Mutex::new(HashSet::new()),
            paused: tokio::sync::RwLock::new(HashSet::new()),
        }))
    }

    /// Pause or resume a routine's schedule at runtime.
    pub async fn set_paused(&self, name: &str, paused: bool) -> Result<()> {
        if self.spec(name).is_none() {
            return Err(anyhow!("unknown routine '{name}'"));
        }
        let mut set = self.paused.write().await;
        if paused {
            set.insert(name.to_string());
        } else {
            set.remove(name);
        }
        info!("🗓️ ops: routine {name} {}", if paused { "paused" } else { "resumed" });
        Ok(())
    }

    fn spec(&self, name: &str) -> Option<&RoutineSpec> {
        self.routines.iter().find(|r| r.name == name)
    }

    /// Next scheduled firing of a routine in the business timezone.
    fn next_due(&self, spec: &RoutineSpec) -> Option<chrono::DateTime<chrono_tz::Tz>> {
        let schedule = cron::Schedule::from_str(&spec.cron).ok()?;
        schedule.upcoming(self.tz).next()
    }

    // ─── Awareness ───────────────────────────────────────────────────────────

    /// The system-prompt block: what's scheduled, when it next fires, and how
    /// the last runs went — so the live Amos answers "what's due / what ran?"
    /// from data.
    pub async fn prompt_block(&self) -> String {
        let mut lines = vec![];
        let paused = self.paused.read().await.clone();
        for spec in &self.routines {
            if !spec.enabled {
                continue;
            }
            if paused.contains(&spec.name) {
                lines.push(format!("- {} ({}): PAUSED", spec.title, spec.name));
                continue;
            }
            let due = self
                .next_due(spec)
                .map(|d| d.format("%a %d %b %H:%M").to_string())
                .unwrap_or_else(|| "unscheduled".into());
            lines.push(format!("- {} ({}): next due {}", spec.title, spec.name, due));
        }
        for run in self.recent_runs(5).await {
            let status = run["status"].as_str().unwrap_or("?");
            let mark = match status {
                "ok" => "✓",
                "running" => "…",
                _ => "✗",
            };
            lines.push(format!(
                "- last: {} {} at {} — {}",
                run["routine"].as_str().unwrap_or("?"),
                mark,
                run["started_at"].as_str().unwrap_or(""),
                run["summary"].as_str().unwrap_or("").chars().take(120).collect::<String>(),
            ));
        }
        if lines.is_empty() {
            "(no routines configured)".into()
        } else {
            lines.join("\n")
        }
    }

    /// Routine schedule + recent runs, for the `ops_status` tool and `/api/ops`.
    pub async fn status(&self) -> serde_json::Value {
        let paused = self.paused.read().await.clone();
        let routines: Vec<_> = self
            .routines
            .iter()
            .map(|spec| {
                serde_json::json!({
                    "name": spec.name,
                    "title": spec.title,
                    "cron": spec.cron,
                    "enabled": spec.enabled,
                    "paused": paused.contains(&spec.name),
                    "notify": spec.notify,
                    "next_due": self.next_due(spec).map(|d| d.to_rfc3339()),
                })
            })
            .collect();
        serde_json::json!({
            "timezone": self.tz.name(),
            "routines": routines,
            "recent_runs": self.recent_runs(10).await,
        })
    }

    pub async fn recent_runs(&self, limit: i64) -> Vec<serde_json::Value> {
        let Some(pool) = &self.pool else { return Vec::new() };
        let rows = sqlx::query(
            r#"
            SELECT routine, title, fired_by, status, started_at, finished_at, LEFT(summary, 400) AS summary
            FROM amos_runs WHERE entity_id = $1 ORDER BY started_at DESC LIMIT $2
            "#,
        )
        .bind(&self.entity)
        .bind(limit)
        .fetch_all(pool)
        .await
        .unwrap_or_default();
        rows.into_iter()
            .map(|r| {
                serde_json::json!({
                    "routine": r.get::<String, _>("routine"),
                    "title": r.get::<String, _>("title"),
                    "fired_by": r.get::<String, _>("fired_by"),
                    "status": r.get::<String, _>("status"),
                    "started_at": r.get::<chrono::DateTime<Utc>, _>("started_at").to_rfc3339(),
                    "finished_at": r.get::<Option<chrono::DateTime<Utc>>, _>("finished_at").map(|d| d.to_rfc3339()),
                    "summary": r.get::<Option<String>, _>("summary").unwrap_or_default(),
                })
            })
            .collect()
    }

    // ─── Scheduling & execution ──────────────────────────────────────────────

    /// Background scheduler: every 30s, fire any routine whose cron has an
    /// occurrence since the last tick (business timezone).
    pub fn spawn_scheduler(self: &Arc<Self>, state: Arc<AppState>) {
        let ops = self.clone();
        tokio::spawn(async move {
            let mut last = Utc::now().with_timezone(&ops.tz);
            let mut tick = tokio::time::interval(std::time::Duration::from_secs(30));
            tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
            loop {
                tick.tick().await;
                let now = Utc::now().with_timezone(&ops.tz);
                let paused = ops.paused.read().await.clone();
                for spec in ops.routines.iter().filter(|s| s.enabled && !paused.contains(&s.name)) {
                    let Ok(schedule) = cron::Schedule::from_str(&spec.cron) else { continue };
                    if schedule.after(&last).next().is_some_and(|due| due <= now) {
                        info!("🗓️ ops: cron fired for {}", spec.name);
                        let _ = ops.run_now(&state, &spec.name, "cron", None).await;
                    }
                }
                last = now;
            }
        });
    }

    /// Fire a routine (Skip concurrency: refused while already running).
    /// Returns immediately; the run executes in the background. `context` is
    /// extra event data appended to the routine's prompt — the reactive-trigger
    /// primitive (a webhook/ERP event posts what happened; the routine acts on
    /// it).
    pub async fn run_now(
        self: &Arc<Self>,
        state: &Arc<AppState>,
        name: &str,
        fired_by: &str,
        context: Option<String>,
    ) -> Result<String> {
        let spec = self.spec(name).ok_or_else(|| {
            anyhow!("unknown routine '{name}' — available: {}", self.routines.iter().map(|r| r.name.as_str()).collect::<Vec<_>>().join(", "))
        })?.clone();
        {
            let mut running = self.running.lock().await;
            if !running.insert(spec.name.clone()) {
                return Err(anyhow!("routine '{}' is already running", spec.name));
            }
        }
        let ops = self.clone();
        let state = state.clone();
        let started = format!("routine '{name}' started ({fired_by})");
        let fired_by = fired_by.to_string();
        tokio::spawn(async move {
            ops.execute(&state, &spec, &fired_by, context.as_deref()).await;
            ops.running.lock().await.remove(&spec.name);
        });
        Ok(started)
    }

    async fn execute(&self, state: &Arc<AppState>, spec: &RoutineSpec, fired_by: &str, context: Option<&str>) {
        let run_id = uuid::Uuid::new_v4();
        info!("🗓️ ops run {run_id}: {} ({fired_by})", spec.name);
        if let Some(pool) = &self.pool {
            let _ = sqlx::query(
                "INSERT INTO amos_runs (id, entity_id, routine, title, fired_by, status) VALUES ($1, $2, $3, $4, $5, 'running')",
            )
            .bind(run_id)
            .bind(&self.entity)
            .bind(&spec.name)
            .bind(&spec.title)
            .bind(fired_by)
            .execute(pool)
            .await;
        }

        let started = std::time::Instant::now();
        let outcome = tokio::time::timeout(std::time::Duration::from_secs(300), self.run_subagent(state, spec, run_id, context)).await;
        let (status, summary) = match outcome {
            Ok(Ok(text)) if !text.trim().is_empty() => ("ok", text),
            Ok(Ok(_)) => ("failed", "the routine produced no report".to_string()),
            Ok(Err(e)) => ("failed", format!("routine error: {e}")),
            Err(_) => ("failed", "routine timed out after 300s".to_string()),
        };
        info!("🗓️ ops run {run_id}: {} → {status} in {:.0}s", spec.name, started.elapsed().as_secs_f32());

        if let Some(pool) = &self.pool {
            let _ = sqlx::query("UPDATE amos_runs SET status = $2, finished_at = now(), summary = $3 WHERE id = $1")
                .bind(run_id)
                .bind(status)
                .bind(&summary)
                .execute(pool)
                .await;
        }

        // In-app notification: the routine's report lands in the ERP inbox the
        // user already checks. (Direct insert: the inbox is per-entity rows in
        // the shared database; there is no public POST endpoint by design.)
        if spec.notify {
            self.notify_inbox(&spec.title, &summary, status).await;
        }

        // Live push: any open Amos session sees the run complete in real time.
        state.push_json(serde_json::json!({
            "type": "routine_done",
            "routine": spec.name,
            "title": spec.title,
            "status": status,
            "summary": summary,
        }));
    }

    /// One-shot sub-agent: Gemini Flash + the routine's scoped/audited ERP
    /// tools + the skill playbook. Text in, report out — no voice, no browser.
    async fn run_subagent(&self, state: &Arc<AppState>, spec: &RoutineSpec, run_id: uuid::Uuid, context: Option<&str>) -> Result<String> {
        use adk_session::{CreateRequest, SessionService};
        use futures::StreamExt;

        // Tool surface: the spec's explicit tools plus the skill's allowed
        // tools — ERP only. Routines never get the browser (nothing should
        // click around the UI unattended) and native session tools don't exist
        // here.
        let mut allowed: HashSet<String> = spec.tools.iter().cloned().collect();
        if let Some(skill) = &spec.skill {
            for s in state.skills.summaries() {
                if &s.name == skill {
                    allowed.extend(s.allowed_tools.iter().cloned());
                }
            }
        }
        allowed.retain(|t| !t.starts_with("browser_"));

        let granted = Arc::new({
            let mut scopes = spec.scopes.clone();
            scopes.push(format!("tenant:{}", self.entity));
            scopes
        });
        let tools = crate::mcp::named_tools(&state.manager, &allowed).await?;
        if tools.is_empty() {
            return Err(anyhow!("no tools resolved for routine '{}'", spec.name));
        }

        let playbook = spec
            .skill
            .as_deref()
            .and_then(|s| state.skills.body_block(s))
            .map(|b| format!("\n\n## Playbook\n{b}"))
            .unwrap_or_default();
        let now = Utc::now().with_timezone(&self.tz);
        let instruction = format!(
            "You are Amos Ops, the background accountant routine runner for this business \
             (Zavora ERA). Today is {} ({}). You run UNATTENDED: never ask questions, never \
             wait for confirmation, and take only the actions the task explicitly allows — \
             your tool access is already limited to exactly those. Figures come from tool \
             results only; never invent a number. Finish with the final report as plain text \
             (short lines, no preamble).{playbook}",
            now.format("%A %d %B %Y, %H:%M"),
            self.tz.name(),
        );

        let api_key = std::env::var("GOOGLE_API_KEY").map_err(|_| anyhow!("GOOGLE_API_KEY not set"))?;
        let model_id = std::env::var("AMOS_OPS_MODEL").unwrap_or_else(|_| "gemini-flash-latest".into());
        let model = Arc::new(adk_model::GeminiModel::new(&api_key, &model_id).map_err(|e| anyhow!("gemini model: {e}"))?);

        let mut builder = adk_agent::LlmAgentBuilder::new("amos-ops")
            .description("Amos background routine runner")
            .model(model)
            .instruction(instruction);
        for tool in tools {
            // Same scope + audit pipeline as the live session: every tool call
            // is checked against the routine's granted scopes and recorded.
            builder = builder.tool(crate::scope::ScopedTool::wrap(
                tool,
                granted.clone(),
                format!("amos-ops:{}", spec.name),
                run_id.to_string(),
                state.audit.clone(),
            ));
        }
        let agent = Arc::new(builder.build().map_err(|e| anyhow!("agent build: {e}"))?);

        let sessions: Arc<dyn SessionService> = Arc::new(adk_session::InMemorySessionService::new());
        let session_id = run_id.to_string();
        sessions
            .create(CreateRequest {
                app_name: "amos-ops".into(),
                user_id: "amos-ops".into(),
                session_id: Some(session_id.clone()),
                state: Default::default(),
            })
            .await
            .map_err(|e| anyhow!("session: {e}"))?;
        let runner = adk_runner::Runner::builder()
            .app_name("amos-ops")
            .agent(agent)
            .session_service(sessions)
            .build()
            .map_err(|e| anyhow!("runner: {e}"))?;

        let mut stream = runner
            .run(
                adk_core::UserId::new("amos-ops").map_err(|e| anyhow!("{e}"))?,
                adk_core::SessionId::new(&session_id).map_err(|e| anyhow!("{e}"))?,
                adk_core::Content::new("user").with_text(&match context {
                    // Reactive trigger: the firing event's data rides along.
                    Some(ctx) => format!("{}\n\n## Trigger context (from the event that fired this run)\n{}", spec.prompt, ctx),
                    None => spec.prompt.clone(),
                }),
            )
            .await
            .map_err(|e| anyhow!("run: {e}"))?;

        // The final report is all text streamed AFTER the last tool call.
        // Gemini streaming (observed): text arrives as partial=true deltas;
        // the closing partial=false event carries NO text; function calls mark
        // the boundary between narration and the answer. So: append every text
        // delta, and reset the buffer whenever a FunctionCall appears — text
        // before a tool call was pre-tool narration, not the report.
        let mut report = String::new();
        while let Some(event) = stream.next().await {
            let event = event.map_err(|e| anyhow!("stream: {e}"))?;
            if let Some(content) = &event.llm_response.content {
                if content.parts.iter().any(|p| matches!(p, adk_core::Part::FunctionCall { .. })) {
                    report.clear();
                }
                for p in &content.parts {
                    if let adk_core::Part::Text { text } = p {
                        report.push_str(text);
                    }
                }
            }
        }
        Ok(report)
    }

    /// Deliver a routine report to the ERP's in-app notification inbox.
    async fn notify_inbox(&self, title: &str, body: &str, status: &str) {
        let Some(pool) = &self.pool else { return };
        let subject = if status == "ok" { format!("Amos · {title}") } else { format!("Amos · {title} — needs attention") };
        let entity: uuid::Uuid = match self.entity.parse() {
            Ok(id) => id,
            Err(_) => return,
        };
        if let Err(e) = sqlx::query(
            r#"INSERT INTO notifications
               (id, entity_id, event_type, channel, recipient, subject, body,
                related_type, related_id, status, scheduled_at, created_at)
               VALUES ($1, $2, 'amos_routine', 'in_app', '', $3, $4, 'amos_routine', NULL, 'sent', now(), now())"#,
        )
        .bind(uuid::Uuid::new_v4())
        .bind(entity)
        .bind(&subject)
        .bind(body)
        .execute(pool)
        .await
        {
            warn!("ops: inbox notification failed: {e}");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every shipped routine parses, carries a valid cron, and respects the
    /// safety defaults (no browser tools; write scopes only where declared).
    #[test]
    fn shipped_routines_parse_and_are_safe() {
        let dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("routines");
        let mut names = Vec::new();
        for entry in std::fs::read_dir(&dir).expect("routines dir").flatten() {
            let path = entry.path();
            if path.extension().is_none_or(|e| e != "toml") {
                continue;
            }
            let spec: RoutineSpec = toml::from_str(&std::fs::read_to_string(&path).unwrap())
                .unwrap_or_else(|e| panic!("{} failed to parse: {e}", path.display()));
            cron::Schedule::from_str(&spec.cron)
                .unwrap_or_else(|e| panic!("{}: invalid cron '{}': {e}", spec.name, spec.cron));
            assert!(!spec.prompt.trim().is_empty(), "{}: empty prompt", spec.name);
            assert!(
                spec.tools.iter().all(|t| !t.starts_with("browser_")),
                "{}: routines must not request browser tools",
                spec.name
            );
            names.push(spec.name);
        }
        for expected in ["morning-briefing", "etims-sweep", "month-end-pack"] {
            assert!(names.iter().any(|n| n == expected), "missing shipped routine {expected}");
        }
        // Only the eTIMS sweep needs a write scope in the shipped pack.
        let etims: RoutineSpec =
            toml::from_str(&std::fs::read_to_string(dir.join("etims-sweep.toml")).unwrap()).unwrap();
        assert!(etims.scopes.iter().any(|s| s == "erp:write"));
        let briefing: RoutineSpec =
            toml::from_str(&std::fs::read_to_string(dir.join("morning-briefing.toml")).unwrap()).unwrap();
        assert_eq!(briefing.scopes, vec!["erp:read"]);
    }
}
