//! CRM back-office routes (leads, pipeline, opportunities, activities, tickets,
//! analytics). Every data route is gated by the per-tenant CRM feature flag; the
//! settings routes stay reachable so an admin can enable the module.

use std::sync::Arc;
use axum::{extract::{Path, Query, State}, Json};
use serde::Deserialize;
use uuid::Uuid;

use crate::AppState;
use super::err_response;
use crate::middleware::auth::{AuthContext};
use zavora_erp_core::crm::*;
use zavora_erp_core::services::crm as svc;
use zavora_erp_core::ErpError;

type ApiResult = Result<Json<serde_json::Value>, axum::response::Response>;
fn er(e: ErpError) -> axum::response::Response { use axum::response::IntoResponse; err_response(e).into_response() }

/// Reject data operations when the tenant hasn't enabled CRM.
async fn gate(state: &Arc<AppState>, entity_id: Uuid) -> Result<(), axum::response::Response> {
    if svc::is_enabled(&state.engine, entity_id).await {
        Ok(())
    } else {
        Err(er(ErpError::ValidationFailed { message: "CRM module is not enabled for this workspace".into() }))
    }
}

#[derive(Deserialize)]
pub struct StatusQuery { pub status: Option<String> }
#[derive(Deserialize)]
pub struct RelatedQuery { pub related_type: Option<String>, pub related_id: Option<Uuid> }
#[derive(Deserialize)]
pub struct EnabledPatch { pub enabled: bool }
#[derive(Deserialize)]
pub struct StatusPatch { pub status: String }

// ─── Settings (always reachable; gates everything else) ──────────────────────

pub async fn get_settings(ctx: AuthContext, State(state): State<Arc<AppState>>) -> ApiResult {
    let s = svc::get_settings(&state.engine, ctx.entity_id).await.map_err(er)?;
    Ok(Json(serde_json::to_value(s).unwrap_or_default()))
}

pub async fn set_enabled(ctx: AuthContext, State(state): State<Arc<AppState>>, Json(p): Json<EnabledPatch>) -> ApiResult {
    let s = svc::set_enabled(&state.engine, ctx.entity_id, p.enabled).await.map_err(er)?;
    Ok(Json(serde_json::to_value(s).unwrap_or_default()))
}

// ─── Pipelines & stages ──────────────────────────────────────────────────────

pub async fn list_pipelines(ctx: AuthContext, State(state): State<Arc<AppState>>) -> ApiResult {
    gate(&state, ctx.entity_id).await?;
    let rows = svc::list_pipelines(&state.engine, ctx.entity_id).await.map_err(er)?;
    Ok(Json(serde_json::to_value(rows).unwrap_or_default()))
}

pub async fn list_stages(ctx: AuthContext, State(state): State<Arc<AppState>>, Path(pipeline_id): Path<Uuid>) -> ApiResult {
    gate(&state, ctx.entity_id).await?;
    let rows = svc::list_stages(&state.engine, ctx.entity_id, pipeline_id).await.map_err(er)?;
    Ok(Json(serde_json::to_value(rows).unwrap_or_default()))
}

// ─── Leads ───────────────────────────────────────────────────────────────────

pub async fn list_leads(ctx: AuthContext, State(state): State<Arc<AppState>>, Query(q): Query<StatusQuery>) -> ApiResult {
    gate(&state, ctx.entity_id).await?;
    let rows = svc::list_leads(&state.engine, ctx.entity_id, q.status).await.map_err(er)?;
    Ok(Json(serde_json::to_value(rows).unwrap_or_default()))
}

pub async fn create_lead(ctx: AuthContext, State(state): State<Arc<AppState>>, Json(req): Json<CreateLeadRequest>) -> ApiResult {
    gate(&state, ctx.entity_id).await?;
    let id = svc::create_lead(&state.engine, ctx.entity_id, req).await.map_err(er)?;
    Ok(Json(serde_json::json!({ "id": id })))
}

pub async fn update_lead(ctx: AuthContext, State(state): State<Arc<AppState>>, Path(id): Path<Uuid>, Json(req): Json<UpdateLeadRequest>) -> ApiResult {
    gate(&state, ctx.entity_id).await?;
    svc::update_lead(&state.engine, ctx.entity_id, id, req).await.map_err(er)?;
    Ok(Json(serde_json::json!({ "status": "ok" })))
}

pub async fn convert_lead(ctx: AuthContext, State(state): State<Arc<AppState>>, Path(id): Path<Uuid>, Json(req): Json<ConvertLeadRequest>) -> ApiResult {
    gate(&state, ctx.entity_id).await?;
    let res = svc::convert_lead(&state.engine, ctx.entity_id, id, req).await.map_err(er)?;
    Ok(Json(res))
}

