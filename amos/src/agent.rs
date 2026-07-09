//! Builds the Amos realtime runner: Gemini Live + bridged MCP tools (ERP +
//! browser) + the native task-list / showcase tools that feed the UI panels.

use crate::mcp;
use crate::persona;
use crate::state::{AmosTask, AppState, SessionState, ShowcaseStep, TaskStatus};
use adk_realtime::config::{RealtimeConfig, ToolDefinition};
use adk_realtime::events::ToolCall;
use adk_realtime::integration::{DefaultToolContextFactory, SessionIdentity, ToolBridgeAdapter};
use adk_realtime::integration::context::ToolContextFactory;
use adk_realtime::runner::{RealtimeRunner, ToolHandler};
use anyhow::Result;
use async_trait::async_trait;
use serde_json::json;
use std::sync::Arc;
use tracing::{info, warn};

pub async fn build_runner(
    state: &Arc<AppState>,
    session: &Arc<SessionState>,
    principal: Arc<crate::auth::Principal>,
    attachments: crate::subagents::AttachmentStore,
    clock: crate::clock::SharedClock,
    entitlements: crate::plan::Entitlements,
) -> Result<RealtimeRunner> {
    let ops_block = match &state.ops {
        Some(ops) => ops.prompt_block().await,
        None => "(ambient operations are not configured)".to_string(),
    };
    let instruction = persona::system_instruction(state)
        .replace("{memories}", &state.memory.profile_block(6).await)
        .replace("{ops}", &ops_block)
        .replace("{now}", &clock.read().await.instruction_block());

    // Voice is the expensive path (audio output ~$12/1M tokens). On a plan
    // without voice, force text-only output modality so the model never
    // generates (billable) audio — the single biggest cost control.
    let config = if entitlements.voice {
        let voice = std::env::var("AMOS_VOICE").unwrap_or_else(|_| "Charon".to_string());
        RealtimeConfig::default().with_instruction(&instruction).with_voice(voice).with_transcription()
    } else {
        RealtimeConfig::default().with_instruction(&instruction).with_modalities(vec!["text".to_string()])
    };

    let session_id = uuid::Uuid::new_v4().to_string();
    let factory: Arc<dyn ToolContextFactory> = Arc::new(DefaultToolContextFactory {
        identity: SessionIdentity {
            app_name: "amos".into(),
            user_id: principal.user_id.to_string(),
            session_id: session_id.clone(),
        },
        memory_service: None,
    });

    // Session authorization context for the scope wrapper + audit trail. The
    // session handle turns on the interactive confirm-before-write gate for
    // ledger:post tools (ambient ops pass None and stay unattended).
    let granted = Arc::new(principal.scopes());
    let user_id = principal.user_id.to_string();
    let scope = |tool: Arc<dyn adk_core::Tool>| {
        crate::scope::ScopedTool::wrap(
            tool,
            granted.clone(),
            user_id.clone(),
            session_id.clone(),
            state.audit.clone(),
            Some(session.clone()),
        )
    };

    let mut builder = RealtimeRunner::builder().model(state.model.clone()).config(config);

    let tools = mcp::agent_tools(&state.manager, &state.skills.extra_allowed_tools()).await?;
    info!("Bridging {} MCP tools (scopes: {:?}) into the realtime session", tools.len(), granted);
    for tool in tools {
        let mut def = ToolBridgeAdapter::definition(tool.as_ref());
        def.parameters = def.parameters.map(|p| sanitize_schema_root(&p));
        // Every ERP/browser tool is scope-checked + audited before it runs.
        let scoped = scope(tool);
        // browser_navigate to the ERP transparently signs in when the login
        // page appears — the model cannot flub authentication it never sees.
        if def.name == "browser_navigate" {
            def.description = Some(
                "Navigate the browser to a URL. Navigating to Zavora ERA signs you in \
                 automatically — you land on the dashboard, ready to work."
                    .into(),
            );
            builder = builder.tool_arc(
                def,
                Arc::new(AutoLoginNavigate {
                    inner: ToolBridgeAdapter::new(scoped, factory.clone()),
                    helper: ErpBrowserHelper { state: state.clone(), factory: factory.clone() },
                }),
            );
            continue;
        }
        builder = builder.tool_arc(def, Arc::new(ToolBridgeAdapter::new(scoped, factory.clone())));
    }

    builder = builder
        .tool_arc(plan_tasks_def(), Arc::new(PlanTasks { session: session.clone() }))
        .tool_arc(update_task_def(), Arc::new(UpdateTask { state: state.clone(), session: session.clone() }))
        .tool_arc(use_skill_def(), Arc::new(UseSkill { state: state.clone(), session: session.clone() }))
        .tool_arc(remember_def(), Arc::new(Remember { state: state.clone() }))
        .tool_arc(recall_def(), Arc::new(Recall { state: state.clone() }))
        .tool_arc(forget_def(), Arc::new(Forget { state: state.clone() }))
        .tool_arc(ops_status_def(), Arc::new(OpsStatus { state: state.clone() }))
        .tool_arc(run_routine_def(), Arc::new(RunRoutine { state: state.clone() }))
        .tool_arc(erp_login_def(), Arc::new(ErpLoginTool { state: state.clone(), factory: factory.clone() }))
        .tool_arc(showcase_def(), Arc::new(ShowcaseStepTool { state: state.clone(), session: session.clone(), factory: factory.clone() }))
        // Specialist sub-agents: read attached PDFs/images (every plan).
        .tool_arc(
            crate::subagents::analyze_attachment_def(),
            Arc::new(crate::subagents::AnalyzeAttachment { attachments, cache: crate::subagents::new_doc_cache() }),
        )
        // Real date/time in the user's timezone + their work-as-of posting date.
        .tool_arc(crate::clock::current_datetime_def(), Arc::new(crate::clock::CurrentDateTime { clock }));

    // web_search (per-query Google grounding cost) is a paid-plan capability.
    if entitlements.web_search {
        builder = builder.tool_arc(crate::subagents::web_search_def(), Arc::new(crate::subagents::WebSearch));
    }

    Ok(builder.build()?)
}

