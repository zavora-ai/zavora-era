//! End-of-session distillation: turn the transcript into durable memories.
//!
//! When a realtime session closes, the transcript is handed to a (non-realtime)
//! Gemini call that extracts a short summary plus any profile facts and skill
//! lessons worth keeping. Best-effort: failures log a warning and nothing else.

use crate::memory::MemoryKind;
use crate::state::AppState;
use serde_json::{Value, json};
use std::sync::Arc;
use tracing::{info, warn};

/// Ignore trivial sessions (greetings, empty connects).
const MIN_TRANSCRIPT_CHARS: usize = 200;

const DISTILL_PROMPT: &str = r#"You are the memory pipeline for Amos, an AI accountant for Zavora Technologies Ltd (Kenya). Below is the transcript of a working session between the business owner and Amos.

Extract ONLY durable knowledge worth keeping across sessions. Respond with pure JSON (no markdown fences):
{
  "summary": "2-3 sentences: what was done/decided this session",
  "profile_facts": ["stable business facts or owner preferences learned, if any"],
  "lessons": [{"skill": "skill-name-or-null", "text": "workflow gotcha worth avoiding next time"}]
}

Rules: no secrets or credentials; no transient figures (balances change); facts must be self-contained sentences; empty arrays are fine. Transcript:

"#;

pub fn spawn(state: Arc<AppState>, transcript: String) {
    if transcript.chars().count() < MIN_TRANSCRIPT_CHARS {
        return;
    }
    tokio::spawn(async move {
        match distill(&transcript).await {
            Ok(distilled) => store(&state, distilled).await,
            Err(e) => warn!("session summarizer failed: {e}"),
        }
    });
}

async fn distill(transcript: &str) -> anyhow::Result<Value> {
    let api_key = std::env::var("GOOGLE_API_KEY")?;
    let model = std::env::var("AMOS_SUMMARY_MODEL").unwrap_or_else(|_| "gemini-flash-latest".into());
    let body = json!({
        "contents": [{"role": "user", "parts": [{"text": format!("{DISTILL_PROMPT}{transcript}")}]}],
        "generationConfig": {"responseMimeType": "application/json", "temperature": 0.2}
    });
    let resp: Value = reqwest::Client::new()
        .post(format!("https://generativelanguage.googleapis.com/v1beta/models/{model}:generateContent"))
        .header("x-goog-api-key", api_key)
        .json(&body)
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;
    let text = resp["candidates"][0]["content"]["parts"][0]["text"]
        .as_str()
        .ok_or_else(|| anyhow::anyhow!("no text in generateContent response"))?;
    Ok(serde_json::from_str(text)?)
}

async fn store(state: &Arc<AppState>, distilled: Value) {
    let mut stored = 0usize;

    if let Some(summary) = distilled["summary"].as_str().filter(|s| !s.trim().is_empty()) {
        if state.memory.remember(MemoryKind::Session, summary, None).await.is_ok() {
            state.push_json(json!({"type": "memory", "kind": "session", "text": summary}));
            stored += 1;
        }
    }
    for fact in distilled["profile_facts"].as_array().into_iter().flatten() {
        if let Some(text) = fact.as_str().filter(|s| !s.trim().is_empty()) {
            if state.memory.remember(MemoryKind::Profile, text, None).await.is_ok() {
                state.push_json(json!({"type": "memory", "kind": "profile", "text": text}));
                stored += 1;
            }
        }
    }
    for lesson in distilled["lessons"].as_array().into_iter().flatten() {
        if let Some(text) = lesson["text"].as_str().filter(|s| !s.trim().is_empty()) {
            let skill = lesson["skill"].as_str().filter(|s| !s.is_empty() && *s != "null");
            if state.memory.remember(MemoryKind::Lesson, text, skill).await.is_ok() {
                state.push_json(json!({"type": "memory", "kind": "lesson", "text": text}));
                stored += 1;
            }
        }
    }
    info!("🧠 session distilled: {stored} memories stored");
}
