use axum::{extract::State, Json};
use std::sync::Arc;

use crate::AppState;
use crate::middleware::auth::AuthContext;
use super::err_response;

pub async fn summary(
    ctx: AuthContext,
    State(state): State<Arc<AppState>>,
) -> Result<Json<serde_json::Value>, impl axum::response::IntoResponse> {
    let summary = match state.engine.dashboard_summary(ctx.entity_id).await {
        Ok(s) => s,
        Err(e) => return Err(err_response(e)),
    };
    let mut out = serde_json::to_value(summary).unwrap_or_default();

    // Entity activity counts so the UI can detect a brand-new tenant (R9 empty state).
    let counts = sqlx::query_as::<_, (i64, i64, i64)>(
        r#"SELECT
            (SELECT COUNT(*) FROM invoices WHERE entity_id = $1),
            (SELECT COUNT(*) FROM bills WHERE entity_id = $1),
            (SELECT COUNT(*) FROM payments WHERE entity_id = $1)"#,
    )
    .bind(ctx.entity_id)
    .fetch_one(state.engine.pool())
    .await
    .unwrap_or((0, 0, 0));

    if let Some(obj) = out.as_object_mut() {
        obj.insert("invoice_count".into(), serde_json::json!(counts.0));
        obj.insert("bill_count".into(), serde_json::json!(counts.1));
        obj.insert("payment_count".into(), serde_json::json!(counts.2));
    }
    Ok(Json(out))
}
