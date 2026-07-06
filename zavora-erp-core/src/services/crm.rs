//! CRM feature-flag + bootstrap service. CRM is an optional, per-tenant add-in:
//! everything is gated by `crm_settings.enabled` (default false). Enabling seeds
//! a default pipeline with stages. Business logic for leads/opportunities/etc.
//! is added in later phases.

use uuid::Uuid;

use crate::crm::*;
use crate::engine::ErpEngine;
use crate::error::ErpResult;

/// Read a tenant's CRM settings, returning a disabled default if none stored.
pub async fn get_settings(engine: &ErpEngine, entity_id: Uuid) -> ErpResult<CrmSettingsRow> {
    let row = sqlx::query_as::<_, CrmSettingsRow>(
        "SELECT * FROM crm_settings WHERE entity_id = $1",
    )
    .bind(entity_id)
    .fetch_optional(engine.pool())
    .await?;
    Ok(row.unwrap_or(CrmSettingsRow {
        entity_id,
        enabled: false,
        default_pipeline_id: None,
        updated_at: chrono::Utc::now(),
    }))
}

/// Whether CRM is enabled for a tenant (gates all CRM/portal routes).
pub async fn is_enabled(engine: &ErpEngine, entity_id: Uuid) -> bool {
    sqlx::query_scalar::<_, bool>("SELECT enabled FROM crm_settings WHERE entity_id = $1")
        .bind(entity_id)
        .fetch_optional(engine.pool())
        .await
        .ok()
        .flatten()
        .unwrap_or(false)
}

/// Enable or disable CRM for a tenant. On first enable, seed a default pipeline.
pub async fn set_enabled(engine: &ErpEngine, entity_id: Uuid, enabled: bool) -> ErpResult<CrmSettingsRow> {
    sqlx::query(
        "INSERT INTO crm_settings (entity_id, enabled, updated_at) VALUES ($1, $2, NOW()) \
         ON CONFLICT (entity_id) DO UPDATE SET enabled = EXCLUDED.enabled, updated_at = NOW()",
    )
    .bind(entity_id)
    .bind(enabled)
    .execute(engine.pool())
    .await?;

    if enabled {
        ensure_default_pipeline(engine, entity_id).await?;
    }
    get_settings(engine, entity_id).await
}

/// Seed a default pipeline + stages if the tenant has none, and point
/// `crm_settings.default_pipeline_id` at it. Idempotent.
pub async fn ensure_default_pipeline(engine: &ErpEngine, entity_id: Uuid) -> ErpResult<Uuid> {
    // Existing default?
    if let Some(id) = sqlx::query_scalar::<_, Uuid>(
        "SELECT id FROM crm_pipelines WHERE entity_id = $1 ORDER BY is_default DESC, created_at LIMIT 1",
    )
    .bind(entity_id)
    .fetch_optional(engine.pool())
    .await?
    {
        return Ok(id);
    }

    let pipeline_id = Uuid::new_v4();
    let mut tx = engine.pool().begin().await?;
    sqlx::query(
        "INSERT INTO crm_pipelines (id, entity_id, name, is_default) VALUES ($1, $2, 'Sales Pipeline', true)",
    )
    .bind(pipeline_id)
    .bind(entity_id)
    .execute(&mut *tx)
    .await?;
    for s in default_pipeline_stages() {
        sqlx::query(
            "INSERT INTO crm_stages (id, entity_id, pipeline_id, name, sort_order, probability, is_won, is_lost) \
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8)",
        )
        .bind(Uuid::new_v4())
        .bind(entity_id)
        .bind(pipeline_id)
        .bind(&s.name)
        .bind(s.sort_order)
        .bind(s.probability)
        .bind(s.is_won)
        .bind(s.is_lost)
        .execute(&mut *tx)
        .await?;
    }
    sqlx::query(
        "INSERT INTO crm_settings (entity_id, enabled, default_pipeline_id, updated_at) \
         VALUES ($1, true, $2, NOW()) \
         ON CONFLICT (entity_id) DO UPDATE SET default_pipeline_id = EXCLUDED.default_pipeline_id, updated_at = NOW()",
    )
    .bind(entity_id)
    .bind(pipeline_id)
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(pipeline_id)
}