// ─── Schema sanitizing ───────────────────────────────────────────────────────
// MCP tools carry full JSON Schema (with `$schema`, `$defs`/`$ref`, `anyOf`,
// `additionalProperties`, ...). Gemini Live's function declarations accept only
// a narrow subset, so reduce every schema to: type, description, enum,
// properties, required, items, nullable — resolving refs and unions.

fn sanitize_schema_root(schema: &serde_json::Value) -> serde_json::Value {
    let empty = serde_json::Map::new();
    let defs = schema
        .get("$defs")
        .or_else(|| schema.get("definitions"))
        .and_then(|d| d.as_object())
        .unwrap_or(&empty);
    let mut out = sanitize_schema(schema, defs, 0);
    // Gemini requires the top level to be an object schema.
    if out.get("type").and_then(|t| t.as_str()) != Some("object") {
        out = json!({"type": "object", "properties": {}});
    }
    out
}

fn sanitize_schema(node: &serde_json::Value, defs: &serde_json::Map<String, serde_json::Value>, depth: usize) -> serde_json::Value {
    if depth > 12 {
        return json!({"type": "string"});
    }
    let Some(obj) = node.as_object() else {
        return json!({"type": "string"});
    };

    // Resolve local $refs ("#/$defs/Name" / "#/definitions/Name").
    if let Some(r) = obj.get("$ref").and_then(|r| r.as_str()) {
        if let Some(name) = r.rsplit('/').next() {
            if let Some(target) = defs.get(name) {
                return sanitize_schema(target, defs, depth + 1);
            }
        }
        return json!({"type": "string"});
    }

    // Unions: take the first non-null branch, mark nullable if null appears.
    for key in ["anyOf", "oneOf", "allOf"] {
        if let Some(branches) = obj.get(key).and_then(|v| v.as_array()) {
            let has_null = branches.iter().any(|b| b.get("type").and_then(|t| t.as_str()) == Some("null"));
            let first = branches
                .iter()
                .find(|b| b.get("type").and_then(|t| t.as_str()) != Some("null"))
                .unwrap_or(node);
            let mut out = sanitize_schema(first, defs, depth + 1);
            if has_null {
                out["nullable"] = json!(true);
            }
            if let (Some(desc), None) = (obj.get("description"), out.get("description")) {
                out["description"] = desc.clone();
            }
            return out;
        }
    }

    let mut out = serde_json::Map::new();
    // `type` may be "string" or ["string","null"].
    match obj.get("type") {
        Some(serde_json::Value::String(t)) => {
            out.insert("type".into(), json!(t));
        }
        Some(serde_json::Value::Array(ts)) => {
            let t = ts.iter().filter_map(|v| v.as_str()).find(|t| *t != "null").unwrap_or("string");
            out.insert("type".into(), json!(t));
            if ts.iter().any(|v| v.as_str() == Some("null")) {
                out.insert("nullable".into(), json!(true));
            }
        }
        _ => {
            if obj.contains_key("properties") {
                out.insert("type".into(), json!("object"));
            } else if obj.contains_key("items") {
                out.insert("type".into(), json!("array"));
            } else {
                out.insert("type".into(), json!("string"));
            }
        }
    }
    if let Some(d) = obj.get("description") {
        out.insert("description".into(), d.clone());
    }
    if let Some(e) = obj.get("enum") {
        out.insert("enum".into(), e.clone());
        // Gemini only supports enums on strings.
        out.insert("type".into(), json!("string"));
    }
    if let Some(props) = obj.get("properties").and_then(|p| p.as_object()) {
        let mut clean = serde_json::Map::new();
        for (k, v) in props {
            clean.insert(k.clone(), sanitize_schema(v, defs, depth + 1));
        }
        out.insert("properties".into(), serde_json::Value::Object(clean));
        if let Some(req) = obj.get("required") {
            out.insert("required".into(), req.clone());
        }
    }
    if let Some(items) = obj.get("items") {
        out.insert("items".into(), sanitize_schema(items, defs, depth + 1));
    }
    serde_json::Value::Object(out)
}

