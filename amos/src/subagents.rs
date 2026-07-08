//! Amos's specialist sub-agents, exposed to the realtime runner as native tools.
//!
//! Gemini Live (the voice/chat model Amos runs on) is a fast conversational
//! model — it cannot itself read an uploaded PDF, look at an image, or reach the
//! open web. So we give Amos two *sub-agents*: separate, single-purpose
//! generateContent calls that Amos delegates to, exactly like a person handing a
//! document to a specialist and asking "what does this say?".
//!
//! - **`analyze_attachment`** — a document/vision analyst. The user drops a PDF
//!   or image into the chat (plumbed over the WebSocket into [`AttachmentStore`]);
//!   this tool feeds those bytes to a multimodal Gemini model and returns a
//!   structured reading (vendor, totals, dates, line items, …).
//! - **`web_search`** — a research analyst grounded in Google Search. Returns an
//!   answer plus the source URLs it was grounded on, so Amos can cite them.
//!
//! Both are stateless per call and hold their own [`Gemini`] client (built from
//! `GOOGLE_API_KEY`, the same key the realtime session and embeddings use).

use adk_gemini::{CachedContentHandle, Content, Gemini, Model, Role, Tool};
use adk_realtime::config::ToolDefinition;
use adk_realtime::events::ToolCall;
use adk_realtime::runner::ToolHandler;
use async_trait::async_trait;
use serde_json::json;
use std::hash::{Hash, Hasher};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{Mutex, RwLock};
use tracing::{info, warn};

/// One file the user attached in the chat, held in memory for the session.
#[derive(Clone)]
pub struct Attachment {
    pub name: String,
    pub mime_type: String,
    /// Base64-encoded bytes (as received from the browser).
    pub data_b64: String,
}

/// Per-session, in-memory store of the files the user has attached. Shared
/// between the WebSocket ingest loop (writer) and the `analyze_attachment` tool
/// (reader). Never persisted — it lives and dies with the browser session.
pub type AttachmentStore = Arc<RwLock<Vec<Attachment>>>;

pub fn new_store() -> AttachmentStore {
    Arc::new(RwLock::new(Vec::new()))
}

/// A Gemini context-cache holding the current attachment set, so repeat
/// questions about the same document reuse cached tokens instead of re-uploading
/// and re-reading it every time (the document is the expensive part).
pub struct CachedDoc {
    fingerprint: u64,
    handle: CachedContentHandle,
}
pub type SharedDocCache = Arc<Mutex<Option<CachedDoc>>>;

pub fn new_doc_cache() -> SharedDocCache {
    Arc::new(Mutex::new(None))
}

/// Identify the attachment set so we can tell when it changed (cache-invalidate).
fn fingerprint(files: &[Attachment]) -> u64 {
    let mut h = std::collections::hash_map::DefaultHasher::new();
    for f in files {
        f.name.hash(&mut h);
        f.mime_type.hash(&mut h);
        f.data_b64.len().hash(&mut h);
    }
    h.finish()
}

const DOC_SYSTEM: &str = "You are a meticulous document-analysis specialist for a Kenyan accounting \
     system. Read the attached file(s) precisely. Extract figures verbatim \
     (never invent or round), keep currency and tax lines (VAT 16%) exact, and \
     use ISO dates. If a value is unreadable, say so rather than guessing. \
     Respond concisely and, where the user asks for fields, return them as a \
     clean structured list.";

/// Build a Gemini client for the sub-agents. Model is overridable via
/// `AMOS_SUBAGENT_MODEL`; defaults to Gemini 3 Flash — fast, multimodal (reads
/// PDFs and images), and supports Google Search grounding with the server-side
/// tool invocations the API now requires.
fn client() -> anyhow::Result<Gemini> {
    let api_key = std::env::var("GOOGLE_API_KEY")
        .map_err(|_| anyhow::anyhow!("GOOGLE_API_KEY not set"))?;
    let model: Model = std::env::var("AMOS_SUBAGENT_MODEL")
        .map(Model::from)
        .unwrap_or(Model::Gemini3FlashPreview);
    Gemini::with_model(api_key, model).map_err(|e| anyhow::anyhow!("gemini client: {e}"))
}