// ═══ Pipelines & stages ══════════════════════════════════════════════════════

pub async fn list_pipelines(engine: &ErpEngine, entity_id: Uuid) -> ErpResult<Vec<PipelineRow>> {
    Ok(sqlx::query_as::<_, PipelineRow>(
        "SELECT * FROM crm_pipelines WHERE entity_id = $1 ORDER BY is_default DESC, created_at",
    ).bind(entity_id).fetch_all(engine.pool()).await?)
}

pub async fn list_stages(engine: &ErpEngine, entity_id: Uuid, pipeline_id: Uuid) -> ErpResult<Vec<StageRow>> {
    Ok(sqlx::query_as::<_, StageRow>(
        "SELECT * FROM crm_stages WHERE entity_id = $1 AND pipeline_id = $2 ORDER BY sort_order",
    ).bind(entity_id).bind(pipeline_id).fetch_all(engine.pool()).await?)
}

/// First non-won/non-lost stage of a pipeline (the entry stage).
async fn entry_stage(engine: &ErpEngine, entity_id: Uuid, pipeline_id: Uuid) -> ErpResult<StageRow> {
    sqlx::query_as::<_, StageRow>(
        "SELECT * FROM crm_stages WHERE entity_id = $1 AND pipeline_id = $2 AND NOT is_won AND NOT is_lost \
         ORDER BY sort_order LIMIT 1",
    ).bind(entity_id).bind(pipeline_id).fetch_optional(engine.pool()).await?
    .ok_or_else(|| crate::error::ErpError::ValidationFailed { message: "Pipeline has no open stage".into() })
}

// ═══ Leads ═══════════════════════════════════════════════════════════════════

pub async fn list_leads(engine: &ErpEngine, entity_id: Uuid, status: Option<String>) -> ErpResult<Vec<LeadRow>> {
    Ok(sqlx::query_as::<_, LeadRow>(
        "SELECT * FROM crm_leads WHERE entity_id = $1 AND ($2::text IS NULL OR status = $2) ORDER BY created_at DESC",
    ).bind(entity_id).bind(status).fetch_all(engine.pool()).await?)
}

pub async fn create_lead(engine: &ErpEngine, entity_id: Uuid, req: CreateLeadRequest) -> ErpResult<Uuid> {
    let id = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO crm_leads (id, entity_id, name, company, email, phone, source, rating, owner_user_id, notes) \
         VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10)",
    )
    .bind(id).bind(entity_id).bind(&req.name).bind(&req.company).bind(&req.email).bind(&req.phone)
    .bind(&req.source).bind(&req.rating).bind(req.owner_user_id).bind(&req.notes)
    .execute(engine.pool()).await?;
    Ok(id)
}

pub async fn update_lead(engine: &ErpEngine, entity_id: Uuid, id: Uuid, req: UpdateLeadRequest) -> ErpResult<()> {
    sqlx::query(
        "UPDATE crm_leads SET \
           name = COALESCE($3, name), company = COALESCE($4, company), email = COALESCE($5, email), \
           phone = COALESCE($6, phone), source = COALESCE($7, source), status = COALESCE($8, status), \
           rating = COALESCE($9, rating), owner_user_id = COALESCE($10, owner_user_id), \
           notes = COALESCE($11, notes), updated_at = NOW() \
         WHERE id = $1 AND entity_id = $2",
    )
    .bind(id).bind(entity_id).bind(&req.name).bind(&req.company).bind(&req.email).bind(&req.phone)
    .bind(&req.source).bind(&req.status).bind(&req.rating).bind(req.owner_user_id).bind(&req.notes)
    .execute(engine.pool()).await?;
    Ok(())
}

