//! Safaricom Daraja (M-Pesa) STK Push client.
//!
//! Implements the two Daraja calls needed to trigger an STK (Lipa na M-Pesa
//! Online) prompt on a customer's phone:
//!   1. **OAuth** — `GET /oauth/v1/generate?grant_type=client_credentials` with
//!      HTTP Basic (consumer key/secret) → short-lived access token.
//!   2. **STK Push** — `POST /mpesa/stkpush/v1/processrequest` with the
//!      timestamped, base64 password and the order details.
//!
//! Credentials are **deployment-level** (like SMTP), read from the environment,
//! because they are issued per Safaricom shortcode, not per tenant. When they
//! are absent the client is `None` and the caller returns a clear "not
//! configured" error — the existing behaviour, now real when configured.
//!
//! NOTE: this path makes outbound calls to Safaricom and cannot be exercised
//! without live Daraja sandbox/production credentials; it is covered by unit
//! tests for the pure helpers (password/timestamp) only.

use base64::{engine::general_purpose::STANDARD as B64, Engine as _};
use rust_decimal::Decimal;
use serde::Deserialize;
use std::time::Duration;

use crate::error::{ErpError, ErpResult};

const DEFAULT_TIMEOUT_SECS: u64 = 20;

/// Daraja configuration loaded from the environment.
#[derive(Debug, Clone)]
pub struct DarajaConfig {
    pub consumer_key: String,
    pub consumer_secret: String,
    /// Lipa na M-Pesa shortcode (Paybill/Till).
    pub shortcode: String,
    /// Lipa na M-Pesa Online passkey.
    pub passkey: String,
    /// Publicly reachable callback URL Daraja will POST the result to.
    pub callback_url: String,
    /// API base — sandbox or production.
    pub base_url: String,
}

impl DarajaConfig {
    /// Build from env. Returns `None` unless all required vars are present.
    /// `MPESA_ENV=production` selects the production host (default: sandbox).
    pub fn from_env() -> Option<Self> {
        let get = |k: &str| std::env::var(k).ok().map(|v| v.trim().to_string()).filter(|v| !v.is_empty());
        let base_url = match std::env::var("MPESA_ENV").ok().as_deref() {
            Some("production") | Some("prod") => "https://api.safaricom.co.ke".to_string(),
            _ => "https://sandbox.safaricom.co.ke".to_string(),
        };
        Some(Self {
            consumer_key: get("MPESA_CONSUMER_KEY")?,
            consumer_secret: get("MPESA_CONSUMER_SECRET")?,
            shortcode: get("MPESA_SHORTCODE")?,
            passkey: get("MPESA_PASSKEY")?,
            callback_url: get("MPESA_CALLBACK_URL")?,
            base_url,
        })
    }
}

/// Result of a successful STK Push initiation.
#[derive(Debug, Clone, Deserialize)]
pub struct StkPushResult {
    #[serde(rename = "CheckoutRequestID")]
    pub checkout_request_id: String,
    #[serde(rename = "MerchantRequestID")]
    pub merchant_request_id: String,
    #[serde(rename = "ResponseCode")]
    pub response_code: String,
    #[serde(rename = "CustomerMessage", default)]
    pub customer_message: String,
}

#[derive(Debug, Deserialize)]
struct OAuthResponse {
    access_token: String,
}

/// The current timestamp in Daraja's `YYYYMMDDHHMMSS` format.
pub fn daraja_timestamp(now: chrono::DateTime<chrono::Utc>) -> String {
    now.format("%Y%m%d%H%M%S").to_string()
}

/// The STK password: base64(shortcode + passkey + timestamp).
pub fn stk_password(shortcode: &str, passkey: &str, timestamp: &str) -> String {
    B64.encode(format!("{shortcode}{passkey}{timestamp}"))
}

/// Normalise a Kenyan MSISDN to the `2547XXXXXXXX` form Daraja expects.
pub fn msisdn(raw: &str) -> String {
    let digits: String = raw.chars().filter(|c| c.is_ascii_digit()).collect();
    if let Some(rest) = digits.strip_prefix("254") {
        format!("254{rest}")
    } else if let Some(rest) = digits.strip_prefix('0') {
        format!("254{rest}")
    } else if digits.len() == 9 {
        format!("254{digits}")
    } else {
        digits
    }
}

