//! Axum routes: the Amos web UI, panel REST endpoints, showcase images, and
//! the realtime WebSocket (binary = PCM audio both ways, JSON = everything else).

use crate::agent;
use crate::state::{AppState, TaskStatus};
use axum::{
    Json, Router,
    extract::{State, WebSocketUpgrade, ws},
    response::{Html, IntoResponse},
    routing::get,
};
use std::sync::Arc;
use tokio::sync::mpsc;
use tower_http::services::ServeDir;
use tracing::{error, info, warn};

pub fn create_router(state: Arc<AppState>) -> Router {
    Router::new()
        .route("/", get(serve_index))
        .route("/api/tasks", get(get_tasks))
        .route("/api/showcase", get(get_showcase))
        .route("/api/snapshot", get(get_snapshot))
        .route("/api/skills", get(get_skills))
        .route("/api/memories", get(get_memories))
        .route("/ws", get(ws_handler))
        .nest_service("/showcase", ServeDir::new(state.showcase_dir.clone()))
        .with_state(state)
}

async fn serve_index() -> Html<String> {
    Html(include_str!("../assets/index.html").to_string())
}

async fn get_tasks(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    Json(state.tasks.read().await.clone())
}

async fn get_showcase(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    Json(state.showcase.read().await.clone())
}

async fn get_skills(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    Json(state.skills.summaries())
}

async fn get_memories(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    Json(state.memory.recent(20).await)
}

/// Live business snapshot for the right-hand panel, straight from the ledger.
async fn get_snapshot(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    match state.erp.dashboard().await {
        Ok(d) => Json(serde_json::json!({
            "as_at": d["as_at"],
            "cash_and_bank": d["cash_and_bank"],
            "total_receivable": d["total_receivable"],
            "total_payable": d["total_payable"],
            "overdue_payable": d["overdue_payable"],
            "overdue_receivable": d["overdue_receivable"],
            "overdue_bill_count": d["overdue_bill_count"],
            "overdue_invoice_count": d["overdue_invoice_count"],
            "bank_accounts": d["bank_accounts"],
            "recent_transactions": d["recent_transactions"].as_array().map(|a| a.iter().take(5).cloned().collect::<Vec<_>>()).unwrap_or_default(),
        })).into_response(),
        Err(e) => (axum::http::StatusCode::BAD_GATEWAY, format!("snapshot unavailable: {e}")).into_response(),
    }
}

// ─── Realtime WebSocket ──────────────────────────────────────────────────────

async fn ws_handler(ws: WebSocketUpgrade, State(state): State<Arc<AppState>>) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_ws(socket, state))
}

/// The verified identity plus the client's session context (timezone +
/// work-as-of date) carried on the handshake frame.
struct Handshake {
    principal: crate::auth::Principal,
    timezone: Option<String>,
    work_date: Option<String>,
    plan: Option<String>,
}

/// Await the client's first `{type:"auth", token, timezone?, work_date?}` frame,
/// verify the token against the served entity, and capture the user's timezone +
/// work-as-of (posting) date preferences. Returns the handshake, or a refusal.
///
/// Dev escape hatch: with `AMOS_DEV_ALLOW_UNAUTH=1` (never set in prod) an
/// absent/blank token yields a synthetic owner principal for the served entity,
/// so standalone `:8090` testing works without the parent ERP page.
async fn authenticate<S>(ws_receiver: &mut S, state: &Arc<AppState>) -> Result<Handshake, String>
where
    S: futures::Stream<Item = Result<ws::Message, axum::Error>> + Unpin,
{
    use futures::StreamExt;
    let dev_allow = std::env::var("AMOS_DEV_ALLOW_UNAUTH").is_ok_and(|v| v == "1" || v == "true");

    // Parse the first frame once so we can read the token *and* the session
    // context (timezone / work-date) it carries.
    let frame = match tokio::time::timeout(std::time::Duration::from_secs(10), ws_receiver.next()).await {
        Ok(Some(Ok(ws::Message::Text(text)))) => serde_json::from_str::<serde_json::Value>(&text)
            .ok()
            .filter(|m| m.get("type").and_then(|t| t.as_str()) == Some("auth")),
        Ok(Some(Ok(ws::Message::Close(_)))) | Ok(None) => return Err("connection closed before authenticating".into()),
        Ok(Some(Ok(_))) => None,             // first frame wasn't auth
        Ok(Some(Err(e))) => return Err(format!("websocket error: {e}")),
        Err(_) => None,                      // timeout
    };
    let str_field = |k: &str| frame.as_ref().and_then(|m| m.get(k)).and_then(|v| v.as_str()).map(String::from);
    let token = str_field("token");
    let timezone = str_field("timezone");
    let work_date = str_field("work_date");
    let plan = str_field("plan");

    let principal = match token {
        Some(t) if !t.trim().is_empty() => state.verifier.verify(&t).map_err(|e| e.to_string())?,
        _ if dev_allow => {
            warn!("AMOS_DEV_ALLOW_UNAUTH set — accepting an unauthenticated dev session");
            crate::auth::Principal {
                user_id: uuid::Uuid::nil(),
                entity_id: state.served_entity,
                role: "Owner".into(),
            }
        }
        _ => return Err("Sign in to the ERP to use Amos.".into()),
    };
    Ok(Handshake { principal, timezone, work_date, plan })
}