/// Convert a lead → mark Converted, link a customer (if provided), and open an
/// opportunity in the default pipeline's entry stage.
pub async fn convert_lead(engine: &ErpEngine, entity_id: Uuid, id: Uuid, req: ConvertLeadRequest) -> ErpResult<serde_json::Value> {
    let lead = sqlx::query_as::<_, LeadRow>("SELECT * FROM crm_leads WHERE id=$1 AND entity_id=$2")
        .bind(id).bind(entity_id).fetch_optional(engine.pool()).await?
        .ok_or_else(|| crate::error::ErpError::NotFound { entity_type: "Lead".into(), id })?;

    let pipeline_id = match req.pipeline_id {
        Some(p) => p,
        None => ensure_default_pipeline(engine, entity_id).await?,
    };
    let stage = entry_stage(engine, entity_id, pipeline_id).await?;

    let opp_id = Uuid::new_v4();
    let opp_name = req.opportunity_name.clone().unwrap_or_else(|| format!("{} opportunity", lead.name));
    sqlx::query(
        "INSERT INTO crm_opportunities (id, entity_id, name, pipeline_id, stage_id, customer_id, lead_id, amount, probability, owner_user_id) \
         VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10)",
    )
    .bind(opp_id).bind(entity_id).bind(&opp_name).bind(pipeline_id).bind(stage.id)
    .bind(req.customer_id).bind(lead.id).bind(req.amount.unwrap_or(rust_decimal::Decimal::ZERO))
    .bind(stage.probability).bind(lead.owner_user_id)
    .execute(engine.pool()).await?;

    sqlx::query(
        "UPDATE crm_leads SET status='Converted', converted_customer_id=$3, converted_opportunity_id=$4, updated_at=NOW() \
         WHERE id=$1 AND entity_id=$2",
    )
    .bind(id).bind(entity_id).bind(req.customer_id).bind(opp_id)
    .execute(engine.pool()).await?;

    Ok(serde_json::json!({ "lead_id": id, "opportunity_id": opp_id, "customer_id": req.customer_id }))
}

// ═══ Opportunities ═══════════════════════════════════════════════════════════

pub async fn list_opportunities(engine: &ErpEngine, entity_id: Uuid, status: Option<String>) -> ErpResult<Vec<OpportunityRow>> {
    Ok(sqlx::query_as::<_, OpportunityRow>(
        "SELECT * FROM crm_opportunities WHERE entity_id = $1 AND ($2::text IS NULL OR status = $2) ORDER BY created_at DESC",
    ).bind(entity_id).bind(status).fetch_all(engine.pool()).await?)
}

pub async fn create_opportunity(engine: &ErpEngine, entity_id: Uuid, req: CreateOpportunityRequest) -> ErpResult<Uuid> {
    let pipeline_id = match req.pipeline_id {
        Some(p) => p,
        None => ensure_default_pipeline(engine, entity_id).await?,
    };
    let (stage_id, probability) = match req.stage_id {
        Some(sid) => {
            let p = sqlx::query_scalar::<_, rust_decimal::Decimal>("SELECT probability FROM crm_stages WHERE id=$1 AND entity_id=$2")
                .bind(sid).bind(entity_id).fetch_optional(engine.pool()).await?.unwrap_or(rust_decimal::Decimal::ZERO);
            (sid, p)
        }
        None => { let s = entry_stage(engine, entity_id, pipeline_id).await?; (s.id, s.probability) }
    };
    let id = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO crm_opportunities (id, entity_id, name, pipeline_id, stage_id, customer_id, lead_id, amount, currency, expected_close_date, probability, owner_user_id) \
         VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12)",
    )
    .bind(id).bind(entity_id).bind(&req.name).bind(pipeline_id).bind(stage_id).bind(req.customer_id)
    .bind(req.lead_id).bind(req.amount).bind(&req.currency).bind(req.expected_close_date).bind(probability).bind(req.owner_user_id)
    .execute(engine.pool()).await?;
    Ok(id)
}

