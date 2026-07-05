//! Approval spend-limits / Delegation of Authority (DoA).
//!
//! Each role may carry a ceiling on the value it is allowed to approve. A user
//! approving a bill, requisition or expense claim above their role's limit is
//! blocked — the document must go to someone with higher authority. A missing
//! or NULL limit means "unlimited" (so existing tenants are unaffected until
//! they configure limits).

use crate::engine::ErpEngine;
use crate::error::{ErpError, ErpResult};
use rust_decimal::Decimal;
use uuid::Uuid;

#[derive(Debug, Clone, serde::Serialize, sqlx::FromRow)]
pub struct ApprovalLimitRow {
    pub role: String,
    pub max_amount: Option<Decimal>,
}

/// All configured limits for the entity.
pub async fn list_limits(engine: &ErpEngine, entity_id: Uuid) -> ErpResult<Vec<ApprovalLimitRow>> {
    let rows = sqlx::query_as::<_, ApprovalLimitRow>(
        "SELECT role, max_amount FROM approval_limits WHERE entity_id=$1 ORDER BY role",
    )
    .bind(entity_id)
    .fetch_all(engine.pool())
    .await?;
    Ok(rows)
}

/// Set (or clear, with `None`) the limit for a role.
pub async fn set_limit(engine: &ErpEngine, entity_id: Uuid, role: &str, max_amount: Option<Decimal>) -> ErpResult<()> {
    sqlx::query(
        r#"INSERT INTO approval_limits (entity_id, role, max_amount) VALUES ($1,$2,$3)
           ON CONFLICT (entity_id, role) DO UPDATE SET max_amount = EXCLUDED.max_amount"#,
    )
    .bind(entity_id)
    .bind(role)
    .bind(max_amount)
    .execute(engine.pool())
    .await?;
    Ok(())
}

/// Enforce the approver's spend limit for a document of value `amount`. Looks up
/// the approver's role, then their configured ceiling. No limit configured →
/// allowed. Over the limit → `PermissionDenied`.
pub async fn assert_within_limit(
    engine: &ErpEngine,
    entity_id: Uuid,
    approver: Uuid,
    amount: Decimal,
    doc_label: &str,
) -> ErpResult<()> {
    let role: Option<String> = sqlx::query_scalar("SELECT role FROM era_users WHERE id=$1 AND entity_id=$2")
        .bind(approver)
        .bind(entity_id)
        .fetch_optional(engine.pool())
        .await?;
    let Some(role) = role else { return Ok(()); };

    let limit: Option<Option<Decimal>> = sqlx::query_scalar(
        "SELECT max_amount FROM approval_limits WHERE entity_id=$1 AND role=$2",
    )
    .bind(entity_id)
    .bind(&role)
    .fetch_optional(engine.pool())
    .await?;

    if let Some(Some(max)) = limit {
        if amount > max {
            return Err(ErpError::ValidationFailed {
                message: format!(
                    "This {doc_label} ({amount}) exceeds your approval limit ({max}) for role {role}. It needs approval from someone with higher authority."
                ),
            });
        }
    }
    Ok(())
}
