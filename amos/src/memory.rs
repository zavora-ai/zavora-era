//! Amos's long-term memory: semantic storage over adk-memory.
//!
//! Backend: `PostgresMemoryService` (pgvector cosine search) with Gemini
//! embeddings; falls back to `InMemoryMemoryService` when no database is
//! reachable so a memory outage never stops Amos from working.
//!
//! Taxonomy (single-tenant today: app "amos", user "zavora"):
//! - `profile` — durable facts about the business and the owner's preferences
//! - `lesson`  — workflow gotchas, scoped per skill via project "skill:<name>"
//! - `session` — end-of-session summaries for continuity

use adk_core::Content;
use adk_memory::{InMemoryMemoryService, MemoryEntry, MemoryService, SearchRequest};
use anyhow::Result;
use serde::Serialize;
use std::sync::Arc;
use tracing::{info, warn};

const APP: &str = "amos";
/// Compact embeddings: plenty for recall quality, fits pgvector's direct
/// HNSW index (≤2000 dims).
const EMBEDDING_DIMS: i32 = 768;

/// Bridge adk-rag's per-text embedding trait to adk-memory's batch trait.
struct GeminiEmbeddingAdapter(adk_rag::GeminiEmbeddingProvider);

#[async_trait::async_trait]
impl adk_memory::EmbeddingProvider for GeminiEmbeddingAdapter {
    async fn embed(&self, texts: &[String]) -> adk_core::Result<Vec<Vec<f32>>> {
        use adk_rag::EmbeddingProvider as RagEmbedding;
        let mut out = Vec::with_capacity(texts.len());
        for text in texts {
            let v = RagEmbedding::embed(&self.0, text)
                .await
                .map_err(|e| adk_core::AdkError::memory(format!("gemini embed: {e}")))?;
            out.push(v);
        }
        Ok(out)
    }

