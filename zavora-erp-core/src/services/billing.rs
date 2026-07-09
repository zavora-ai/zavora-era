//! Subscription billing: Zavora plan checkout via Paystack.
//!
//! A new tenant picks a plan at signup. Paid plans are taken to a Paystack
//! checkout (card + M-Pesa mobile-money + bank in one flow); on a verified
//! `charge.success` the subscription is activated for a month. The plan PRICE
//! is authoritative HERE (not the browser) so the amount charged can't be
//! tampered with client-side.

use chrono::{Duration, Utc};
use rust_decimal::Decimal;
use serde::Serialize;
use uuid::Uuid;

use crate::engine::ErpEngine;
use crate::error::{ErpError, ErpResult};

/// A Zavora subscription plan and its monthly price in KES. The keys mirror the
/// UI's `config/pricing.ts`; the prices are the source of truth for charging.
pub struct Plan {
    pub key: &'static str,
    pub name: &'static str,
    /// Monthly price in KES (whole shillings). Free = 0.
    pub monthly_kes: i64,
}

pub const PLANS: &[Plan] = &[
    Plan { key: "free", name: "Free", monthly_kes: 0 },
    Plan { key: "starter", name: "Starter", monthly_kes: 2_500 },
    Plan { key: "business", name: "Business", monthly_kes: 6_900 },
    Plan { key: "business_pro", name: "Business Pro", monthly_kes: 14_900 },
];

pub fn plan_by_key(key: &str) -> Option<&'static Plan> {
    PLANS.iter().find(|p| p.key == key)
}

#[derive(Debug, Serialize)]
pub struct CheckoutResponse {
    /// Present for paid plans — redirect the browser here to pay.
    pub authorization_url: Option<String>,
    pub reference: Option<String>,
    /// True when the plan is free and no payment is needed (activated immediately).
    pub free: bool,
    pub plan: String,
}

/// Start a subscription checkout for a tenant's chosen plan. Free plans activate
/// immediately (trialing) and return `free: true`. Paid plans initialise a
/// Paystack transaction offering card, M-Pesa and bank, and return the
/// authorization URL to redirect the owner to.
pub async fn start_checkout(
    engine: &ErpEngine,
    entity_id: Uuid,
    owner_email: &str,
    plan_key: &str,
    callback_url: Option<String>,
) -> ErpResult<CheckoutResponse> {
    let plan = plan_by_key(plan_key).ok_or_else(|| ErpError::ValidationFailed {
        message: format!("Unknown plan '{plan_key}'"),
    })?;

    // Free plan: nothing to charge — set a trialing subscription and return.
    if plan.monthly_kes == 0 {
        set_subscription(engine, entity_id, plan.key, "trialing", None).await?;
        return Ok(CheckoutResponse { authorization_url: None, reference: None, free: true, plan: plan.key.to_string() });
    }

    let secret = crate::payments::paystack::secret_key().ok_or_else(|| ErpError::PaymentError {
        message: "Card billing is not configured (PAYSTACK_SECRET_KEY unset).".to_string(),
    })?;
    if owner_email.trim().is_empty() {
        return Err(ErpError::ValidationFailed { message: "An email is required to check out.".to_string() });
    }

    let reference = format!("SUB-{}-{}", plan.key, &Uuid::new_v4().to_string()[..8]);
    // Paystack takes the amount in the currency subunit (KES cents).
    let subunit = plan.monthly_kes * 100;

    let mut body = serde_json::json!({
        "email": owner_email,
        "amount": subunit.to_string(),
        "currency": "KES",
        "reference": reference,
        // Offer card + M-Pesa (mobile money) + bank in the one checkout.
        "channels": ["card", "mobile_money", "bank", "ussd"],
        "metadata": { "purpose": "subscription", "plan": plan.key, "entity_id": entity_id },
    });
    if let Some(cb) = callback_url.filter(|c| !c.trim().is_empty()) {
        body["callback_url"] = cb.into();
    }

    let resp: serde_json::Value = reqwest::Client::new()
        .post("https://api.paystack.co/transaction/initialize")
        .bearer_auth(&secret)
        .json(&body)
        .send()
        .await
        .map_err(|e| ErpError::PaymentError { message: format!("Paystack request failed: {e}") })?
        .json()
        .await
        .map_err(|e| ErpError::PaymentError { message: format!("Paystack response invalid: {e}") })?;

    if resp["status"].as_bool() != Some(true) {
        return Err(ErpError::PaymentError {
            message: format!("Paystack rejected the request: {}", resp["message"].as_str().unwrap_or("unknown")),
        });
    }
    let auth_url = resp["data"]["authorization_url"].as_str().ok_or_else(|| ErpError::PaymentError {
        message: "Paystack did not return an authorization_url".to_string(),
    })?;

    sqlx::query(
        r#"INSERT INTO paystack_transactions
           (entity_id, reference, amount, currency, customer_email, status, authorization_url, purpose, plan)
           VALUES ($1, $2, $3, 'KES', $4, 'initialized', $5, 'subscription', $6)"#,
    )
    .bind(entity_id)
    .bind(&reference)
    .bind(Decimal::from(plan.monthly_kes))
    .bind(owner_email)
    .bind(auth_url)
    .bind(plan.key)
    .execute(engine.pool())
    .await?;

    // Mark the subscription pending payment (still trialing until it settles).
    set_subscription(engine, entity_id, plan.key, "trialing", None).await?;

    Ok(CheckoutResponse {
        authorization_url: Some(auth_url.to_string()),
        reference: Some(reference),
        free: false,
        plan: plan.key.to_string(),
    })
}

