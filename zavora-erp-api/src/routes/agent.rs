use axum::{extract::State, Json};
use std::sync::Arc;

use crate::AppState;
use crate::middleware::auth::{AuthContext, require_permission};
use super::err_response;
use zavora_erp_core::{PostingRequest, reporting::ReportRequest};

fn bind_posting_actor(req: &mut PostingRequest, user_id: uuid::Uuid) {
    req.posted_by = zavora_erp_core::AgentOrUserId::User(user_id);
}

fn bind_report_tenant(req: &mut ReportRequest, entity_id: uuid::Uuid) {
    req.entity_id = entity_id;
}

/// Agent posting endpoint — spec section 27.
///
/// The agentic layer authenticates with the same identity headers as any other client.
/// Posting to the GL requires a journal-posting role.
pub async fn post_from_agent(
    ctx: AuthContext,
    State(state): State<Arc<AppState>>,
    Json(mut req): Json<PostingRequest>,
) -> Result<Json<serde_json::Value>, impl axum::response::IntoResponse> {
    bind_posting_actor(&mut req, ctx.user_id);
    match state.engine.post_from_agent_for(ctx.entity_id, req).await {
        Ok(result) => Ok(Json(serde_json::to_value(result).unwrap_or_default())),
        Err(e) => Err(err_response(e)),
    }
}

/// Agent report endpoint — spec section 27.
pub async fn run_report(
    ctx: AuthContext,
    State(state): State<Arc<AppState>>,
    Json(mut req): Json<ReportRequest>,
) -> Result<Json<serde_json::Value>, impl axum::response::IntoResponse> {
    bind_report_tenant(&mut req, ctx.entity_id);
    let permission = if super::reports::is_payroll_report(&req.report_type) { "pay_run.read" } else { "report.read" };
    require_permission(&state, &ctx, permission).await.map_err(err_response)?;
    match state.engine.run_report(req).await {
        Ok(data) => Ok(Json(serde_json::to_value(data).unwrap_or_default())),
        Err(e) => Err(err_response(e)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::NaiveDate;
    use zavora_erp_core::{AgentOrUserId, ledger::{CreateJournalEntryRequest, JournalSource}, reporting::{ReportParameters, ReportType}};

    #[test]
    fn caller_identity_replaces_forged_report_tenant_and_posting_actor() {
        let caller = uuid::Uuid::new_v4();
        let tenant = uuid::Uuid::new_v4();
        let mut report = ReportRequest { entity_id: uuid::Uuid::new_v4(), report_type: ReportType::TrialBalance, parameters: ReportParameters::default() };
        bind_report_tenant(&mut report, tenant);
        assert_eq!(report.entity_id, tenant);

        let mut posting = PostingRequest {
            entry: CreateJournalEntryRequest {
                date: NaiveDate::from_ymd_opt(2026, 8, 9).unwrap(), source: JournalSource::Agent("amos".into()),
                reference: "security-test".into(), description: "security-test".into(), source_id: None, lines: vec![], post_immediately: true,
            },
            posted_by: AgentOrUserId::User(uuid::Uuid::new_v4()),
        };
        bind_posting_actor(&mut posting, caller);
        assert_eq!(posting.posted_by, AgentOrUserId::User(caller));
    }
}