// ─── plan_tasks ──────────────────────────────────────────────────────────────

fn plan_tasks_def() -> ToolDefinition {
    ToolDefinition {
        name: "plan_tasks".into(),
        description: Some(
            "Create (or replace) your visible task list for the current request. \
             Call this FIRST for any multi-step piece of work, before executing anything."
                .into(),
        ),
        parameters: Some(json!({
            "type": "object",
            "properties": {
                "tasks": {
                    "type": "array",
                    "items": {"type": "string"},
                    "description": "Short task titles, in execution order"
                }
            },
            "required": ["tasks"]
        })),
    }
}

struct PlanTasks {
    session: Arc<SessionState>,
}

#[async_trait]
impl ToolHandler for PlanTasks {
    async fn execute(&self, call: &ToolCall) -> adk_realtime::error::Result<serde_json::Value> {
        let titles: Vec<String> = call.arguments["tasks"]
            .as_array()
            .map(|a| a.iter().filter_map(|t| t.as_str().map(String::from)).collect())
            .unwrap_or_default();
        let tasks: Vec<AmosTask> = titles
            .iter()
            .enumerate()
            .map(|(i, t)| AmosTask { id: i as u32 + 1, title: t.clone(), status: TaskStatus::Pending, note: None })
            .collect();
        info!("📋 plan_tasks: {} tasks", tasks.len());
        *self.session.tasks.write().await = tasks;
        self.session.push_tasks().await;
        Ok(json!({"status": "ok", "task_count": titles.len(), "message": "Task list is now visible to the user."}))
    }
}

// ─── update_task ─────────────────────────────────────────────────────────────

fn update_task_def() -> ToolDefinition {
    ToolDefinition {
        name: "update_task".into(),
        description: Some(
            "Update one task on your visible task list. Mark it in_progress when you start \
             and done (or failed, with a note) when finished."
                .into(),
        ),
        parameters: Some(json!({
            "type": "object",
            "properties": {
                "id": {"type": "integer", "description": "Task id from plan_tasks (1-based)"},
                "status": {"type": "string", "enum": ["pending", "in_progress", "done", "failed"]},
                "note": {"type": "string", "description": "Optional short note (e.g. what failed, or a key figure)"}
            },
            "required": ["id", "status"]
        })),
    }
}

struct UpdateTask {
    state: Arc<AppState>,
    session: Arc<SessionState>,
}