async fn handle_ws(socket: ws::WebSocket, state: Arc<AppState>) {
    use adk_realtime::events::ServerEvent;
    use base64::Engine as _;
    use futures::{SinkExt, StreamExt};

    info!("🎙️ Amos session connecting…");
    let (mut ws_sender, mut ws_receiver) = socket.split();

    let send_error = |msg: String| serde_json::json!({"type": "error", "message": msg}).to_string();
    let send_fatal =
        |msg: String| serde_json::json!({"type": "error", "fatal": true, "message": msg}).to_string();

    // ── Identity gate ────────────────────────────────────────────────────────
    // Before ANY tool or model turn, the connection must prove it belongs to a
    // user of the served entity. The client sends {type:"auth", token:<JWT>} as
    // its first frame; we verify the signature and that the token's entity ==
    // the entity this Amos serves. A mismatch, a bad/expired token, or no auth
    // frame ⇒ the session is refused. No runner, no tools, no data, no memory.
    let handshake = match authenticate(&mut ws_receiver, &state).await {
        Ok(h) => h,
        Err(reason) => {
            warn!("session refused: {reason}");
            if let Some(sink) = &state.audit {
                let _ = sink
                    .log(adk_auth::AuditEvent::authentication("unknown", adk_auth::AuditOutcome::Denied))
                    .await;
            }
            let _ = ws_sender.send(ws::Message::Text(send_fatal(reason).into())).await;
            let _ = ws_sender.send(ws::Message::Close(None)).await;
            return;
        }
    };
    let principal = handshake.principal;
    // Per-user timezone + work-as-of (posting) date, from the handshake. Shared
    // so a mid-session `context` frame (the user changing their work-date) can
    // update the clock the current_datetime tool reads.
    let clock = crate::clock::shared(crate::clock::SessionClock::from_handshake(
        handshake.timezone.as_deref(),
        handshake.work_date.as_deref(),
    ));
    {
        let c = clock.read().await;
        info!("🕑 session clock: tz {} · posting date {}", c.tz.name(), c.effective_posting_date());
    }
    // Plan entitlements: gate the expensive capabilities (voice, web search) by
    // tier. Resolved handshake → AMOS_PLAN env → Business default.
    let entitlements = crate::plan::Plan::resolve(handshake.plan.as_deref()).entitlements();
    info!("💳 plan: {} · voice {} · web_search {}", entitlements.plan, entitlements.voice, entitlements.web_search);
    info!("🔓 session authenticated: user {} · role {} · entity {}", principal.user_id, principal.role, principal.entity_id);
    if let Some(sink) = &state.audit {
        let _ = sink
            .log(adk_auth::AuditEvent::authentication(&principal.user_id.to_string(), adk_auth::AuditOutcome::Allowed))
            .await;
    }
    let principal = Arc::new(principal);

    // Per-session store for files the user attaches in the chat. The ingest loop
    // below writes to it; the `analyze_attachment` sub-agent tool reads from it.
    let attachments = crate::subagents::new_store();

    // Fresh runner per browser session, scoped to this principal.
    let runner = match agent::build_runner(&state, principal.clone(), attachments.clone(), clock.clone(), entitlements).await {
        Ok(r) => Arc::new(r),
        Err(e) => {
            error!("Failed to build runner: {e}");
            let _ = ws_sender.send(ws::Message::Text(send_error(format!("Failed to initialize Amos: {e}")).into())).await;
            return;
        }
    };

    if let Err(e) = runner.connect().await {
        error!("Failed to connect to Gemini Live: {e}");
        let _ = ws_sender.send(ws::Message::Text(send_error(format!("Connection failed: {e}")).into())).await;
        return;
    }
    info!("✓ Connected to Gemini Live");

    let _ = ws_sender
        .send(ws::Message::Text(
            serde_json::json!({
                "type": "connected",
                "session_id": runner.session_id().await,
                "entitlements": entitlements,
            }).to_string().into(),
        ))
        .await;

    let (tx, mut rx) = mpsc::channel::<ws::Message>(64);

    // Session transcript for the end-of-session memory distillation:
    // (buffer, last_speaker) — a speaker tag is inserted when the voice
    // changes so the summarizer can follow the dialogue. Capped so a long
    // session can't grow unbounded.
    let transcript = Arc::new(tokio::sync::Mutex::new((String::new(), ' ')));
    const TRANSCRIPT_CAP: usize = 30_000;
    async fn scribe(t: &tokio::sync::Mutex<(String, char)>, speaker: char, text: &str) {
        let mut guard = t.lock().await;
        if guard.0.len() >= TRANSCRIPT_CAP {
            return;
        }
        if guard.1 != speaker {
            guard.0.push_str(if speaker == 'u' { "\n[owner]: " } else { "\n[amos]: " });
            guard.1 = speaker;
        }
        guard.0.push_str(text);
    }

    // Task: browser → Gemini (audio + control/chat messages).
    let runner_send = runner.clone();
    let transcript_send = transcript.clone();
    let tx_guard = tx.clone();
    let attachments_ingest = attachments.clone();
    let clock_ingest = clock.clone();
    let voice_enabled = entitlements.voice;
    let send_handle = tokio::spawn(async move {
        let mut voice_upsold = false;
        while let Some(Ok(msg)) = ws_receiver.next().await {
            match msg {
                ws::Message::Binary(data) => {
                    // Voice is a paid capability — drop audio on a text-only plan
                    // and upsell once (the UI hides the mic, so this is a backstop).
                    if !voice_enabled {
                        if !voice_upsold {
                            voice_upsold = true;
                            let _ = tx_guard.send(ws::Message::Text(
                                serde_json::json!({"type": "notice", "message": "Voice chat is available on the Business plan. You can keep typing to Amos here."}).to_string().into(),
                            )).await;
                        }
                        continue;
                    }
                    let audio_b64 = base64::engine::general_purpose::STANDARD.encode(&data);
                    if let Err(e) = runner_send.send_audio(&audio_b64).await {
                        warn!("send_audio failed: {e}");
                        break;
                    }
                }
                ws::Message::Text(text) => {
                    if let Ok(msg) = serde_json::from_str::<serde_json::Value>(&text) {
                        match msg.get("type").and_then(|t| t.as_str()) {
                            Some("text") => {
                                if let Some(content) = msg.get("content").and_then(|c| c.as_str()) {
                                    // Prompt-injection / exfil screen: refuse
                                    // before the turn ever reaches the model.
                                    let screen = crate::guard::screen_user_input(content);
                                    if let Some(reason) = crate::guard::fail_reason(&screen) {
                                        warn!("guardrail blocked user input");
                                        let _ = tx_guard.send(ws::Message::Text(
                                            serde_json::json!({"type": "text_delta", "content": reason}).to_string().into(),
                                        )).await;
                                        let _ = tx_guard.send(ws::Message::Text(
                                            serde_json::json!({"type": "response_done"}).to_string().into(),
                                        )).await;
                                        continue;
                                    }
                                    scribe(&transcript_send, 'u', content).await;
                                    let _ = runner_send.send_text(content).await;
                                    let _ = runner_send.create_response().await;
                                }
                            }
                            Some("attachment") => {
                                // The user attached a file (paperclip). Stash it
                                // for the analyze_attachment sub-agent, then let
                                // the model know it's available to read.
                                let name = msg.get("name").and_then(|v| v.as_str()).unwrap_or("attachment").to_string();
                                let mime = msg.get("mime").and_then(|v| v.as_str()).unwrap_or("application/octet-stream").to_string();
                                if let Some(data) = msg.get("data").and_then(|v| v.as_str()) {
                                    let att = crate::subagents::Attachment { name: name.clone(), mime_type: mime.clone(), data_b64: data.to_string() };
                                    attachments_ingest.write().await.push(att);
                                    info!("📎 received attachment {name} ({mime}, {} b64 chars)", data.len());
                                    let _ = tx_guard.send(ws::Message::Text(
                                        serde_json::json!({"type": "attachment_ack", "name": name}).to_string().into(),
                                    )).await;
                                    let _ = runner_send.send_text(&format!(
                                        "(system) The user attached a file named \"{name}\" ({mime}). \
                                         It is available to read via the analyze_attachment tool. \
                                         Call analyze_attachment when the user's request needs its contents."
                                    )).await;
                                }
                            }
                            Some("context") => {
                                // The user changed their timezone or work-as-of
                                // date mid-session; refresh the clock the
                                // current_datetime tool reads.
                                let tz = msg.get("timezone").and_then(|v| v.as_str());
                                let wd = msg.get("work_date").and_then(|v| v.as_str());
                                let updated = crate::clock::SessionClock::from_handshake(tz, wd);
                                info!("🕑 context update: tz {} · posting date {}", updated.tz.name(), updated.effective_posting_date());
                                *clock_ingest.write().await = updated;
                            }
                            Some("commit_audio") => {
                                let _ = runner_send.commit_audio().await;
                            }
                            Some("create_response") => {
                                let _ = runner_send.create_response().await;
                            }
                            Some("interrupt") => {
                                let _ = runner_send.interrupt().await;
                            }
                            _ => {}
                        }
                    }
                }
                ws::Message::Close(_) => break,
                _ => {}
            }
        }
    });

    // Task: Gemini → browser (audio, transcripts, tool events).
    let runner_recv = runner.clone();
    let tx_events = tx.clone();
    let state_recv = state.clone();
    let transcript_recv = transcript.clone();
    // Gemini Live tends to end its turn mid-workplan, narrating the next step
    // instead of calling its tool. When a turn completes with unfinished tasks,
    // nudge the model to keep executing (capped so a confused session can't loop).
    let auto_continues = Arc::new(std::sync::atomic::AtomicU8::new(12));
    let recv_handle = tokio::spawn(async move {
        loop {
            match runner_recv.next_event().await {
                Some(Ok(event)) => {
                    let ws_msg = match &event {
                        ServerEvent::AudioDelta { delta, .. } => Some(ws::Message::Binary(delta.clone().into())),
                        ServerEvent::TextDelta { delta, .. } => Some(ws::Message::Text(
                            serde_json::json!({"type": "text_delta", "content": delta}).to_string().into(),
                        )),
                        ServerEvent::TranscriptDelta { delta, .. } => {
                            scribe(&transcript_recv, 'a', delta).await;
                            Some(ws::Message::Text(
                                serde_json::json!({"type": "transcript", "content": delta}).to_string().into(),
                            ))
                        }
                        ServerEvent::InputTranscriptDelta { delta, .. } => {
                            scribe(&transcript_recv, 'u', delta).await;
                            Some(ws::Message::Text(
                                serde_json::json!({"type": "input_transcript", "content": delta}).to_string().into(),
                            ))
                        }
                        ServerEvent::SpeechStarted { .. } => Some(ws::Message::Text(
                            serde_json::json!({"type": "speech_started"}).to_string().into(),
                        )),
                        ServerEvent::SpeechStopped { .. } => Some(ws::Message::Text(
                            serde_json::json!({"type": "speech_stopped"}).to_string().into(),
                        )),
                        ServerEvent::ResponseDone { .. } => {
                            let incomplete = {
                                let tasks = state_recv.tasks.read().await;
                                !tasks.is_empty()
                                    && tasks.iter().any(|t| matches!(t.status, TaskStatus::Pending | TaskStatus::InProgress))
                            };
                            if incomplete
                                && auto_continues.fetch_update(std::sync::atomic::Ordering::SeqCst, std::sync::atomic::Ordering::SeqCst, |n| n.checked_sub(1)).is_ok()
                            {
                                let runner = runner_recv.clone();
                                let state = state_recv.clone();
                                tokio::spawn(async move {
                                    tokio::time::sleep(std::time::Duration::from_millis(2500)).await;
                                    let still_incomplete = {
                                        let tasks = state.tasks.read().await;
                                        tasks.iter().any(|t| matches!(t.status, TaskStatus::Pending | TaskStatus::InProgress))
                                    };
                                    if still_incomplete {
                                        info!("⏩ auto-continue: workplan unfinished, nudging the model");
                                        let _ = runner
                                            .send_text(
                                                "(system) Your workplan still has unfinished tasks. Continue \
                                                 executing NOW — call the next tool immediately instead of \
                                                 narrating. Exception: if you are waiting for the user to \
                                                 confirm a posting or answer a question, wait for them.",
                                            )
                                            .await;
                                        let _ = runner.create_response().await;
                                    }
                                });
                            }
                            Some(ws::Message::Text(
                                serde_json::json!({"type": "response_done"}).to_string().into(),
                            ))
                        }
                        ServerEvent::FunctionCallDone { call_id, name, arguments, .. } => {
                            // next_event() hands us raw events — tool execution is
                            // our responsibility. Dispatch off-loop so a slow tool
                            // (browser navigation, reports) never stalls audio.
                            let runner = runner_recv.clone();
                            let (call_id, tool_name, args) = (call_id.clone(), name.clone(), arguments.clone());
                            tokio::spawn(async move {
                                info!("⚙ dispatching {tool_name} ({call_id})");
                                let started = std::time::Instant::now();
                                match tokio::time::timeout(
                                    std::time::Duration::from_secs(120),
                                    runner.dispatch_tool_call(&call_id, &tool_name, &args),
                                )
                                .await
                                {
                                    Ok(Ok(())) => info!("⚙ {tool_name} done in {:.1}s", started.elapsed().as_secs_f32()),
                                    Ok(Err(e)) => warn!("tool dispatch failed for {tool_name}: {e}"),
                                    Err(_) => {
                                        warn!("tool {tool_name} timed out after 120s; sending error to model");
                                        let _ = runner
                                            .send_tool_response(adk_realtime::events::ToolResponse {
                                                call_id: call_id.clone(),
                                                output: serde_json::json!({"error": format!("{tool_name} timed out after 120s")}),
                                            })
                                            .await;
                                    }
                                }
                            });
                            Some(ws::Message::Text(
                                serde_json::json!({"type": "tool_call", "name": name, "arguments": arguments}).to_string().into(),
                            ))
                        }
                        ServerEvent::Error { error, .. } => Some(ws::Message::Text(
                            serde_json::json!({"type": "error", "message": error.message}).to_string().into(),
                        )),
                        _ => None,
                    };
                    if let Some(msg) = ws_msg {
                        if tx_events.send(msg).await.is_err() {
                            break;
                        }
                    }
                }
                Some(Err(e)) => {
                    warn!("Gemini stream error: {e}");
                    break;
                }
                None => {
                    info!("Gemini session closed");
                    break;
                }
            }
        }
    });

    // Task: panel pushes (tasks/showcase) → browser.
    let mut push_rx = state.push.subscribe();
    let tx_push = tx.clone();
    let push_handle = tokio::spawn(async move {
        while let Ok(json) = push_rx.recv().await {
            if tx_push.send(ws::Message::Text(json.into())).await.is_err() {
                break;
            }
        }
    });

    // Task: drain channel → websocket.
    let forward_handle = tokio::spawn(async move {
        while let Some(msg) = rx.recv().await {
            if ws_sender.send(msg).await.is_err() {
                break;
            }
        }
    });

    tokio::select! {
        _ = send_handle => {}
        _ = recv_handle => {}
        _ = forward_handle => {}
        _ = push_handle => {}
    }

    let _ = runner.close().await;
    info!("🔇 Amos session closed");

    // Distill the session into long-term memory (best-effort, off-thread).
    let session_transcript = transcript.lock().await.0.clone();
    crate::summarizer::spawn(state.clone(), session_transcript);
}