// ─── Contextual follow-up suggestions ───────────────────────────────────────

/// Propose up to 3 short follow-up actions from the recent conversation, as
/// `{label, prompt}` objects the UI renders as tappable chips. Best-effort and
/// cheap (a single flash call); returns empty on any error.
pub async fn generate_followups(context: &str) -> Vec<serde_json::Value> {
    if context.trim().is_empty() {
        return Vec::new();
    }
    let client = match client() {
        Ok(c) => c,
        Err(_) => return Vec::new(),
    };
    let prompt = format!(
        "You assist the owner of a Kenyan business inside their accounting system (Zavora ERP). \
         Based on the recent conversation below, propose EXACTLY 3 short, specific follow-up \
         actions the user is likely to want next. Return ONLY a JSON array of objects with keys \
         \"label\" (2-4 words, Title Case) and \"prompt\" (a full first-person question or \
         instruction to send). No markdown, no prose.\n\nConversation:\n{}",
        context.chars().rev().take(2400).collect::<String>().chars().rev().collect::<String>(),
    );
    let text = match client.generate_content().with_user_message(prompt).execute().await {
        Ok(resp) => resp.text(),
        Err(e) => { warn!("followups failed: {e}"); return Vec::new(); }
    };
    // Tolerate ```json fences and surrounding prose — extract the JSON array.
    let slice = match (text.find('['), text.rfind(']')) {
        (Some(a), Some(b)) if b > a => &text[a..=b],
        _ => return Vec::new(),
    };
    serde_json::from_str::<Vec<serde_json::Value>>(slice)
        .unwrap_or_default()
        .into_iter()
        .filter(|v| v.get("label").and_then(|l| l.as_str()).is_some_and(|s| !s.is_empty())
            && v.get("prompt").and_then(|p| p.as_str()).is_some_and(|s| !s.is_empty()))
        .take(3)
        .collect()
}

// ─── analyze_attachment (document / vision sub-agent) ────────────────────────

pub fn analyze_attachment_def() -> ToolDefinition {
    ToolDefinition {
        name: "analyze_attachment".into(),
        description: Some(
            "Read the file(s) the user attached in this chat — a PDF or an image \
             (invoice, receipt, bank statement, contract, photo of a document). \
             A specialist vision model reads them and returns exactly what you ask \
             for. Use this whenever the user says they've attached, uploaded, or \
             sent a document, or refers to 'this invoice / receipt / statement'. \
             If nothing is attached, this tells you so — then ask the user to \
             attach the file using the paperclip."
                .into(),
        ),
        parameters: Some(json!({
            "type": "object",
            "properties": {
                "instructions": {
                    "type": "string",
                    "description": "What to extract or answer from the attachment(s), e.g. \
                        'Extract vendor name, invoice number, date, each line item with \
                        amount, subtotal, VAT and total' or 'What is this document and \
                        what is the total payable?'"
                }
            },
            "required": ["instructions"]
        })),
    }
}

pub struct AnalyzeAttachment {
    pub attachments: AttachmentStore,
    /// Per-session context cache for the attached document(s).
    pub cache: SharedDocCache,
}