// ─── Opportunities ───────────────────────────────────────────────────────────

pub async fn list_opportunities(ctx: AuthContext, State(state): State<Arc<AppState>>, Query(q): Query<StatusQuery>) -> ApiResult {
    gate(&state, ctx.entity_id).await?;
    let rows = svc::list_opportunities(&state.engine, ctx.entity_id, q.status).await.map_err(er)?;
    Ok(Json(serde_json::to_value(rows).unwrap_or_default()))
}

pub async fn create_opportunity(ctx: AuthContext, State(state): State<Arc<AppState>>, Json(req): Json<CreateOpportunityRequest>) -> ApiResult {
    gate(&state, ctx.entity_id).await?;
    let id = svc::create_opportunity(&state.engine, ctx.entity_id, req).await.map_err(er)?;
    Ok(Json(serde_json::json!({ "id": id })))
}

pub async fn move_opportunity(ctx: AuthContext, State(state): State<Arc<AppState>>, Path(id): Path<Uuid>, Json(req): Json<MoveOpportunityRequest>) -> ApiResult {
    gate(&state, ctx.entity_id).await?;
    svc::move_opportunity(&state.engine, ctx.entity_id, id, Some(ctx.user_id), req).await.map_err(er)?;
    Ok(Json(serde_json::json!({ "status": "ok" })))
}

pub async fn win_opportunity(ctx: AuthContext, State(state): State<Arc<AppState>>, Path(id): Path<Uuid>) -> ApiResult {
    gate(&state, ctx.entity_id).await?;
    svc::close_opportunity(&state.engine, ctx.entity_id, id, Some(ctx.user_id), true, None).await.map_err(er)?;
    Ok(Json(serde_json::json!({ "status": "won" })))
}

pub async fn lose_opportunity(ctx: AuthContext, State(state): State<Arc<AppState>>, Path(id): Path<Uuid>, Json(req): Json<LoseOpportunityRequest>) -> ApiResult {
    gate(&state, ctx.entity_id).await?;
    svc::close_opportunity(&state.engine, ctx.entity_id, id, Some(ctx.user_id), false, req.reason).await.map_err(er)?;
    Ok(Json(serde_json::json!({ "status": "lost" })))
}

// ─── Activities ──────────────────────────────────────────────────────────────

pub async fn list_activities(ctx: AuthContext, State(state): State<Arc<AppState>>, Query(q): Query<RelatedQuery>) -> ApiResult {
    gate(&state, ctx.entity_id).await?;
    let rows = svc::list_activities(&state.engine, ctx.entity_id, q.related_type, q.related_id).await.map_err(er)?;
    Ok(Json(serde_json::to_value(rows).unwrap_or_default()))
}

pub async fn create_activity(ctx: AuthContext, State(state): State<Arc<AppState>>, Json(req): Json<CreateActivityRequest>) -> ApiResult {
    gate(&state, ctx.entity_id).await?;
    let id = svc::create_activity(&state.engine, ctx.entity_id, req).await.map_err(er)?;
    Ok(Json(serde_json::json!({ "id": id })))
}

pub async fn complete_activity(ctx: AuthContext, State(state): State<Arc<AppState>>, Path(id): Path<Uuid>) -> ApiResult {
    gate(&state, ctx.entity_id).await?;
    svc::set_activity_done(&state.engine, ctx.entity_id, id, true).await.map_err(er)?;
    Ok(Json(serde_json::json!({ "status": "done" })))
}

// ─── Tickets (staff) ─────────────────────────────────────────────────────────

pub async fn list_tickets(ctx: AuthContext, State(state): State<Arc<AppState>>, Query(q): Query<StatusQuery>) -> ApiResult {
    gate(&state, ctx.entity_id).await?;
    let rows = svc::list_tickets(&state.engine, ctx.entity_id, q.status).await.map_err(er)?;
    Ok(Json(serde_json::to_value(rows).unwrap_or_default()))
}

pub async fn get_ticket(ctx: AuthContext, State(state): State<Arc<AppState>>, Path(id): Path<Uuid>) -> ApiResult {
    gate(&state, ctx.entity_id).await?;
    let v = svc::get_ticket(&state.engine, ctx.entity_id, id).await.map_err(er)?;
    Ok(Json(v))
}