/// A configured Daraja client.
pub struct DarajaClient {
    http: reqwest::Client,
    cfg: DarajaConfig,
}

impl DarajaClient {
    /// Construct from env, or `None` when not configured.
    pub fn from_env() -> Option<Self> {
        let cfg = DarajaConfig::from_env()?;
        let http = reqwest::Client::builder()
            .timeout(Duration::from_secs(DEFAULT_TIMEOUT_SECS))
            .build()
            .ok()?;
        Some(Self { http, cfg })
    }

    async fn access_token(&self) -> ErpResult<String> {
        let url = format!("{}/oauth/v1/generate?grant_type=client_credentials", self.cfg.base_url);
        let resp = self
            .http
            .get(&url)
            .basic_auth(&self.cfg.consumer_key, Some(&self.cfg.consumer_secret))
            .send()
            .await
            .map_err(|e| ErpError::PaymentError { message: format!("Daraja OAuth request failed: {e}") })?;
        if !resp.status().is_success() {
            return Err(ErpError::PaymentError {
                message: format!("Daraja OAuth returned {}", resp.status()),
            });
        }
        let body: OAuthResponse = resp
            .json()
            .await
            .map_err(|e| ErpError::PaymentError { message: format!("Daraja OAuth parse error: {e}") })?;
        Ok(body.access_token)
    }

    /// Initiate an STK Push for `amount` to `phone`, tagged with `account_ref`
    /// (e.g. the invoice number) and `description`.
    pub async fn stk_push(
        &self,
        phone: &str,
        amount: Decimal,
        account_ref: &str,
        description: &str,
    ) -> ErpResult<StkPushResult> {
        let token = self.access_token().await?;
        let timestamp = daraja_timestamp(chrono::Utc::now());
        let password = stk_password(&self.cfg.shortcode, &self.cfg.passkey, &timestamp);
        // Daraja expects a whole-number amount.
        let amount_int = amount.round().to_string();

        let payload = serde_json::json!({
            "BusinessShortCode": self.cfg.shortcode,
            "Password": password,
            "Timestamp": timestamp,
            "TransactionType": "CustomerPayBillOnline",
            "Amount": amount_int,
            "PartyA": msisdn(phone),
            "PartyB": self.cfg.shortcode,
            "PhoneNumber": msisdn(phone),
            "CallBackURL": self.cfg.callback_url,
            "AccountReference": account_ref,
            "TransactionDesc": description,
        });

        let url = format!("{}/mpesa/stkpush/v1/processrequest", self.cfg.base_url);
        let resp = self
            .http
            .post(&url)
            .bearer_auth(token)
            .json(&payload)
            .send()
            .await
            .map_err(|e| ErpError::PaymentError { message: format!("Daraja STK push failed: {e}") })?;

        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();
        if !status.is_success() {
            return Err(ErpError::PaymentError {
                message: format!("Daraja STK push HTTP {status}: {text}"),
            });
        }
        serde_json::from_str::<StkPushResult>(&text).map_err(|e| ErpError::PaymentError {
            message: format!("Daraja STK push parse error: {e} (body: {text})"),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn password_is_base64_of_concatenation() {
        let ts = "20260628120000";
        let pw = stk_password("174379", "passkey123", ts);
        let decoded = String::from_utf8(B64.decode(pw).unwrap()).unwrap();
        assert_eq!(decoded, "174379passkey123{ts}".replace("{ts}", ts));
    }

    #[test]
    fn timestamp_format_is_14_digits() {
        let ts = daraja_timestamp(chrono::Utc::now());
        assert_eq!(ts.len(), 14);
        assert!(ts.chars().all(|c| c.is_ascii_digit()));
    }

    #[test]
    fn msisdn_normalises_kenyan_formats() {
        assert_eq!(msisdn("0712345678"), "254712345678");
        assert_eq!(msisdn("712345678"), "254712345678");
        assert_eq!(msisdn("254712345678"), "254712345678");
        assert_eq!(msisdn("+254 712 345 678"), "254712345678");
    }
}