#[async_trait]
impl ToolHandler for UpdateTask {
    async fn execute(&self, call: &ToolCall) -> adk_realtime::error::Result<serde_json::Value> {
        let id = call.arguments["id"].as_u64().unwrap_or(0) as u32;
        let status = match call.arguments["status"].as_str().unwrap_or("") {
            "in_progress" => TaskStatus::InProgress,
            "done" => TaskStatus::Done,
            "failed" => TaskStatus::Failed,
            _ => TaskStatus::Pending,
        };
        let note = call.arguments["note"].as_str().map(String::from);
        let mut found = false;
        let mut failed_title = None;
        {
            let mut tasks = self.session.tasks.write().await;
            if let Some(task) = tasks.iter_mut().find(|t| t.id == id) {
                task.status = status;
                if status == TaskStatus::Failed {
                    failed_title = Some(task.title.clone());
                }
                if note.is_some() {
                    task.note = note.clone();
                }
                found = true;
            }
        }
        self.session.push_tasks().await;

        // Failures are how Amos learns: file the note as a lesson under the
        // skill in play so the next run of that playbook sees it.
        if let (Some(title), Some(note)) = (failed_title, note) {
            let state = self.state.clone();
            let session = self.session.clone();
            tokio::spawn(async move {
                let skill = session.active_skill.read().await.clone();
                let text = format!("While doing '{title}': {note}");
                if matches!(state.memory.remember(crate::memory::MemoryKind::Lesson, &text, skill.as_deref()).await, Ok(true)) {
                    info!("🧠 auto-lesson filed{}", skill.as_deref().map(|s| format!(" under {s}")).unwrap_or_default());
                    state.push_json(json!({"type": "memory", "kind": "lesson", "text": text}));
                }
            });
        }
        Ok(json!({"status": if found { "ok" } else { "unknown_task_id" }}))
    }
}

// ─── use_skill ───────────────────────────────────────────────────────────────

fn use_skill_def() -> ToolDefinition {
    ToolDefinition {
        name: "use_skill".into(),
        description: Some(
            "Load the step-by-step playbook for an accounting job. Call this BEFORE planning \
             any multi-step work, with the skill name from your skills catalog, then follow \
             the returned workflow exactly."
                .into(),
        ),
        parameters: Some(json!({
            "type": "object",
            "properties": {
                "name": {"type": "string", "description": "Skill name from the catalog, e.g. record-vendor-bill"}
            },
            "required": ["name"]
        })),
    }
}

struct UseSkill {
    state: Arc<AppState>,
    session: Arc<SessionState>,
}

#[async_trait]
impl ToolHandler for UseSkill {
    async fn execute(&self, call: &ToolCall) -> adk_realtime::error::Result<serde_json::Value> {
        let name = call.arguments["name"].as_str().unwrap_or("").to_string();
        match self.state.skills.body_block(&name) {
            Some(mut block) => {
                info!("📖 use_skill: {name}");
                // The learning loop: lessons filed under this skill (from past
                // failures and corrections) ride along with the playbook.
                if let Some(lessons) = self.state.memory.lessons_block(&name, &block).await {
                    block.push_str(&lessons);
                }
                *self.session.active_skill.write().await = Some(name.clone());
                self.session.push_json(json!({"type": "skill", "name": name}));
                Ok(json!({"skill": name, "playbook": block, "instruction": "Follow this workflow exactly — tool order, checks, and confirmation gates."}))
            }
            None => Ok(json!({
                "error": format!("Unknown skill '{name}'"),
                "available_skills": self.state.skills.names(),
            })),
        }
    }
}

// ─── remember / recall ───────────────────────────────────────────────────────

fn remember_def() -> ToolDefinition {
    ToolDefinition {
        name: "remember".into(),
        description: Some(
            "Store a durable memory. Use kind 'profile' for business facts and the owner's \
             preferences (especially when the user corrects you), and kind 'lesson' for \
             workflow gotchas discovered while working (attach the skill name). \
             Never store passwords, keys, or other secrets."
                .into(),
        ),
        parameters: Some(json!({
            "type": "object",
            "properties": {
                "kind": {"type": "string", "enum": ["profile", "lesson"]},
                "content": {"type": "string", "description": "The fact or lesson, specific and self-contained"},
                "skill": {"type": "string", "description": "For lessons: the skill this applies to (e.g. record-vendor-bill)"}
            },
            "required": ["kind", "content"]
        })),
    }
}

struct Remember {
    state: Arc<AppState>,
}