pub async fn reply_ticket(ctx: AuthContext, State(state): State<Arc<AppState>>, Path(id): Path<Uuid>, Json(req): Json<TicketReplyRequest>) -> ApiResult {
    gate(&state, ctx.entity_id).await?;
    svc::reply_ticket(&state.engine, ctx.entity_id, id, "staff", Some(ctx.user_id), &req.body).await.map_err(er)?;
    Ok(Json(serde_json::json!({ "status": "ok" })))
}

pub async fn set_ticket_status(ctx: AuthContext, State(state): State<Arc<AppState>>, Path(id): Path<Uuid>, Json(p): Json<StatusPatch>) -> ApiResult {
    gate(&state, ctx.entity_id).await?;
    svc::set_ticket_status(&state.engine, ctx.entity_id, id, &p.status).await.map_err(er)?;
    Ok(Json(serde_json::json!({ "status": "ok" })))
}

// ─── Analytics ───────────────────────────────────────────────────────────────

pub async fn analytics(ctx: AuthContext, State(state): State<Arc<AppState>>) -> ApiResult {
    gate(&state, ctx.entity_id).await?;
    let v = svc::analytics(&state.engine, ctx.entity_id).await.map_err(er)?;
    Ok(Json(v))
}

// ─── Assisted onboarding: invite a customer to the portal ───────────────────

pub async fn invite_customer(ctx: AuthContext, State(state): State<Arc<AppState>>, Json(req): Json<InviteCustomerRequest>) -> ApiResult {
    gate(&state, ctx.entity_id).await?;

    let email = req.email.trim().to_lowercase();
    let display_name = req.display_name.clone().unwrap_or_else(|| email.clone());
    let (password_hash, status): (Option<String>, &str) = match req.password.as_deref() {
        Some(pw) if pw.len() >= 8 => (Some(zavora_erp_core::auth::hash_password(pw).map_err(er)?), "active"),
        Some(_) => return Err(er(ErpError::ValidationFailed { message: "Password must be at least 8 characters".into() })),
        None => (None, "invited"),
    };

    let existing: Option<Uuid> = sqlx::query_scalar(
        "SELECT id FROM customer_users WHERE entity_id = $1 AND lower(email) = lower($2)",
    )
    .bind(ctx.entity_id).bind(&email).fetch_optional(state.engine.pool()).await.map_err(|e| er(ErpError::Database(e)))?;

    let cust_id = match existing {
        Some(id) => {
            sqlx::query("UPDATE customer_users SET customer_id = COALESCE($1, customer_id), status = $2, password_hash = COALESCE($3, password_hash) WHERE id = $4")
                .bind(req.customer_id).bind(status).bind(&password_hash).bind(id)
                .execute(state.engine.pool()).await.map_err(|e| er(ErpError::Database(e)))?;
            id
        }
        None => {
            let id = Uuid::new_v4();
            sqlx::query("INSERT INTO customer_users (id, entity_id, email, display_name, password_hash, status, customer_id) VALUES ($1,$2,$3,$4,$5,$6,$7)")
                .bind(id).bind(ctx.entity_id).bind(&email).bind(&display_name).bind(&password_hash).bind(status).bind(req.customer_id)
                .execute(state.engine.pool()).await.map_err(|e| er(ErpError::Database(e)))?;
            id
        }
    };

    // Invited (no password): issue a single-use set-password token + email link.
    if password_hash.is_none() {
        let token = Uuid::new_v4().to_string();
        sqlx::query("UPDATE customer_users SET set_token = $1, set_token_expires = NOW() + INTERVAL '7 days' WHERE id = $2")
            .bind(&token).bind(cust_id).execute(state.engine.pool()).await.map_err(|e| er(ErpError::Database(e)))?;
        let base = std::env::var("APP_BASE_URL").unwrap_or_else(|_| "http://localhost:3000".into());
        let link = format!("{base}/customerportal/set-password?token={token}");
        let email_req = zavora_erp_core::notifications::SendNotificationRequest {
            event_type: zavora_erp_core::notifications::NotificationEventType::LeaveRequestDecided,
            channels: vec![zavora_erp_core::types::Channel::Email],
            recipients: vec![email.clone()],
            subject: Some("You've been invited to the customer portal".into()),
            body: format!("Set your password to access your account: {link}\n\nThis link expires in 7 days."),
            related_type: Some("customer_user".into()), related_id: Some(cust_id), schedule_at: None, attachments: Vec::new(),
        };
        let _ = zavora_erp_core::services::notifications::send_notification(&state.engine, ctx.entity_id, email_req).await;
    }

    Ok(Json(serde_json::json!({ "customer_user_id": cust_id, "email": email, "status": status })))
}