/// Move an opportunity to a stage: updates probability, records an event, and
/// auto-marks Won/Lost when the target stage is a won/lost stage.
pub async fn move_opportunity(engine: &ErpEngine, entity_id: Uuid, id: Uuid, actor: Option<Uuid>, req: MoveOpportunityRequest) -> ErpResult<()> {
    let from_stage = sqlx::query_scalar::<_, Uuid>("SELECT stage_id FROM crm_opportunities WHERE id=$1 AND entity_id=$2")
        .bind(id).bind(entity_id).fetch_optional(engine.pool()).await?
        .ok_or_else(|| crate::error::ErpError::NotFound { entity_type: "Opportunity".into(), id })?;
    let stage = sqlx::query_as::<_, StageRow>("SELECT * FROM crm_stages WHERE id=$1 AND entity_id=$2")
        .bind(req.stage_id).bind(entity_id).fetch_optional(engine.pool()).await?
        .ok_or_else(|| crate::error::ErpError::NotFound { entity_type: "Stage".into(), id: req.stage_id })?;

    let status = if stage.is_won { "Won" } else if stage.is_lost { "Lost" } else { "Open" };
    let mut tx = engine.pool().begin().await?;
    sqlx::query(
        "UPDATE crm_opportunities SET stage_id=$3, probability=$4, status=$5, \
           closed_at = CASE WHEN $5 IN ('Won','Lost') THEN NOW() ELSE NULL END \
         WHERE id=$1 AND entity_id=$2",
    )
    .bind(id).bind(entity_id).bind(stage.id).bind(stage.probability).bind(status)
    .execute(&mut *tx).await?;
    sqlx::query(
        "INSERT INTO crm_opportunity_events (id, entity_id, opportunity_id, from_stage, to_stage, note, actor_id) \
         VALUES ($1,$2,$3,$4,$5,$6,$7)",
    )
    .bind(Uuid::new_v4()).bind(entity_id).bind(id).bind(from_stage).bind(stage.id).bind(&req.note).bind(actor)
    .execute(&mut *tx).await?;
    tx.commit().await?;
    Ok(())
}

/// Close an opportunity Won or Lost (moves to the pipeline's won/lost stage).
pub async fn close_opportunity(engine: &ErpEngine, entity_id: Uuid, id: Uuid, actor: Option<Uuid>, won: bool, reason: Option<String>) -> ErpResult<()> {
    let (pipeline_id, from_stage) = sqlx::query_as::<_, (Uuid, Uuid)>(
        "SELECT pipeline_id, stage_id FROM crm_opportunities WHERE id=$1 AND entity_id=$2",
    ).bind(id).bind(entity_id).fetch_optional(engine.pool()).await?
    .ok_or_else(|| crate::error::ErpError::NotFound { entity_type: "Opportunity".into(), id })?;
    let target = sqlx::query_as::<_, StageRow>(
        "SELECT * FROM crm_stages WHERE entity_id=$1 AND pipeline_id=$2 AND is_won=$3 AND is_lost=$4 ORDER BY sort_order LIMIT 1",
    ).bind(entity_id).bind(pipeline_id).bind(won).bind(!won).fetch_optional(engine.pool()).await?;

    let (stage_id, probability) = match &target {
        Some(s) => (s.id, s.probability),
        None => (from_stage, if won { rust_decimal_macros::dec!(100) } else { rust_decimal::Decimal::ZERO }),
    };
    let status = if won { "Won" } else { "Lost" };
    let mut tx = engine.pool().begin().await?;
    sqlx::query(
        "UPDATE crm_opportunities SET stage_id=$3, probability=$4, status=$5, lost_reason=$6, closed_at=NOW() WHERE id=$1 AND entity_id=$2",
    )
    .bind(id).bind(entity_id).bind(stage_id).bind(probability).bind(status).bind(if won { None } else { reason.clone() })
    .execute(&mut *tx).await?;
    sqlx::query(
        "INSERT INTO crm_opportunity_events (id, entity_id, opportunity_id, from_stage, to_stage, note, actor_id) VALUES ($1,$2,$3,$4,$5,$6,$7)",
    )
    .bind(Uuid::new_v4()).bind(entity_id).bind(id).bind(from_stage).bind(stage_id)
    .bind(reason.or_else(|| Some(status.to_string()))).bind(actor)
    .execute(&mut *tx).await?;
    tx.commit().await?;
    Ok(())
}

// ═══ Activities ══════════════════════════════════════════════════════════════