#[async_trait]
impl ToolHandler for AnalyzeAttachment {
    async fn execute(&self, call: &ToolCall) -> adk_realtime::error::Result<serde_json::Value> {
        let instructions = call.arguments["instructions"]
            .as_str()
            .unwrap_or("Describe this document and extract every field, amount, date and party.")
            .to_string();

        let files = self.attachments.read().await.clone();
        if files.is_empty() {
            return Ok(json!({
                "status": "no_attachment",
                "message": "No file is attached to this chat. Ask the user to attach the \
                            PDF or image using the paperclip button, then try again."
            }));
        }

        let client = match client() {
            Ok(c) => c,
            Err(e) => return Ok(json!({"status": "error", "message": e.to_string()})),
        };

        let names: Vec<String> = files.iter().map(|f| f.name.clone()).collect();
        info!("📎 analyze_attachment: {} file(s) {:?}", files.len(), names);

        // ── Cached path ──────────────────────────────────────────────────────
        // Repeat questions about the same document reuse a Gemini context cache
        // (the document tokens are billed once, then at the cheap cache rate).
        // Small files fall below Gemini's minimum cache size and simply fall
        // through to the inline path — which self-selects caching to the large
        // documents where it actually pays off.
        let want_cache = std::env::var("AMOS_ATTACHMENT_CACHE").map(|v| v != "0").unwrap_or(true);
        if want_cache {
            let fp = fingerprint(&files);
            let mut slot = self.cache.lock().await;
            if slot.as_ref().map(|c| c.fingerprint) != Some(fp) {
                *slot = None; // attachment set changed → drop the stale cache
            }
            if slot.is_none() {
                let mut cb = client.create_cache().with_system_instruction(DOC_SYSTEM).with_ttl(Duration::from_secs(600));
                for f in &files {
                    cb = cb.with_content(Content::inline_data(f.mime_type.clone(), f.data_b64.clone()).with_role(Role::User));
                }
                match cb.execute().await {
                    Ok(handle) => { info!("📎 cached document set ({} file(s))", files.len()); *slot = Some(CachedDoc { fingerprint: fp, handle }); }
                    Err(e) => { info!("📎 not cached (falling back to inline): {e}"); }
                }
            }
            if let Some(doc) = slot.as_ref() {
                match client.generate_content().with_cached_content(&doc.handle).with_user_message(instructions.clone()).execute().await {
                    Ok(resp) => return Ok(json!({"status": "ok", "cached": true, "files": names, "analysis": resp.text()})),
                    Err(e) => { warn!("cached read failed, retrying inline: {e}"); *slot = None; }
                }
            }
        }

        // ── Inline path (no cache / small file / cache miss) ─────────────────
        let mut builder = client.generate_content().with_system_instruction(DOC_SYSTEM).with_user_message(instructions);
        for f in &files {
            builder = builder.with_inline_data(f.data_b64.clone(), f.mime_type.clone());
        }
        match builder.execute().await {
            Ok(resp) => Ok(json!({"status": "ok", "cached": false, "files": names, "analysis": resp.text()})),
            Err(e) => {
                warn!("analyze_attachment failed: {e}");
                Ok(json!({"status": "error", "message": format!("Could not read the attachment: {e}")}))
            }
        }
    }
}

// ─── web_search (Google Search sub-agent) ────────────────────────────────────

pub fn web_search_def() -> ToolDefinition {
    ToolDefinition {
        name: "web_search".into(),
        description: Some(
            "Search the live internet (Google) for current, factual, or external \
             information Amos does not already know — e.g. today's KRA/CBK rates, a \
             supplier's details, current M-Pesa/tax rules, exchange rates, or news. \
             Returns an answer grounded in real web sources plus their URLs, which \
             you should cite. Do NOT use this for the user's own ledger data (use \
             the ERP tools for that)."
                .into(),
        ),
        parameters: Some(json!({
            "type": "object",
            "properties": {
                "query": {
                    "type": "string",
                    "description": "The search query or question, phrased naturally."
                }
            },
            "required": ["query"]
        })),
    }
}

pub struct WebSearch;

