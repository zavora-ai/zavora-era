//! Paystack card payments: transaction initialisation and webhook types.
//!
//! Flow: the ERP asks Paystack to `initialize` a transaction for an invoice
//! (server-to-server, authenticated with the SECRET key), gets back an
//! `authorization_url`, and sends the payer there. When the charge settles,
//! Paystack POSTs a `charge.success` webhook signed with
//! `x-paystack-signature = HMAC-SHA512(raw_body, secret_key)` — unlike M-Pesa,
//! this is cryptographically verifiable, so a forged callback is rejected.
//!
//! The SECRET key lives only in the `PAYSTACK_SECRET_KEY` environment variable
//! (never the database). The public key is safe in settings for the browser.

use hmac::{Hmac, Mac};
use serde::{Deserialize, Serialize};
use sha2::Sha512;

/// Reads the Paystack secret key from the environment. `None` (and callers
/// fail loudly) when unset, so a misconfigured deployment can't silently accept
/// unverified webhooks.
pub fn secret_key() -> Option<String> {
    std::env::var("PAYSTACK_SECRET_KEY").ok().filter(|k| !k.trim().is_empty())
}

/// Verify a webhook body against the `x-paystack-signature` header using the
/// secret key. Constant-time comparison via the MAC verifier.
pub fn verify_signature(raw_body: &[u8], signature_hex: &str, secret: &str) -> bool {
    let Ok(mut mac) = Hmac::<Sha512>::new_from_slice(secret.as_bytes()) else {
        return false;
    };
    mac.update(raw_body);
    let Ok(expected) = hex::decode(signature_hex.trim()) else {
        return false;
    };
    mac.verify_slice(&expected).is_ok()
}

/// Paystack webhook envelope (we only model the fields we act on).
#[derive(Debug, Clone, Deserialize)]
pub struct PaystackEvent {
    pub event: String,
    pub data: PaystackChargeData,
}

#[derive(Debug, Clone, Deserialize)]
pub struct PaystackChargeData {
    pub reference: String,
    /// Amount in the currency's SUBUNIT (kobo/cents) — divide by 100.
    #[serde(default)]
    pub amount: i64,
    #[serde(default)]
    pub currency: Option<String>,
    #[serde(default)]
    pub status: Option<String>,
}

impl PaystackEvent {
    /// True for a settled successful charge — the only event that records money.
    pub fn is_successful_charge(&self) -> bool {
        self.event == "charge.success"
            && self.data.status.as_deref() == Some("success")
    }
}

/// Request to start a Paystack card payment for an invoice.
#[derive(Debug, Clone, Deserialize)]
pub struct PaystackInitRequest {
    pub invoice_id: uuid::Uuid,
    /// Payer email — Paystack requires it; falls back to the customer's email.
    #[serde(default)]
    pub email: Option<String>,
    /// Where Paystack redirects the payer after payment (the ERP/portal invoice page).
    #[serde(default)]
    pub callback_url: Option<String>,
}

/// The initialised transaction the browser needs to redirect the payer.
#[derive(Debug, Clone, Serialize)]
pub struct PaystackInitResponse {
    pub authorization_url: String,
    pub reference: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn signature_roundtrips_and_rejects_tampering() {
        let secret = "sk_test_example";
        let body = br#"{"event":"charge.success","data":{"reference":"ref_1","amount":150000,"status":"success"}}"#;
        // Compute the signature the way Paystack does.
        let mut mac = Hmac::<Sha512>::new_from_slice(secret.as_bytes()).unwrap();
        mac.update(body);
        let sig = hex::encode(mac.finalize().into_bytes());

        assert!(verify_signature(body, &sig, secret), "valid signature must verify");
        assert!(!verify_signature(body, &sig, "wrong_secret"), "wrong secret must fail");
        assert!(!verify_signature(b"tampered", &sig, secret), "tampered body must fail");
    }

    #[test]
    fn only_successful_charge_records() {
        let ev: PaystackEvent = serde_json::from_slice(
            br#"{"event":"charge.success","data":{"reference":"r","amount":100,"status":"success"}}"#,
        )
        .unwrap();
        assert!(ev.is_successful_charge());

        let failed: PaystackEvent = serde_json::from_slice(
            br#"{"event":"charge.success","data":{"reference":"r","amount":100,"status":"failed"}}"#,
        )
        .unwrap();
        assert!(!failed.is_successful_charge());
    }
}