#[async_trait]
impl ToolHandler for Remember {
    async fn execute(&self, call: &ToolCall) -> adk_realtime::error::Result<serde_json::Value> {
        let kind = crate::memory::MemoryKind::parse(call.arguments["kind"].as_str().unwrap_or(""))
            .unwrap_or(crate::memory::MemoryKind::Profile);
        let content = call.arguments["content"].as_str().unwrap_or("").to_string();
        let skill = call.arguments["skill"].as_str().map(String::from);
        if content.trim().is_empty() {
            return Ok(json!({"error": "content must not be empty"}));
        }
        // Enforce the AGENTS.md rule in code: secrets never enter memory.
        if crate::guard::looks_like_secret(&content) {
            warn!("remember rejected: content looks like a secret");
            return Ok(json!({"error": "I won't store passwords, keys, or other secrets in memory."}));
        }
        match self.state.memory.remember(kind, &content, skill.as_deref()).await {
            Ok(true) => {
                info!("🧠 remember [{kind:?}]: {content}");
                self.state.push_json(json!({"type": "memory", "kind": kind, "text": content}));
                Ok(json!({"status": "remembered"}))
            }
            Ok(false) => Ok(json!({"status": "already_known", "message": "A very similar memory already exists; nothing stored."})),
            Err(e) => {
                warn!("remember failed: {e}");
                Ok(json!({"error": e.to_string()}))
            }
        }
    }
}

fn recall_def() -> ToolDefinition {
    ToolDefinition {
        name: "recall".into(),
        description: Some(
            "Search your long-term memory semantically. Use when the user references past \
             work or preferences, or before starting a job you may have done before."
                .into(),
        ),
        parameters: Some(json!({
            "type": "object",
            "properties": {
                "query": {"type": "string", "description": "What you're trying to remember"},
                "skill": {"type": "string", "description": "Optional: also include lessons scoped to this skill"}
            },
            "required": ["query"]
        })),
    }
}

struct Recall {
    state: Arc<AppState>,
}

#[async_trait]
impl ToolHandler for Recall {
    async fn execute(&self, call: &ToolCall) -> adk_realtime::error::Result<serde_json::Value> {
        let query = call.arguments["query"].as_str().unwrap_or("");
        let skill = call.arguments["skill"].as_str();
        match self.state.memory.recall(query, skill, 5).await {
            Ok(items) if items.is_empty() => Ok(json!({"memories": [], "note": "nothing relevant remembered"})),
            Ok(items) => Ok(json!({"memories": items})),
            Err(e) => Ok(json!({"error": e.to_string()})),
        }
    }
}

fn forget_def() -> ToolDefinition {
    ToolDefinition {
        name: "forget".into(),
        description: Some(
            "Delete stored memories that are wrong or outdated — use when the user corrects \
             a fact you had remembered, or a lesson no longer applies. Pass the distinctive \
             words of the memory to remove (recall it first to quote it accurately)."
                .into(),
        ),
        parameters: Some(json!({
            "type": "object",
            "properties": {
                "query": {"type": "string", "description": "Distinctive words of the memory to delete (matches full-text)"},
                "skill": {"type": "string", "description": "For lessons: the skill the lesson is filed under"}
            },
            "required": ["query"]
        })),
    }
}

struct Forget {
    state: Arc<AppState>,
}

#[async_trait]
impl ToolHandler for Forget {
    async fn execute(&self, call: &ToolCall) -> adk_realtime::error::Result<serde_json::Value> {
        let query = call.arguments["query"].as_str().unwrap_or("");
        let skill = call.arguments["skill"].as_str();
        if query.trim().is_empty() {
            return Ok(json!({"error": "query must not be empty"}));
        }
        match self.state.memory.forget(query, skill).await {
            Ok(0) => Ok(json!({"status": "nothing_matched", "message": "No stored memory matched those words."})),
            Ok(n) => {
                info!("🧠 forget: removed {n} memories matching '{query}'");
                self.state.push_json(json!({"type": "memory_removed", "count": n}));
                Ok(json!({"status": "forgotten", "removed": n}))
            }
            Err(e) => Ok(json!({"error": e.to_string()})),
        }
    }
}

// ─── erp_login ───────────────────────────────────────────────────────────────

// ─── ambient ops: ops_status / run_routine ──────────────────────────────────

fn ops_status_def() -> ToolDefinition {
    ToolDefinition {
        name: "ops_status".into(),
        description: Some(
            "Your practice calendar: the scheduled background routines (morning briefing, \
             eTIMS sweep, month-end pack, ...), when each next fires, and how recent runs \
             went. Use when the user asks what's scheduled, whether something ran, or what \
             happened while they were away."
                .into(),
        ),
        parameters: Some(json!({"type": "object", "properties": {}})),
    }
}

struct OpsStatus {
    state: Arc<AppState>,
}