pub async fn list_activities(engine: &ErpEngine, entity_id: Uuid, related_type: Option<String>, related_id: Option<Uuid>) -> ErpResult<Vec<ActivityRow>> {
    Ok(sqlx::query_as::<_, ActivityRow>(
        "SELECT * FROM crm_activities WHERE entity_id=$1 \
           AND ($2::text IS NULL OR related_type=$2) AND ($3::uuid IS NULL OR related_id=$3) \
         ORDER BY done, COALESCE(due_date, created_at) DESC",
    ).bind(entity_id).bind(related_type).bind(related_id).fetch_all(engine.pool()).await?)
}

pub async fn create_activity(engine: &ErpEngine, entity_id: Uuid, req: CreateActivityRequest) -> ErpResult<Uuid> {
    let id = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO crm_activities (id, entity_id, kind, subject, notes, due_date, related_type, related_id, owner_user_id) \
         VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9)",
    )
    .bind(id).bind(entity_id).bind(&req.kind).bind(&req.subject).bind(&req.notes).bind(req.due_date)
    .bind(&req.related_type).bind(req.related_id).bind(req.owner_user_id)
    .execute(engine.pool()).await?;
    Ok(id)
}

pub async fn set_activity_done(engine: &ErpEngine, entity_id: Uuid, id: Uuid, done: bool) -> ErpResult<()> {
    sqlx::query("UPDATE crm_activities SET done=$3, done_at=CASE WHEN $3 THEN NOW() ELSE NULL END WHERE id=$1 AND entity_id=$2")
        .bind(id).bind(entity_id).bind(done).execute(engine.pool()).await?;
    Ok(())
}

// ═══ Tickets (staff side) ════════════════════════════════════════════════════

pub async fn list_tickets(engine: &ErpEngine, entity_id: Uuid, status: Option<String>) -> ErpResult<Vec<TicketRow>> {
    Ok(sqlx::query_as::<_, TicketRow>(
        "SELECT * FROM crm_tickets WHERE entity_id=$1 AND ($2::text IS NULL OR status=$2) ORDER BY updated_at DESC",
    ).bind(entity_id).bind(status).fetch_all(engine.pool()).await?)
}

pub async fn get_ticket(engine: &ErpEngine, entity_id: Uuid, id: Uuid) -> ErpResult<serde_json::Value> {
    let ticket = sqlx::query_as::<_, TicketRow>("SELECT * FROM crm_tickets WHERE id=$1 AND entity_id=$2")
        .bind(id).bind(entity_id).fetch_optional(engine.pool()).await?
        .ok_or_else(|| crate::error::ErpError::NotFound { entity_type: "Ticket".into(), id })?;
    let messages = sqlx::query_as::<_, TicketMessageRow>(
        "SELECT * FROM crm_ticket_messages WHERE ticket_id=$1 ORDER BY created_at",
    ).bind(id).fetch_all(engine.pool()).await?;
    Ok(serde_json::json!({ "ticket": ticket, "messages": messages }))
}

pub async fn create_ticket(engine: &ErpEngine, entity_id: Uuid, req: &CreateTicketRequest, created_by_customer: Option<Uuid>) -> ErpResult<Uuid> {
    let id = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO crm_tickets (id, entity_id, customer_id, subject, description, priority, created_by_customer_user_id) \
         VALUES ($1,$2,$3,$4,$5,$6,$7)",
    )
    .bind(id).bind(entity_id).bind(req.customer_id).bind(&req.subject).bind(&req.description).bind(&req.priority).bind(created_by_customer)
    .execute(engine.pool()).await?;
    Ok(id)
}

pub async fn reply_ticket(engine: &ErpEngine, entity_id: Uuid, ticket_id: Uuid, author_kind: &str, author_id: Option<Uuid>, body: &str) -> ErpResult<()> {
    let mut tx = engine.pool().begin().await?;
    sqlx::query(
        "INSERT INTO crm_ticket_messages (id, entity_id, ticket_id, author_kind, author_id, body) VALUES ($1,$2,$3,$4,$5,$6)",
    )
    .bind(Uuid::new_v4()).bind(entity_id).bind(ticket_id).bind(author_kind).bind(author_id).bind(body)
    .execute(&mut *tx).await?;
    sqlx::query("UPDATE crm_tickets SET updated_at=NOW() WHERE id=$1 AND entity_id=$2")
        .bind(ticket_id).bind(entity_id).execute(&mut *tx).await?;
    tx.commit().await?;
    Ok(())
}