/// Activate a subscription after a verified Paystack `charge.success` for a
/// subscription transaction. Idempotent: activating an already-active period is
/// harmless (it re-stamps the same month). Extends the paid-through date one
/// month from now.
pub async fn activate_from_reference(engine: &ErpEngine, entity_id: Uuid, reference: &str) -> ErpResult<()> {
    let plan: Option<String> = sqlx::query_scalar(
        "SELECT plan FROM paystack_transactions WHERE entity_id = $1 AND reference = $2 AND purpose = 'subscription'",
    )
    .bind(entity_id)
    .bind(reference)
    .fetch_optional(engine.pool())
    .await?
    .flatten();
    let Some(plan) = plan else {
        return Err(ErpError::NotFound { entity_type: "Subscription".to_string(), id: Uuid::nil() });
    };

    let period_end = Utc::now() + Duration::days(30);
    set_subscription(engine, entity_id, &plan, "active", Some(period_end)).await?;

    sqlx::query("UPDATE paystack_transactions SET status = 'success' WHERE entity_id = $1 AND reference = $2")
        .bind(entity_id)
        .bind(reference)
        .execute(engine.pool())
        .await?;
    Ok(())
}

/// Write the tenant's subscription state onto entity_settings.subscription, and
/// mirror the plan into branding.plan (where signup already records it) so both
/// stay consistent.
async fn set_subscription(
    engine: &ErpEngine,
    entity_id: Uuid,
    plan: &str,
    status: &str,
    period_end: Option<chrono::DateTime<Utc>>,
) -> ErpResult<()> {
    let sub = serde_json::json!({
        "plan": plan,
        "status": status,
        "current_period_end": period_end,
        "updated_at": Utc::now(),
    });
    sqlx::query(
        r#"UPDATE entity_settings
           SET subscription = $1,
               branding = COALESCE(branding, '{}'::jsonb) || jsonb_build_object('plan', $2::text)
           WHERE entity_id = $3"#,
    )
    .bind(sub)
    .bind(plan)
    .bind(entity_id)
    .execute(engine.pool())
    .await?;
    engine.invalidate_config(entity_id).await;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plan_prices_are_defined() {
        assert_eq!(plan_by_key("free").unwrap().monthly_kes, 0);
        assert_eq!(plan_by_key("business").unwrap().monthly_kes, 6_900);
        assert!(plan_by_key("nonsense").is_none());
    }
}