#[async_trait]
impl ToolHandler for OpsStatus {
    async fn execute(&self, _call: &ToolCall) -> adk_realtime::error::Result<serde_json::Value> {
        match &self.state.ops {
            Some(ops) => Ok(ops.status().await),
            None => Ok(json!({"error": "ambient operations are not configured"})),
        }
    }
}

fn run_routine_def() -> ToolDefinition {
    ToolDefinition {
        name: "run_routine".into(),
        description: Some(
            "Trigger a scheduled routine NOW instead of waiting for its cron (e.g. 'run the \
             month-end pack', 'send me the briefing now'). The routine runs in the background; \
             its report arrives as a notification and you'll see it in ops_status. Get routine \
             names from ops_status."
                .into(),
        ),
        parameters: Some(json!({
            "type": "object",
            "properties": {
                "name": {"type": "string", "description": "Routine name from ops_status, e.g. morning-briefing"},
                "context": {"type": "string", "description": "Optional: extra context for this run (e.g. 'focus on the Equity USD account')"}
            },
            "required": ["name"]
        })),
    }
}

struct RunRoutine {
    state: Arc<AppState>,
}

#[async_trait]
impl ToolHandler for RunRoutine {
    async fn execute(&self, call: &ToolCall) -> adk_realtime::error::Result<serde_json::Value> {
        let name = call.arguments["name"].as_str().unwrap_or("");
        let context = call.arguments["context"].as_str().map(String::from);
        match &self.state.ops {
            Some(ops) => match ops.run_now(&self.state, name, "manual", context).await {
                Ok(msg) => {
                    info!("🗓️ manual routine trigger: {name}");
                    Ok(json!({"status": "started", "message": msg}))
                }
                Err(e) => Ok(json!({"error": e.to_string()})),
            },
            None => Ok(json!({"error": "ambient operations are not configured"})),
        }
    }
}

fn erp_login_def() -> ToolDefinition {
    ToolDefinition {
        name: "erp_login".into(),
        description: Some(
            "Open Zavora ERA in the browser and log in. Call this FIRST whenever you need \
             the browser — it handles navigation and authentication in one deterministic step. \
             After it succeeds you are on the dashboard and can navigate anywhere."
                .into(),
        ),
        parameters: Some(json!({"type": "object", "properties": {}})),
    }
}

/// Collect every "text" field from an MCP tool result, whatever its shape.
fn collect_text(value: &serde_json::Value, out: &mut String) {
    match value {
        serde_json::Value::String(s) => {
            out.push_str(s);
            out.push('\n');
        }
        serde_json::Value::Object(o) => {
            if let Some(serde_json::Value::String(s)) = o.get("text") {
                out.push_str(s);
                out.push('\n');
            }
            for (k, v) in o {
                if k != "text" {
                    collect_text(v, out);
                }
            }
        }
        serde_json::Value::Array(a) => {
            for v in a {
                collect_text(v, out);
            }
        }
        _ => {}
    }
}

/// Pull `[ref=eNN]` off a snapshot line.
fn extract_ref(line: &str) -> Option<String> {
    let start = line.find("[ref=")? + 5;
    let end = line[start..].find(']')? + start;
    Some(line[start..end].to_string())
}

pub struct ErpBrowserHelper {
    pub state: Arc<AppState>,
    pub factory: Arc<dyn ToolContextFactory>,
}

impl ErpBrowserHelper {
    async fn call_browser(&self, call_id: &str, name: &str, args: serde_json::Value) -> anyhow::Result<String> {
        let tool = mcp::find_tool(&self.state.manager, name)
            .await
            .ok_or_else(|| anyhow::anyhow!("{name} tool unavailable"))?;
        let ctx = self.factory.create_context(call_id);
        let result = tool.execute(ctx, args).await.map_err(|e| anyhow::anyhow!("{name}: {e}"))?;
        let mut text = String::new();
        collect_text(&result, &mut text);
        Ok(text)
    }