pub async fn set_ticket_status(engine: &ErpEngine, entity_id: Uuid, id: Uuid, status: &str) -> ErpResult<()> {
    sqlx::query("UPDATE crm_tickets SET status=$3, updated_at=NOW() WHERE id=$1 AND entity_id=$2")
        .bind(id).bind(entity_id).bind(status).execute(engine.pool()).await?;
    Ok(())
}

// ═══ Analytics ═══════════════════════════════════════════════════════════════

/// Pipeline value by stage, weighted forecast, win rate, and lead conversion.
pub async fn analytics(engine: &ErpEngine, entity_id: Uuid) -> ErpResult<serde_json::Value> {
    use rust_decimal::Decimal;
    use sqlx::Row;

    // Open pipeline grouped by stage.
    let stage_rows = sqlx::query(
        "SELECT s.name, s.sort_order, COUNT(o.id) AS cnt, COALESCE(SUM(o.amount),0) AS total \
         FROM crm_stages s \
         LEFT JOIN crm_opportunities o ON o.stage_id = s.id AND o.status='Open' \
         WHERE s.entity_id=$1 AND NOT s.is_won AND NOT s.is_lost \
         GROUP BY s.name, s.sort_order ORDER BY s.sort_order",
    ).bind(entity_id).fetch_all(engine.pool()).await?;
    let by_stage: Vec<serde_json::Value> = stage_rows.iter().map(|r| serde_json::json!({
        "stage": r.get::<String,_>("name"),
        "count": r.get::<i64,_>("cnt"),
        "value": r.get::<Decimal,_>("total"),
    })).collect();

    // Forecast (weighted) + open totals.
    let (open_count, open_value, forecast): (i64, Decimal, Decimal) = sqlx::query_as(
        "SELECT COUNT(*), COALESCE(SUM(amount),0), COALESCE(SUM(amount*probability/100),0) \
         FROM crm_opportunities WHERE entity_id=$1 AND status='Open'",
    ).bind(entity_id).fetch_one(engine.pool()).await?;

    // Win rate + average deal.
    let (won, lost, won_value): (i64, i64, Decimal) = sqlx::query_as(
        "SELECT \
           COUNT(*) FILTER (WHERE status='Won'), \
           COUNT(*) FILTER (WHERE status='Lost'), \
           COALESCE(SUM(amount) FILTER (WHERE status='Won'),0) \
         FROM crm_opportunities WHERE entity_id=$1",
    ).bind(entity_id).fetch_one(engine.pool()).await?;
    let closed = won + lost;
    let win_rate = if closed > 0 { (won as f64) / (closed as f64) * 100.0 } else { 0.0 };
    let avg_won = if won > 0 { won_value / Decimal::from(won) } else { Decimal::ZERO };

    // Leads: total + converted.
    let (leads_total, leads_converted): (i64, i64) = sqlx::query_as(
        "SELECT COUNT(*), COUNT(*) FILTER (WHERE status='Converted') FROM crm_leads WHERE entity_id=$1",
    ).bind(entity_id).fetch_one(engine.pool()).await?;
    let conversion = if leads_total > 0 { (leads_converted as f64) / (leads_total as f64) * 100.0 } else { 0.0 };

    // Open activities.
    let open_activities: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM crm_activities WHERE entity_id=$1 AND NOT done",
    ).bind(entity_id).fetch_one(engine.pool()).await?;

    Ok(serde_json::json!({
        "pipeline_by_stage": by_stage,
        "open_count": open_count,
        "open_value": open_value,
        "forecast": forecast.round_dp(2),
        "won": won, "lost": lost, "win_rate": (win_rate * 100.0).round() / 100.0,
        "won_value": won_value, "avg_won_deal": avg_won.round_dp(2),
        "leads_total": leads_total, "leads_converted": leads_converted,
        "lead_conversion_rate": (conversion * 100.0).round() / 100.0,
        "open_activities": open_activities,
    }))
}