    fn dimensions(&self) -> usize {
        use adk_rag::EmbeddingProvider as RagEmbedding;
        RagEmbedding::dimensions(&self.0)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum MemoryKind {
    Profile,
    Lesson,
    Session,
}

impl MemoryKind {
    fn author(self) -> &'static str {
        match self {
            Self::Profile => "profile",
            Self::Lesson => "lesson",
            Self::Session => "session",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "profile" => Some(Self::Profile),
            "lesson" => Some(Self::Lesson),
            "session" => Some(Self::Session),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct MemoryItem {
    pub kind: String,
    pub text: String,
    pub at: chrono::DateTime<chrono::Utc>,
}

pub struct AmosMemory {
    service: Arc<dyn MemoryService>,
    /// Tenant scope for every read/write (the served entity id). Isolates
    /// memory per tenant — a different deployment (different entity) shares
    /// none of it.
    user_scope: String,
    pub backend: &'static str,
}

fn skill_project(skill: &str) -> String {
    format!("skill:{skill}")
}

impl AmosMemory {
    pub async fn connect(served_entity: uuid::Uuid) -> Self {
        let url = std::env::var("AMOS_MEMORY_DATABASE_URL")
            .unwrap_or_else(|_| "postgres://zavora:zavora@localhost:5433/zavora_era".to_string());

        match Self::connect_postgres(&url).await {
            Ok(service) => {
                info!("memory: postgres (pgvector, {EMBEDDING_DIMS} dims) at {url}");
                return Self { service, user_scope: served_entity.to_string(), backend: "postgres" };
            }
            Err(e) => warn!("memory: postgres unavailable ({e}); falling back to in-memory"),
        }

        Self { service: Arc::new(InMemoryMemoryService::new()), user_scope: served_entity.to_string(), backend: "in-memory" }
    }

    async fn connect_postgres(url: &str) -> Result<Arc<dyn MemoryService>> {
        let api_key = std::env::var("GOOGLE_API_KEY")
            .map_err(|_| anyhow::anyhow!("GOOGLE_API_KEY not set"))?;
        let provider = adk_rag::GeminiEmbeddingProvider::new(&api_key)
            .map_err(|e| anyhow::anyhow!("gemini embedding provider: {e}"))?
            .with_output_dimensionality(EMBEDDING_DIMS);
        let service =
            adk_memory::PostgresMemoryService::new(url, Some(Arc::new(GeminiEmbeddingAdapter(provider))))
                .await
                .map_err(|e| anyhow::anyhow!("{e}"))?;
        service.migrate().await.map_err(|e| anyhow::anyhow!("memory migrate: {e}"))?;
        Ok(Arc::new(service))
    }

    /// Store a memory. Lessons are scoped to their skill's project so they
    /// surface when that skill is next used.
    pub async fn remember(&self, kind: MemoryKind, text: &str, skill: Option<&str>) -> Result<()> {
        let entry = MemoryEntry {
            content: Content::new("assistant").with_text(text),
            author: kind.author().to_string(),
            timestamp: chrono::Utc::now(),
        };
        match (kind, skill) {
            (MemoryKind::Lesson, Some(skill)) => {
                self.service
                    .add_entry_to_project(APP, &self.user_scope, &skill_project(skill), entry)
                    .await
                    .map_err(|e| anyhow::anyhow!("{e}"))?;
            }
            _ => {
                self.service.add_entry(APP, &self.user_scope, entry).await.map_err(|e| anyhow::anyhow!("{e}"))?;
            }
        }
        Ok(())
    }

    /// Semantic search. With `skill`, project-scoped lessons for that skill
    /// are included alongside global memories.
    pub async fn recall(&self, query: &str, skill: Option<&str>, limit: usize) -> Result<Vec<MemoryItem>> {
        self.search(query, skill, limit, Some(0.3)).await
    }

    /// Internal ranked search without a similarity threshold — used for panel
    /// listings and prompt injection, where "best available" beats "nothing".
    async fn search(
        &self,
        query: &str,
        skill: Option<&str>,
        limit: usize,
        min_score: Option<f32>,
    ) -> Result<Vec<MemoryItem>> {
        let resp = self
            .service
            .search(SearchRequest {
                query: query.to_string(),
                user_id: self.user_scope.clone(),
                app_name: APP.to_string(),
                limit: Some(limit),
                min_score,
                project_id: skill.map(skill_project),
            })
            .await
            .map_err(|e| anyhow::anyhow!("{e}"))?;
        Ok(resp.memories.into_iter().map(to_item).collect())
    }

    /// The "what you remember" block injected into the system prompt: profile
    /// facts plus the most recent session summary.
    pub async fn profile_block(&self, limit: usize) -> String {
        let mut lines = Vec::new();
        // Broad query: pgvector still ranks by similarity, and the in-memory
        // backend keyword-matches; either way we only surface `profile` and
        // the freshest `session` entries.
        if let Ok(items) = self.search("Zavora business facts preferences history", None, limit * 3, None).await {
            let mut sessions: Vec<&MemoryItem> = Vec::new();
            for item in &items {
                match item.kind.as_str() {
                    "profile" if lines.len() < limit => {
                        lines.push(format!("- {}", item.text));
                    }
                    "session" => sessions.push(item),
                    _ => {}
                }
            }
            if let Some(latest) = sessions.iter().max_by_key(|i| i.at) {
                lines.push(format!("- Last session ({}): {}", latest.at.format("%d %b %Y"), latest.text));
            }
        }
        if lines.is_empty() {
            "(nothing yet — use the remember tool as you learn about the business)".to_string()
        } else {
            lines.join("\n")
        }
    }

    /// Lessons block appended to a skill's playbook by use_skill.
    pub async fn lessons_block(&self, skill: &str, description: &str) -> Option<String> {
        let items = self.search(description, Some(skill), 8, None).await.ok()?;
        let lessons: Vec<String> = items
            .into_iter()
            .filter(|i| i.kind == "lesson")
            .map(|i| format!("- {}", i.text))
            .collect();
        if lessons.is_empty() {
            None
        } else {
            Some(format!("\n\n## Lessons learned from previous runs\n{}", lessons.join("\n")))
        }
    }

    /// Recent memories for the UI panel (best-effort, keyword-broad).
    pub async fn recent(&self, limit: usize) -> Vec<MemoryItem> {
        let mut items = self
            .search("Zavora business lesson session preference fact", None, limit, None)
            .await
            .unwrap_or_default();
        items.sort_by_key(|i| std::cmp::Reverse(i.at));
        items
    }
}

fn to_item(e: MemoryEntry) -> MemoryItem {
    MemoryItem {
        kind: e.author.clone(),
        text: adk_memory::text::extract_text(&e.content),
        at: e.timestamp,
    }
}