    /// If the current page is the ERP sign-in form, complete it. Returns true
    /// when a login was performed.
    async fn ensure_logged_in(&self, call_id: &str) -> anyhow::Result<bool> {
        tokio::time::sleep(std::time::Duration::from_millis(800)).await;
        let snapshot = self.call_browser(call_id, "browser_snapshot", json!({})).await?;
        if !snapshot.contains("Sign in") {
            return Ok(false);
        }

        let mut textbox_refs = snapshot.lines().filter(|l| l.contains("textbox")).filter_map(extract_ref);
        let email_ref = textbox_refs.next().ok_or_else(|| anyhow::anyhow!("email field not found on login page"))?;
        let password_ref = textbox_refs.next().ok_or_else(|| anyhow::anyhow!("password field not found on login page"))?;
        let signin_ref = snapshot
            .lines()
            .find(|l| l.contains("button") && l.contains("Sign in"))
            .and_then(extract_ref)
            .ok_or_else(|| anyhow::anyhow!("sign-in button not found"))?;

        self.call_browser(call_id, "browser_type", json!({"element": "email field", "target": email_ref, "text": self.state.erp_login_email})).await?;
        self.call_browser(call_id, "browser_type", json!({"element": "password field", "target": password_ref, "text": self.state.erp_login_password})).await?;
        self.call_browser(call_id, "browser_click", json!({"element": "Sign in button", "target": signin_ref})).await?;
        tokio::time::sleep(std::time::Duration::from_millis(1500)).await;

        let after = self.call_browser(call_id, "browser_snapshot", json!({})).await?;
        if after.contains("Sign in") && after.contains("password") {
            anyhow::bail!("login did not complete — still on the sign-in page");
        }
        info!("🔐 signed into the ERP UI");
        Ok(true)
    }

    async fn login(&self, call_id: &str) -> anyhow::Result<serde_json::Value> {
        let ui_url = self.state.erp_ui_url.clone();
        self.call_browser(call_id, "browser_navigate", json!({"url": ui_url})).await?;
        let logged_in = self.ensure_logged_in(call_id).await?;
        Ok(json!({
            "status": if logged_in { "logged_in" } else { "already_logged_in" },
            "message": "Zavora ERA is open and signed in; the sidebar links to Bills, Invoices, Payments, Reports and more."
        }))
    }
}

struct ErpLoginTool {
    state: Arc<AppState>,
    factory: Arc<dyn ToolContextFactory>,
}

#[async_trait]
impl ToolHandler for ErpLoginTool {
    async fn execute(&self, call: &ToolCall) -> adk_realtime::error::Result<serde_json::Value> {
        info!("🔐 erp_login: signing into the ERP UI");
        let helper = ErpBrowserHelper { state: self.state.clone(), factory: self.factory.clone() };
        match helper.login(&call.call_id).await {
            Ok(v) => Ok(v),
            Err(e) => {
                warn!("erp_login failed: {e}");
                Ok(json!({"error": e.to_string()}))
            }
        }
    }
}

/// browser_navigate wrapper: after navigating to the ERP UI, complete the
/// sign-in form automatically if it appears.
pub struct AutoLoginNavigate {
    pub inner: ToolBridgeAdapter,
    pub helper: ErpBrowserHelper,
}

#[async_trait]
impl ToolHandler for AutoLoginNavigate {
    async fn execute(&self, call: &ToolCall) -> adk_realtime::error::Result<serde_json::Value> {
        let result = self.inner.execute(call).await?;
        let url = call.arguments["url"].as_str().unwrap_or("");
        if url.starts_with(&self.helper.state.erp_ui_url) {
            match self.helper.ensure_logged_in(&call.call_id).await {
                Ok(true) => {
                    return Ok(json!({
                        "status": "ok",
                        "message": "Navigated to Zavora ERA and signed in automatically. You are on the dashboard; the sidebar links to Bills, Invoices, Payments, Reports and more."
                    }));
                }
                Ok(false) => {}
                Err(e) => warn!("auto-login after navigate failed: {e}"),
            }
        }
        Ok(result)
    }
}

// ─── showcase_step ───────────────────────────────────────────────────────────

fn showcase_def() -> ToolDefinition {
    ToolDefinition {
        name: "showcase_step".into(),
        description: Some(
            "Capture the browser's current view as a showcase card the user sees next to the \
             chat. Navigate the browser to something worth showing first, then call this with \
             a short caption describing what's on screen."
                .into(),
        ),
        parameters: Some(json!({
            "type": "object",
            "properties": {
                "caption": {"type": "string", "description": "Short caption, e.g. 'The 12 Google bills, all posted'"}
            },
            "required": ["caption"]
        })),
    }
}

