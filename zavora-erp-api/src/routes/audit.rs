use axum::{extract::{Path, Query, State}, Json};
use std::sync::Arc;
use uuid::Uuid;

use crate::AppState;
use crate::middleware::auth::AuthContext;
use super::err_response;
use zavora_erp_core::audit::*;
use zavora_erp_core::services::audit as svc;

#[derive(serde::Deserialize)]
pub struct AuditQueryParams {
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}

pub async fn query(
    ctx: AuthContext,
    State(state): State<Arc<AppState>>,
    Query(params): Query<AuditQueryParams>,
) -> Result<Json<serde_json::Value>, impl axum::response::IntoResponse> {
    let q = AuditQuery {
        entity_id: ctx.entity_id,
        object_type: None,
        object_id: None,
        actor: None,
        event_type: None,
        from: None,
        to: None,
        limit: params.limit.or(Some(50)),
        offset: params.offset,
    };
    match svc::query_events(&state.engine, q).await {
        Ok(page) => {
            let mut v = serde_json::to_value(page).unwrap_or_default();
            enrich_actors(&state, ctx.entity_id, &mut v).await;
            Ok(Json(v))
        }
        Err(e) => Err(err_response(e)),
    }
}

/// Resolve each event's actor (a `{type,id}` reference) to a human name + email so
/// the audit trail can show *who* did it, not a bare UUID. User actors are looked
/// up in era_users; Agent actors use their id as the name.
async fn enrich_actors(state: &Arc<AppState>, entity_id: Uuid, value: &mut serde_json::Value) {
    let Some(events) = value.get_mut("events").and_then(|e| e.as_array_mut()) else { return };
    let ids: Vec<Uuid> = events
        .iter()
        .filter_map(|e| {
            let a = e.get("actor")?;
            if a.get("type")?.as_str()? == "User" {
                a.get("id")?.as_str()?.parse::<Uuid>().ok()
            } else {
                None
            }
        })
        .collect();

    let mut names: std::collections::HashMap<Uuid, (String, String)> = Default::default();
    if !ids.is_empty() {
        if let Ok(rows) = sqlx::query_as::<_, (Uuid, String, String)>(
            "SELECT id, display_name, email FROM era_users WHERE entity_id = $1 AND id = ANY($2)",
        )
        .bind(entity_id)
        .bind(&ids)
        .fetch_all(state.engine.pool())
        .await
        {
            for (id, name, email) in rows {
                names.insert(id, (name, email));
            }
        }
    }

    for e in events.iter_mut() {
        let (atype, aid) = {
            let a = e.get("actor");
            (
                a.and_then(|a| a.get("type")).and_then(|t| t.as_str()).unwrap_or("").to_string(),
                a.and_then(|a| a.get("id")).and_then(|i| i.as_str()).unwrap_or("").to_string(),
            )
        };
        let (name, email) = if atype == "User" {
            aid.parse::<Uuid>()
                .ok()
                .and_then(|id| names.get(&id).cloned())
                .unwrap_or_else(|| ("Unknown user".to_string(), String::new()))
        } else if atype == "Agent" {
            (format!("{aid} (system)"), String::new())
        } else {
            ("System".to_string(), String::new())
        };
        if let Some(obj) = e.as_object_mut() {
            obj.insert("actor_name".into(), serde_json::json!(name));
            obj.insert("actor_email".into(), serde_json::json!(email));
        }
    }
}

pub async fn for_object(
    ctx: AuthContext,
    State(state): State<Arc<AppState>>,
    Path((object_type, object_id)): Path<(String, Uuid)>,
) -> Result<Json<serde_json::Value>, impl axum::response::IntoResponse> {
    let q = AuditQuery {
        entity_id: ctx.entity_id,
        object_type: Some(object_type),
        object_id: Some(object_id),
        actor: None,
        event_type: None,
        from: None,
        to: None,
        limit: Some(100),
        offset: Some(0),
    };
    match svc::query_events(&state.engine, q).await {
        Ok(page) => {
            let mut v = serde_json::to_value(page).unwrap_or_default();
            enrich_actors(&state, ctx.entity_id, &mut v).await;
            Ok(Json(v))
        }
        Err(e) => Err(err_response(e)),
    }
}
