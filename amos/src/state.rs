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

pub struct AppState {
    pub model: Arc<GeminiRealtimeModel>,
    pub manager: Arc<McpServerManager>,
    pub erp: crate::erp::ErpClient,
    pub skills: crate::skills::SkillsCatalog,
    pub memory: crate::memory::AmosMemory,
    /// The skill last loaded via use_skill — failed tasks auto-file lessons
    /// under it. Cleared when a fresh workplan is created.
    pub active_skill: RwLock<Option<String>>,
    pub tasks: RwLock<Vec<AmosTask>>,
    pub showcase: RwLock<Vec<ShowcaseStep>>,
    /// JSON messages pushed to every connected UI websocket.
    pub push: broadcast::Sender<String>,
    pub showcase_dir: PathBuf,
    pub erp_ui_url: String,
    pub erp_login_email: String,
    pub erp_login_password: String,
}

impl AppState {
    pub fn new(manager: Arc<McpServerManager>, memory: crate::memory::AmosMemory) -> Result<Self> {
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
            active_skill: RwLock::new(None),
            tasks: RwLock::new(Vec::new()),
            showcase: RwLock::new(Vec::new()),
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

    /// Push a JSON message to all connected UIs (ignores "no receivers").
    pub fn push_json(&self, value: serde_json::Value) {
        let _ = self.push.send(value.to_string());
    }

    pub async fn push_tasks(&self) {
        let tasks = self.tasks.read().await.clone();
        self.push_json(serde_json::json!({"type": "tasks", "tasks": tasks}));
    }
}