/// Newest PNG in `dir` modified within the last 60s (a screenshot the browser
/// MCP just wrote, whatever it decided to call the file).
fn newest_recent_png(dir: &std::path::Path) -> Option<std::path::PathBuf> {
    let now = std::time::SystemTime::now();
    std::fs::read_dir(dir)
        .ok()?
        .flatten()
        .filter(|e| e.path().extension().is_some_and(|x| x == "png"))
        .filter_map(|e| {
            let modified = e.metadata().ok()?.modified().ok()?;
            let age = now.duration_since(modified).ok()?;
            (age.as_secs() < 60).then_some((modified, e.path()))
        })
        .max_by_key(|(modified, _)| *modified)
        .map(|(_, path)| path)
}

/// Ceiling on evidence cards kept per session — a marathon session can't grow
/// the feed (and the UI's DOM) without bound.
const SHOWCASE_SESSION_CAP: usize = 50;

struct ShowcaseStepTool {
    state: Arc<AppState>,
    session: Arc<SessionState>,
    factory: Arc<dyn ToolContextFactory>,
}

impl ShowcaseStepTool {
    /// Best-effort recovery of a base64 PNG embedded anywhere in a tool result.
    fn find_base64_image(value: &serde_json::Value) -> Option<String> {
        match value {
            serde_json::Value::Object(o) => {
                if o.get("type").and_then(|t| t.as_str()) == Some("image") {
                    if let Some(data) = o.get("data").and_then(|d| d.as_str()) {
                        return Some(data.to_string());
                    }
                }
                o.values().find_map(Self::find_base64_image)
            }
            serde_json::Value::Array(a) => a.iter().find_map(Self::find_base64_image),
            _ => None,
        }
    }
}

#[async_trait]
impl ToolHandler for ShowcaseStepTool {
    async fn execute(&self, call: &ToolCall) -> adk_realtime::error::Result<serde_json::Value> {
        use base64::Engine as _;

        let caption = call.arguments["caption"].as_str().unwrap_or("Showcase").to_string();
        let step_id = self.session.showcase.read().await.len() as u32 + 1;
        let filename = format!("step-{step_id}-{}.png", chrono::Utc::now().timestamp());
        let path = self.state.showcase_dir.join(&filename);

        // Drive the screenshot through the Playwright MCP server.
        let mut image_url = None;
        match mcp::find_tool(&self.state.manager, "browser_take_screenshot").await {
            Some(tool) => {
                let ctx = self.factory.create_context(&call.call_id);
                // Absolute path: the MCP resolves relative filenames against its
                // own cwd, ignoring --output-dir.
                let args = json!({"filename": path.to_string_lossy(), "type": "png", "scale": "css"});
                match tool.execute(ctx, args).await {
                    Ok(result) => {
                        // Playwright MCP saves into --output-dir but may pick its
                        // own file name; fall back to the freshest PNG there, then
                        // to base64 image content embedded in the result.
                        if !path.exists() {
                            if let Some(recent) = newest_recent_png(&self.state.showcase_dir) {
                                let _ = std::fs::rename(&recent, &path);
                            }
                        }
                        if !path.exists() {
                            if let Some(b64) = Self::find_base64_image(&result) {
                                if let Ok(bytes) = base64::engine::general_purpose::STANDARD.decode(b64) {
                                    let _ = std::fs::write(&path, bytes);
                                }
                            }
                        }
                        if path.exists() {
                            image_url = Some(format!("/showcase/{filename}"));
                        } else {
                            warn!("showcase screenshot not found at {}", path.display());
                        }
                    }
                    Err(e) => warn!("browser_take_screenshot failed: {e}"),
                }
            }
            None => warn!("browser_take_screenshot tool not available"),
        }

        let step = ShowcaseStep {
            id: step_id,
            caption: caption.clone(),
            image_url: image_url.clone(),
            at: chrono::Utc::now(),
        };
        {
            let mut feed = self.session.showcase.write().await;
            feed.push(step.clone());
            if feed.len() > SHOWCASE_SESSION_CAP {
                let excess = feed.len() - SHOWCASE_SESSION_CAP;
                feed.drain(..excess);
            }
        }
        self.session.push_json(json!({"type": "showcase", "step": step}));
        info!("📸 showcase_step #{step_id}: {caption}");

        Ok(json!({
            "status": if image_url.is_some() { "ok" } else { "no_screenshot" },
            "image_url": image_url,
            "message": "Step card is now visible to the user."
        }))
    }
}
