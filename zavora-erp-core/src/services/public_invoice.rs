//! Public (unauthenticated) invoice pay-link flow.
//!
//! A customer opens an invoice by its random `public_token` — no login — sees a
//! sanitized summary (which stamps `viewed_at` on first open), and can pay it by
//! card via Paystack, reusing the authenticated card-payment machinery
//! (`paystack_initialize`). Only sent/posted invoices with a token are
//! reachable; drafts and voided/cancelled invoices are treated as not found so
//! nothing internal leaks.

use rust_decimal::Decimal;
use serde::Serialize;
use uuid::Uuid;

use crate::engine::ErpEngine;
use crate::error::{ErpError, ErpResult};

/// A sanitized invoice view safe to expose without authentication.
#[derive(Debug, Clone, Serialize)]
pub struct PublicInvoiceView {
    pub number: String,
    pub company_name: String,
    pub currency: String,
    pub gross_total: Decimal,
    pub amount_paid: Decimal,
    pub balance_due: Decimal,
    pub status: String,
    pub issue_date: chrono::NaiveDate,
    pub due_date: chrono::NaiveDate,
    /// True when there is an outstanding balance on a live (non-draft/voided) invoice.
    pub payable: bool,
}

#[derive(sqlx::FromRow)]
struct PubRow {
    id: Uuid,
    entity_id: Uuid,
    number: String,
    currency: String,
    gross_total: Decimal,
    amount_paid: Decimal,
    balance_due: Decimal,
    status: String,
    issue_date: chrono::NaiveDate,
    due_date: chrono::NaiveDate,
    viewed_at: Option<chrono::DateTime<chrono::Utc>>,
}

fn is_public_visible(status: &str) -> bool {
    !matches!(status, "draft" | "voided" | "cancelled")
}

/// Resolve a public invoice by token, stamping `viewed_at` on first open.
pub async fn get_public_invoice(engine: &ErpEngine, token: &str) -> ErpResult<PublicInvoiceView> {
    let row = sqlx::query_as::<_, PubRow>(
        r#"SELECT id, entity_id, number, currency, gross_total, amount_paid, balance_due,
                  status, issue_date, due_date, viewed_at
           FROM invoices WHERE public_token = $1"#,
    )
    .bind(token)
    .fetch_optional(engine.pool())
    .await?
    .filter(|r| is_public_visible(&r.status))
    .ok_or_else(|| ErpError::NotFound {
        entity_type: "Invoice".to_string(),
        id: Uuid::nil(),
    })?;

    // Stamp first-view (idempotent — only when not already set).
    if row.viewed_at.is_none() {
        sqlx::query("UPDATE invoices SET viewed_at = now() WHERE id = $1 AND viewed_at IS NULL")
            .bind(row.id)
            .execute(engine.pool())
            .await?;
    }

    let company_name = sqlx::query_scalar::<_, Option<String>>(
        "SELECT branding->>'company_name' FROM entity_settings WHERE entity_id = $1",
    )
    .bind(row.entity_id)
    .fetch_optional(engine.pool())
    .await?
    .flatten()
    .filter(|s| !s.trim().is_empty())
    .unwrap_or_else(|| "The business".to_string());

    let payable = row.balance_due > Decimal::ZERO && row.status != "paid";

    Ok(PublicInvoiceView {
        number: row.number,
        company_name,
        currency: row.currency,
        gross_total: row.gross_total,
        amount_paid: row.amount_paid,
        balance_due: row.balance_due,
        status: row.status,
        issue_date: row.issue_date,
        due_date: row.due_date,
        payable,
    })
}

/// Start a Paystack card payment for a public invoice (by token), reusing the
/// authenticated initialization path once the token is resolved to its tenant.
pub async fn pay_public_invoice(
    engine: &ErpEngine,
    token: &str,
    email: Option<String>,
    callback_url: Option<String>,
) -> ErpResult<crate::payments::paystack::PaystackInitResponse> {
    let rec = sqlx::query_as::<_, (Uuid, Uuid, Decimal, String)>(
        "SELECT id, entity_id, balance_due, status FROM invoices WHERE public_token = $1",
    )
    .bind(token)
    .fetch_optional(engine.pool())
    .await?
    .filter(|(_, _, _, status)| is_public_visible(status));

    let (invoice_id, entity_id, balance_due, _status) = rec.ok_or_else(|| ErpError::NotFound {
        entity_type: "Invoice".to_string(),
        id: Uuid::nil(),
    })?;

    if balance_due <= Decimal::ZERO {
        return Err(ErpError::ValidationFailed {
            message: "This invoice has already been paid.".to_string(),
        });
    }

    crate::services::payments::paystack_initialize(
        engine,
        entity_id,
        crate::payments::paystack::PaystackInitRequest {
            invoice_id,
            email,
            callback_url,
        },
    )
    .await
}
