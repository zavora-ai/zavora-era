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
/// subscription transaction, capturing the reusable authorization so renewals
/// can re-charge without the customer. Idempotent. Extends the paid-through
/// date 30 days from now.
pub async fn activate_from_charge(
    engine: &ErpEngine,
    entity_id: Uuid,
    data: &crate::payments::paystack::PaystackChargeData,
) -> ErpResult<()> {
    let plan: Option<String> = sqlx::query_scalar(
        "SELECT plan FROM paystack_transactions WHERE entity_id = $1 AND reference = $2 AND purpose = 'subscription'",
    )
    .bind(entity_id)
    .bind(&data.reference)
    .fetch_optional(engine.pool())
    .await?
    .flatten();
    let Some(plan) = plan else {
        return Err(ErpError::NotFound { entity_type: "Subscription".to_string(), id: Uuid::nil() });
    };

    let period_end = Utc::now() + Duration::days(30);
    let mut patch = serde_json::json!({
        "plan": plan,
        "status": "active",
        "current_period_end": period_end,
        "failed_attempts": 0,
        "updated_at": Utc::now(),
    });
    // Store the reusable authorization + email for automatic renewal.
    if let Some(code) = data.reusable_auth_code() {
        patch["authorization_code"] = code.into();
    }
    if let Some(email) = data.customer_email() {
        patch["billing_email"] = email.into();
    }
    merge_subscription(engine, entity_id, patch, Some(&plan)).await?;

    sqlx::query("UPDATE paystack_transactions SET status = 'success' WHERE entity_id = $1 AND reference = $2")
        .bind(entity_id)
        .bind(&data.reference)
        .execute(engine.pool())
        .await?;
    Ok(())
}

/// Cancel a tenant's subscription. Access continues until the paid-through date
/// (`current_period_end`); renewal stops.
pub async fn cancel(engine: &ErpEngine, entity_id: Uuid) -> ErpResult<()> {
    merge_subscription(
        engine,
        entity_id,
        serde_json::json!({ "status": "cancelled", "updated_at": Utc::now() }),
        None,
    )
    .await
}

/// Charge every active subscription whose paid-through date has passed, using
/// the stored Paystack authorization. Returns (renewed, failed) counts. Called
/// by the scheduler. A subscription with no saved authorization, or that fails
/// 3 times, is marked `past_due` (kept, not deleted — an operator can follow up).
pub async fn process_renewals(engine: &ErpEngine) -> ErpResult<(u32, u32)> {
    let Some(secret) = crate::payments::paystack::secret_key() else {
        return Ok((0, 0)); // billing not configured on this instance
    };

    // Due = active, past period end, on a paid plan.
    let due = sqlx::query_as::<_, (Uuid, serde_json::Value)>(
        r#"SELECT entity_id, subscription FROM entity_settings
           WHERE subscription->>'status' = 'active'
             AND subscription->>'current_period_end' IS NOT NULL
             AND (subscription->>'current_period_end')::timestamptz < NOW()"#,
    )
    .fetch_all(engine.pool())
    .await?;

    let mut renewed = 0u32;
    let mut failed = 0u32;
    for (entity_id, sub) in due {
        let plan_key = sub.get("plan").and_then(|v| v.as_str()).unwrap_or("");
        let Some(plan) = plan_by_key(plan_key) else { continue };
        if plan.monthly_kes == 0 {
            continue;
        }
        let auth_code = sub.get("authorization_code").and_then(|v| v.as_str());
        let email = sub.get("billing_email").and_then(|v| v.as_str());
        let attempts = sub.get("failed_attempts").and_then(|v| v.as_u64()).unwrap_or(0);

        let charged = match (auth_code, email) {
            (Some(code), Some(email)) if !code.is_empty() => {
                charge_authorization(&secret, email, code, plan.monthly_kes).await
            }
            _ => false, // no saved authorization to charge
        };

        if charged {
            let period_end = Utc::now() + Duration::days(30);
            let _ = merge_subscription(
                engine,
                entity_id,
                serde_json::json!({ "current_period_end": period_end, "failed_attempts": 0, "updated_at": Utc::now() }),
                None,
            )
            .await;
            renewed += 1;
        } else {
            let next = attempts + 1;
            let status = if next >= 3 { "past_due" } else { "active" };
            let _ = merge_subscription(
                engine,
                entity_id,
                serde_json::json!({ "status": status, "failed_attempts": next, "updated_at": Utc::now() }),
                None,
            )
            .await;
            failed += 1;
        }
    }
    Ok((renewed, failed))
}