#[async_trait]
impl ToolHandler for WebSearch {
    async fn execute(&self, call: &ToolCall) -> adk_realtime::error::Result<serde_json::Value> {
        let query = match call.arguments["query"].as_str() {
            Some(q) if !q.trim().is_empty() => q,
            _ => return Ok(json!({"status": "error", "message": "Provide a non-empty query."})),
        };

        let client = match client() {
            Ok(c) => c,
            Err(e) => return Ok(json!({"status": "error", "message": e.to_string()})),
        };

        info!("🔍 web_search: {query}");
        let result = client
            .generate_content()
            .with_system_instruction(
                "You are a research analyst. Use Google Search to answer with current, \
                 accurate facts. Be concise. Prefer authoritative sources (KRA, CBK, \
                 official sites). If the web does not clearly answer, say so.",
            )
            .with_user_message(query)
            .with_tool(Tool::google_search())
            // Required by the Gemini API whenever a built-in tool (google_search)
            // is used — otherwise it 400s "Tool call context circulation is not
            // enabled". Lets the server run the search in-loop and hand us the
            // grounded answer + citations.
            .with_server_side_tool_invocations()
            .execute()
            .await;

        match result {
            Ok(resp) => {
                // Collect the web sources Gemini grounded the answer on.
                let sources: Vec<serde_json::Value> = resp
                    .candidates
                    .first()
                    .and_then(|c| c.grounding_metadata.as_ref())
                    .and_then(|g| g.grounding_chunks.as_ref())
                    .map(|chunks| {
                        chunks
                            .iter()
                            .filter_map(|ch| ch.web.as_ref())
                            .map(|w| {
                                json!({
                                    "title": w.title.clone().unwrap_or_default(),
                                    "url": w.uri.as_ref().map(|u| u.to_string()).unwrap_or_default(),
                                })
                            })
                            .collect()
                    })
                    .unwrap_or_default();

                Ok(json!({
                    "status": "ok",
                    "answer": resp.text(),
                    "sources": sources,
                }))
            }
            Err(e) => {
                warn!("web_search failed: {e}");
                Ok(json!({"status": "error", "message": format!("Search failed: {e}")}))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn call(name: &str, args: serde_json::Value) -> ToolCall {
        ToolCall { call_id: "test".into(), name: name.into(), arguments: args }
    }

    // Live smoke tests — require GOOGLE_API_KEY and network. Run explicitly:
    //   cargo test -p amos -- --ignored --nocapture
    #[tokio::test]
    #[ignore]
    async fn web_search_grounds_on_real_sources() {
        let out = WebSearch
            .execute(&call("web_search", json!({"query": "What is Kenya's standard VAT rate?"})))
            .await
            .unwrap();
        println!("web_search → {}", serde_json::to_string_pretty(&out).unwrap());
        assert_eq!(out["status"], "ok");
        assert!(!out["answer"].as_str().unwrap().is_empty(), "expected a non-empty answer");
        assert!(out["sources"].as_array().map(|s| !s.is_empty()).unwrap_or(false), "expected grounded sources");
    }

    #[tokio::test]
    #[ignore]
    async fn analyze_attachment_reads_a_png() {
        // 1×1 red PNG.
        let png_b64 = "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAAC0lEQVR42mP8z8BQDwAEhQGAhKmMIQAAAABJRU5ErkJggg==";
        let store = new_store();
        store.write().await.push(Attachment {
            name: "dot.png".into(),
            mime_type: "image/png".into(),
            data_b64: png_b64.into(),
        });
        let out = AnalyzeAttachment { attachments: store, cache: new_doc_cache() }
            .execute(&call("analyze_attachment", json!({"instructions": "What colour is this image? One word."})))
            .await
            .unwrap();
        println!("analyze_attachment → {}", serde_json::to_string_pretty(&out).unwrap());
        assert_eq!(out["status"], "ok");
    }

    #[tokio::test]
    async fn analyze_attachment_without_files_asks_for_one() {
        let out = AnalyzeAttachment { attachments: new_store(), cache: new_doc_cache() }
            .execute(&call("analyze_attachment", json!({"instructions": "read it"})))
            .await
            .unwrap();
        assert_eq!(out["status"], "no_attachment");
    }
}
