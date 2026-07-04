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

async fn handle_ws(socket: ws::WebSocket, state: Arc<AppState>) {
    use adk_realtime::events::ServerEvent;
    use base64::Engine as _;
    use futures::{SinkExt, StreamExt};

    info!("🎙️ Amos session connecting…");
    let (mut ws_sender, mut ws_receiver) = socket.split();

    let send_error = |msg: String| serde_json::json!({"type": "error", "message": msg}).to_string();

    // Fresh runner per browser session.
    let runner = match agent::build_runner(&state).await {
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
            serde_json::json!({"type": "connected", "session_id": runner.session_id().await}).to_string().into(),
        ))
        .await;

    let (tx, mut rx) = mpsc::channel::<ws::Message>(64);

    // Task: browser → Gemini (audio + control/chat messages).
    let runner_send = runner.clone();
    let send_handle = tokio::spawn(async move {
        while let Some(Ok(msg)) = ws_receiver.next().await {
            match msg {
                ws::Message::Binary(data) => {
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
                                    let _ = runner_send.send_text(content).await;
                                    let _ = runner_send.create_response().await;
                                }
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
                        ServerEvent::TranscriptDelta { delta, .. } => Some(ws::Message::Text(
                            serde_json::json!({"type": "transcript", "content": delta}).to_string().into(),
                        )),
                        ServerEvent::InputTranscriptDelta { delta, .. } => Some(ws::Message::Text(
                            serde_json::json!({"type": "input_transcript", "content": delta}).to_string().into(),
                        )),
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
}