/// Charge a saved Paystack authorization for a renewal. Returns true on a
/// successful charge.
async fn charge_authorization(secret: &str, email: &str, auth_code: &str, amount_kes: i64) -> bool {
    let body = serde_json::json!({
        "email": email,
        "amount": (amount_kes * 100).to_string(),
        "currency": "KES",
        "authorization_code": auth_code,
    });
    let resp: Result<serde_json::Value, _> = async {
        reqwest::Client::new()
            .post("https://api.paystack.co/transaction/charge_authorization")
            .bearer_auth(secret)
            .json(&body)
            .send()
            .await?
            .json()
            .await
    }
    .await;
    matches!(
        resp,
        Ok(v) if v["status"].as_bool() == Some(true)
            && v["data"]["status"].as_str() == Some("success")
    )
}

/// Merge a patch into entity_settings.subscription, preserving existing keys
/// (so a status update doesn't wipe the stored authorization). When `plan` is
/// given it's also mirrored into branding.plan. Also syncs the platform
/// tenants directory (plan_key / plan_status) for the ops console.
async fn merge_subscription(
    engine: &ErpEngine,
    entity_id: Uuid,
    patch: serde_json::Value,
    plan: Option<&str>,
) -> ErpResult<()> {
    let branding_plan = plan.map(serde_json::Value::from).unwrap_or(serde_json::Value::Null);
    sqlx::query(
        r#"UPDATE entity_settings
           SET subscription = COALESCE(subscription, '{}'::jsonb) || $1,
               branding = CASE WHEN $2::jsonb = 'null'::jsonb THEN branding
                               ELSE COALESCE(branding, '{}'::jsonb) || jsonb_build_object('plan', $2::jsonb) END
           WHERE entity_id = $3"#,
    )
    .bind(&patch)
    .bind(&branding_plan)
    .bind(entity_id)
    .execute(engine.pool())
    .await?;
    engine.invalidate_config(entity_id).await;

    // Platform directory: keep ops console plan badges in sync with billing.
    let status = patch.get("status").and_then(|v| v.as_str());
    let plan_key = plan.or_else(|| patch.get("plan").and_then(|v| v.as_str()));
    if plan_key.is_some() || status.is_some() {
        if let Err(e) = crate::services::platform::sync_tenant_billing(
            engine.pool(),
            entity_id,
            plan_key,
            status,
        )
        .await
        {
            tracing::warn!(%entity_id, error = %e, "platform tenant billing sync failed");
        }
    }
    Ok(())
}

/// Set (or reset) a subscription to a specific plan/status without a payment —
/// used for free-plan trials and the initial pending state.
async fn set_subscription(
    engine: &ErpEngine,
    entity_id: Uuid,
    plan: &str,
    status: &str,
    period_end: Option<chrono::DateTime<Utc>>,
) -> ErpResult<()> {
    merge_subscription(
        engine,
        entity_id,
        serde_json::json!({
            "plan": plan,
            "status": status,
            "current_period_end": period_end,
            "updated_at": Utc::now(),
        }),
        Some(plan),
    )
    .await
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::platform::map_subscription_status;

    #[test]
    fn plan_prices_are_defined() {
        assert_eq!(plan_by_key("free").unwrap().monthly_kes, 0);
        assert_eq!(plan_by_key("business").unwrap().monthly_kes, 6_900);
        assert!(plan_by_key("nonsense").is_none());
    }

    #[test]
    fn subscription_status_maps_for_platform_directory() {
        assert_eq!(map_subscription_status("trialing"), "trial");
        assert_eq!(map_subscription_status("active"), "active");
        assert_eq!(map_subscription_status("past_due"), "past_due");
        assert_eq!(map_subscription_status("cancelled"), "active");
    }
}
