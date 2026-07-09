//! Application state for Amos: config, live task list, showcase feed, and the
//! broadcast channel that pushes panel updates to every connected UI.

use adk_realtime::gemini::{GeminiLiveBackend, GeminiRealtimeModel};
use adk_tool::mcp::manager::McpServerManager;
use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::{RwLock, broadcast};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskStatus {
    Pending,
    InProgress,
    Done,
    Failed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AmosTask {
    pub id: u32,
    pub title: String,
    pub status: TaskStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShowcaseStep {
    pub id: u32,
    pub caption: String,
    /// URL the UI can load the screenshot from (`/showcase/<file>`).
    pub image_url: Option<String>,
    pub at: chrono::DateTime<chrono::Utc>,
}

/// Pending write-confirmations for one session: the code gate behind
/// "postings always require your explicit confirmation". A ledger:post tool
/// registers a request here and blocks until the UI's Approve/Deny button
/// resolves it (or it times out). Prompt instructions alone could not stop
/// the model from writing immediately — this can.
#[derive(Default)]
pub struct Confirmations {
    pending: tokio::sync::Mutex<std::collections::HashMap<String, tokio::sync::oneshot::Sender<bool>>>,
}

impl Confirmations {
    /// Register a new confirmation; returns its id and the receiver the tool
    /// awaits.
    pub async fn request(&self) -> (String, tokio::sync::oneshot::Receiver<bool>) {
        let id = uuid::Uuid::new_v4().to_string();
        let (tx, rx) = tokio::sync::oneshot::channel();
        self.pending.lock().await.insert(id.clone(), tx);
        (id, rx)
    }

    /// Resolve a pending confirmation from the UI. Returns false if the id is
    /// unknown (already resolved / timed out) — resolving twice is harmless.
    pub async fn resolve(&self, id: &str, approve: bool) -> bool {
        match self.pending.lock().await.remove(id) {
            Some(tx) => tx.send(approve).is_ok(),
            None => false,
        }
    }

    /// Drop a request that timed out so the map doesn't accumulate.
    pub async fn forget(&self, id: &str) {
        self.pending.lock().await.remove(id);
    }
}

/// State scoped to ONE realtime session (one browser connection). Tasks,
/// evidence, and the active skill belong to the conversation that created
/// them — process-global versions bled between concurrent sessions (one
/// user's workplan overwrote another's, and evidence pushed to every UI).
pub struct SessionState {
    /// The skill last loaded via use_skill — failed tasks auto-file lessons
    /// under it.
    pub active_skill: RwLock<Option<String>>,
    pub tasks: RwLock<Vec<AmosTask>>,
    pub showcase: RwLock<Vec<ShowcaseStep>>,
    /// Write-confirmation gate for this session's posting tools.
    pub confirmations: Confirmations,
    /// JSON messages pushed to THIS session's UI websocket only.
    pub push: broadcast::Sender<String>,
}

impl SessionState {
    pub fn new() -> Self {
        let (push, _) = broadcast::channel(256);
        Self {
            active_skill: RwLock::new(None),
            tasks: RwLock::new(Vec::new()),
            showcase: RwLock::new(Vec::new()),
            confirmations: Confirmations::default(),
            push,
        }
    }

    /// Push a JSON message to this session's UI (ignores "no receivers").
    pub fn push_json(&self, value: serde_json::Value) {
        let _ = self.push.send(value.to_string());
    }

    pub async fn push_tasks(&self) {
        let tasks = self.tasks.read().await.clone();
        self.push_json(serde_json::json!({"type": "tasks", "tasks": tasks}));
    }
}

#[cfg(test)]
mod tests {
    use super::Confirmations;

    #[tokio::test]
    async fn confirmation_resolves_approval_to_waiter() {
        let c = Confirmations::default();
        let (id, rx) = c.request().await;
        assert!(c.resolve(&id, true).await, "first resolve reaches the waiter");
        assert_eq!(rx.await, Ok(true));
        // Second resolve (double click / stale id) is a no-op.
        assert!(!c.resolve(&id, false).await);
    }

    #[tokio::test]
    async fn confirmation_decline_and_forget() {
        let c = Confirmations::default();
        let (id, rx) = c.request().await;
        assert!(c.resolve(&id, false).await);
        assert_eq!(rx.await, Ok(false));

        let (id2, rx2) = c.request().await;
        c.forget(&id2).await;
        // Sender dropped → the waiting tool sees a closed channel (timeout path).
        assert!(rx2.await.is_err());
        assert!(!c.resolve(&id2, true).await);
    }
}

pub struct AppState {
    pub model: Arc<GeminiRealtimeModel>,
    pub manager: Arc<McpServerManager>,
    pub erp: crate::erp::ErpClient,
    pub skills: crate::skills::SkillsCatalog,
    pub memory: crate::memory::AmosMemory,
    /// Verifies user tokens and enforces the served-entity boundary.
    pub verifier: crate::auth::TokenVerifier,
    /// The single tenant this Amos serves.
    pub served_entity: uuid::Uuid,
    /// Optional audit sink (Postgres) — logs auth + tool-access events.
    pub audit: Option<std::sync::Arc<dyn adk_auth::AuditSink>>,
    /// Optional session-transcript store (Postgres) — the durable record of
    /// past conversations.
    pub history: Option<std::sync::Arc<crate::history::SessionHistory>>,
    /// Ambient operations: scheduled routine sub-agents + the ops ledger.
    /// `None` when no routines are configured.
    pub ops: Option<std::sync::Arc<crate::ops::Ops>>,
    /// Tenant-wide JSON messages pushed to every connected UI websocket.
    /// Memory events only — memory is shared across the tenant's sessions;
    /// per-session data (tasks, showcase, skill) goes over `SessionState.push`.
    pub push: broadcast::Sender<String>,
    pub showcase_dir: PathBuf,
    pub erp_ui_url: String,
    pub erp_login_email: String,
    pub erp_login_password: String,
}

impl AppState {
    pub fn new(
        manager: Arc<McpServerManager>,
        memory: crate::memory::AmosMemory,
        served_entity: uuid::Uuid,
        audit: Option<std::sync::Arc<dyn adk_auth::AuditSink>>,
        history: Option<std::sync::Arc<crate::history::SessionHistory>>,
        ops: Option<std::sync::Arc<crate::ops::Ops>>,
    ) -> Result<Self> {
        let api_key = std::env::var("GOOGLE_API_KEY")
            .map_err(|_| anyhow::anyhow!("GOOGLE_API_KEY environment variable must be set"))?;
        let model_id = std::env::var("GEMINI_LIVE_MODEL")
            .unwrap_or_else(|_| "models/gemini-live-2.5-flash-native-audio".to_string());
        let backend = GeminiLiveBackend::studio(&api_key);
        let model = Arc::new(GeminiRealtimeModel::new(backend, &model_id));

        let showcase_dir = std::env::var("AMOS_SHOWCASE_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("showcase"));
        std::fs::create_dir_all(&showcase_dir)?;

        let (push, _) = broadcast::channel(256);

        Ok(Self {
            model,
            manager,
            erp: crate::erp::ErpClient::from_env()?,
            skills: crate::skills::SkillsCatalog::load(),
            memory,
            verifier: crate::auth::TokenVerifier::new(served_entity)?,
            served_entity,
            audit,
            history,
            ops,
            push,
            showcase_dir,
            erp_ui_url: std::env::var("ERP_UI_URL").unwrap_or_else(|_| "http://localhost:3000".into()),
            // Browser (showcase) login may differ from the API service user so
            // the user can watch Amos work under a familiar account.
            erp_login_email: std::env::var("ERP_LOGIN_EMAIL")
                .or_else(|_| std::env::var("ZAVORA_EMAIL"))
                .unwrap_or_default(),
            erp_login_password: std::env::var("ERP_LOGIN_PASSWORD")
                .or_else(|_| std::env::var("ZAVORA_PASSWORD"))
                .unwrap_or_default(),
        })
    }

    /// Push a tenant-wide JSON message (memory events) to all connected UIs
    /// (ignores "no receivers").
    pub fn push_json(&self, value: serde_json::Value) {
        let _ = self.push.send(value.to_string());
    }
}
