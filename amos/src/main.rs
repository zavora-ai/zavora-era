//! Amos AI — Your personal AI accountant for Zavora ERA.
//!
//! Realtime voice + chat agent (Gemini Live) with ERP tools (mcp-erp, zavora
//! backend) and a Playwright-driven browser for showcasing work in the ERP UI.
//!
//! ```bash
//! # amos/.env: GOOGLE_API_KEY, ZAVORA_API_URL, ZAVORA_EMAIL, ZAVORA_PASSWORD
//! cargo run -p amos
//! # open http://localhost:8090
//! ```

mod agent;
mod audit;
mod auth;
mod config;
mod erp;
mod guard;
mod mcp;
mod memory;
mod scope;
mod persona;
mod routes;
mod skills;
mod state;
mod summarizer;

use crate::state::AppState;
use anyhow::Result;
use std::sync::Arc;
use tracing::info;

#[tokio::main]
async fn main() -> Result<()> {
    rustls::crypto::aws_lc_rs::default_provider()
        .install_default()
        .expect("Failed to install rustls crypto provider");

    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info,amos=debug".into()),
        )
        .init();

    // Load amos/.env regardless of the workspace cwd.
    let _ = dotenvy::from_path(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(".env"));
    let _ = dotenvy::dotenv();

    info!("🧮 Amos AI — Your personal AI accountant. Starting up…");

    let showcase_dir = std::env::var("AMOS_SHOWCASE_DIR")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|_| std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("showcase"));
    std::fs::create_dir_all(&showcase_dir)?;

    // Resolve the single tenant this Amos serves — the only entity any user is
    // allowed to access here. Explicit env wins; otherwise derive from the
    // service account's own tenant.
    let served_entity = resolve_served_entity().await?;
    info!("🔒 Serving entity {served_entity} (sessions for other tenants are refused)");

    let manager = mcp::start_manager(&showcase_dir).await?;
    info!("✓ MCP servers started (erp + browser)");

    let memory = memory::AmosMemory::connect(served_entity).await;

    // Optional Postgres audit trail (auth + tool-access), sharing the ERP DB.
    let audit = build_audit_sink(served_entity).await;

    let state = Arc::new(AppState::new(manager, memory, served_entity, audit)?);
    info!("✓ Memory online ({}) · audit {}", state.memory.backend,
        if state.audit.is_some() { "on" } else { "off" });
    let app = routes::create_router(state.clone());

    let port = std::env::var("AMOS_PORT").unwrap_or_else(|_| "8090".to_string());
    let addr = format!("0.0.0.0:{port}");
    let listener = tokio::net::TcpListener::bind(&addr).await?;

    info!("🌐 Amos listening on http://localhost:{port}");
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await?;

    // Reap MCP children (mcp-erp, Playwright + its Chromium) — otherwise every
    // restart leaks orphaned processes that poison later browser sessions.
    info!("Shutting down MCP servers…");
    let _ = state.manager.shutdown().await;
    Ok(())
}

/// The entity this Amos serves: `AMOS_SERVED_ENTITY_ID` if set, else derived
/// from the service account's own tenant.
async fn resolve_served_entity() -> Result<uuid::Uuid> {
    if let Ok(id) = std::env::var("AMOS_SERVED_ENTITY_ID") {
        return id.parse().map_err(|_| anyhow::anyhow!("AMOS_SERVED_ENTITY_ID is not a valid UUID"));
    }
    erp::ErpClient::from_env()?.resolve_entity().await
}

/// Amos audit sink over the ERP database, if reachable. Best-effort: a missing
/// sink just means no audit rows, never a boot failure.
async fn build_audit_sink(served_entity: uuid::Uuid) -> Option<Arc<dyn adk_auth::AuditSink>> {
    let url = std::env::var("AMOS_MEMORY_DATABASE_URL")
        .or_else(|_| std::env::var("AMOS_AUDIT_DATABASE_URL"))
        .unwrap_or_else(|_| "postgres://zavora:zavora@localhost:5433/zavora_era".to_string());
    audit::AmosAuditSink::connect(&url, served_entity)
        .await
        .map(|s| Arc::new(s) as Arc<dyn adk_auth::AuditSink>)
}

async fn shutdown_signal() {
    let ctrl_c = async {
        let _ = tokio::signal::ctrl_c().await;
    };
    let terminate = async {
        if let Ok(mut sig) = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()) {
            sig.recv().await;
        }
    };
    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }
}
