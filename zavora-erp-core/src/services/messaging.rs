//! Outbound SMS and WhatsApp delivery providers for the notification worker.
//!
//! Email (SMTP via `lettre`) and InApp delivery are handled directly in the
//! worker. This module adds the two remaining channels as **env-gated HTTP
//! providers**, following the same "configured per deployment, clear error when
//! not" convention as the M-Pesa gateway and the OCR sidecar:
//!
//! * **SMS** — Africa's Talking (the dominant Kenyan SMS gateway). Set
//!   `AT_USERNAME` + `AT_API_KEY` (+ optional `AT_SENDER_ID`).
//! * **WhatsApp** — Twilio WhatsApp. Set `TWILIO_ACCOUNT_SID` +
//!   `TWILIO_AUTH_TOKEN` + `TWILIO_WHATSAPP_FROM` (e.g. `whatsapp:+14155238886`).
//!
//! Both providers are built once at worker startup from the environment. When a
//! channel is not configured, the builder returns `None` and the worker reports
//! a clear "channel not configured" error (the prior behaviour, now real when
//! configured). Transient HTTP failures surface as `Err(_)` so the worker's
//! existing retry/backoff applies.

use std::time::Duration;

/// Default request timeout for provider HTTP calls.
const DEFAULT_TIMEOUT_SECS: u64 = 15;

/// Normalise a phone number to E.164-ish digits for Kenyan numbers.
///
/// Accepts common local formats and returns a `+2547XXXXXXXX`-style string:
/// * `0712345678`        → `+254712345678`
/// * `712345678`         → `+254712345678`
/// * `254712345678`      → `+254712345678`
/// * `+254712345678`     → unchanged
///
/// Non-Kenyan numbers already in `+<country><digits>` form are preserved. Spaces,
/// hyphens and parentheses are stripped. Returns the cleaned string; callers that
/// require strict validation should check the result starts with `+`.
pub fn normalize_phone(raw: &str) -> String {
    // Strip everything except digits and a leading '+'.
    let mut s = String::with_capacity(raw.len());
    for (i, c) in raw.chars().enumerate() {
        if c == '+' && i == 0 {
            s.push('+');
        } else if c.is_ascii_digit() {
            s.push(c);
        }
    }

    if let Some(rest) = s.strip_prefix('+') {
        // Already international.
        return format!("+{rest}");
    }
    if let Some(rest) = s.strip_prefix("254") {
        return format!("+254{rest}");
    }
    if let Some(rest) = s.strip_prefix('0') {
        // Local trunk-prefixed number → Kenyan E.164.
        return format!("+254{rest}");
    }
    // Bare subscriber number (e.g. starts with 7 or 1) → assume Kenyan.
    if s.len() == 9 && (s.starts_with('7') || s.starts_with('1')) {
        return format!("+254{s}");
    }
    // Fall back: prefix '+' so downstream gets an international-looking value.
    format!("+{s}")
}

// ---------------------------------------------------------------------------
// SMS — Africa's Talking
// ---------------------------------------------------------------------------

/// Africa's Talking SMS provider configuration, loaded from the environment.
#[derive(Debug, Clone)]
pub struct SmsProvider {
    client: reqwest::Client,
    username: String,
    api_key: String,
    sender_id: Option<String>,
    base_url: String,
}

impl SmsProvider {
    /// Build from env. Returns `None` when SMS is not configured (`AT_USERNAME`
    /// or `AT_API_KEY` missing/empty).
    pub fn from_env() -> Option<Self> {
        let username = non_empty_env("AT_USERNAME")?;
        let api_key = non_empty_env("AT_API_KEY")?;
        let sender_id = non_empty_env("AT_SENDER_ID");
        // Sandbox username uses the sandbox endpoint automatically.
        let base_url = std::env::var("AT_BASE_URL").ok().filter(|v| !v.trim().is_empty()).unwrap_or_else(|| {
            if username == "sandbox" {
                "https://api.sandbox.africastalking.com".to_string()
            } else {
                "https://api.africastalking.com".to_string()
            }
        });
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(DEFAULT_TIMEOUT_SECS))
            .build()
            .ok()?;
        Some(Self { client, username, api_key, sender_id, base_url })
    }

    fn send_url(&self) -> String {
        format!("{}/version1/messaging", self.base_url.trim_end_matches('/'))
    }

    /// Send an SMS to `recipient`. Returns `Ok(())` on a 2xx provider response.
    pub async fn send(&self, recipient: &str, body: &str) -> Result<(), String> {
        let to = normalize_phone(recipient);
        let mut form: Vec<(&str, String)> = vec![
            ("username", self.username.clone()),
            ("to", to),
            ("message", body.to_string()),
        ];
        if let Some(ref from) = self.sender_id {
            form.push(("from", from.clone()));
        }

        let resp = self
            .client
            .post(self.send_url())
            .header("apiKey", &self.api_key)
            .header(reqwest::header::ACCEPT, "application/json")
            .form(&form)
            .send()
            .await
            .map_err(|e| format!("Africa's Talking request failed: {e}"))?;

        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();
        if !status.is_success() {
            return Err(format!("Africa's Talking HTTP {status}: {text}"));
        }
        // AT returns 201 with a recipients array; a per-recipient status of
        // anything other than "Success"/"Sent" indicates a logical failure.
        if let Ok(v) = serde_json::from_str::<serde_json::Value>(&text) {
            if let Some(recipients) = v
                .get("SMSMessageData")
                .and_then(|d| d.get("Recipients"))
                .and_then(|r| r.as_array())
            {
                if recipients.is_empty() {
                    return Err(format!("Africa's Talking accepted no recipients: {text}"));
                }
                for r in recipients {
                    let rstatus = r.get("status").and_then(|s| s.as_str()).unwrap_or("");
                    if !rstatus.eq_ignore_ascii_case("Success") && !rstatus.eq_ignore_ascii_case("Sent") {
                        return Err(format!("Africa's Talking recipient status '{rstatus}': {text}"));
                    }
                }
            }
        }
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// WhatsApp — Twilio
// ---------------------------------------------------------------------------

/// Twilio WhatsApp provider configuration, loaded from the environment.
#[derive(Debug, Clone)]
pub struct WhatsAppProvider {
    client: reqwest::Client,
    account_sid: String,
    auth_token: String,
    from: String,
    base_url: String,
}

impl WhatsAppProvider {
    /// Build from env. Returns `None` when WhatsApp is not configured
    /// (`TWILIO_ACCOUNT_SID`, `TWILIO_AUTH_TOKEN`, or `TWILIO_WHATSAPP_FROM`
    /// missing/empty).
    pub fn from_env() -> Option<Self> {
        let account_sid = non_empty_env("TWILIO_ACCOUNT_SID")?;
        let auth_token = non_empty_env("TWILIO_AUTH_TOKEN")?;
        let from = non_empty_env("TWILIO_WHATSAPP_FROM").map(|f| ensure_whatsapp_prefix(&f))?;
        let base_url = std::env::var("TWILIO_BASE_URL")
            .ok()
            .filter(|v| !v.trim().is_empty())
            .unwrap_or_else(|| "https://api.twilio.com".to_string());
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(DEFAULT_TIMEOUT_SECS))
            .build()
            .ok()?;
        Some(Self { client, account_sid, auth_token, from, base_url })
    }

    fn send_url(&self) -> String {
        format!(
            "{}/2010-04-01/Accounts/{}/Messages.json",
            self.base_url.trim_end_matches('/'),
            self.account_sid
        )
    }

    /// Send a WhatsApp message to `recipient`. Returns `Ok(())` on a 2xx.
    pub async fn send(&self, recipient: &str, body: &str) -> Result<(), String> {
        let to = ensure_whatsapp_prefix(&normalize_phone(recipient));
        let form = vec![
            ("To", to),
            ("From", self.from.clone()),
            ("Body", body.to_string()),
        ];

        let resp = self
            .client
            .post(self.send_url())
            .basic_auth(&self.account_sid, Some(&self.auth_token))
            .header(reqwest::header::ACCEPT, "application/json")
            .form(&form)
            .send()
            .await
            .map_err(|e| format!("Twilio request failed: {e}"))?;

        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();
        if !status.is_success() {
            return Err(format!("Twilio HTTP {status}: {text}"));
        }
        Ok(())
    }
}

/// Ensure a Twilio WhatsApp address carries the `whatsapp:` scheme prefix.
fn ensure_whatsapp_prefix(s: &str) -> String {
    if s.starts_with("whatsapp:") {
        s.to_string()
    } else {
        format!("whatsapp:{s}")
    }
}

/// Reduce an HTML notification body to plain text suitable for SMS/WhatsApp.
///
/// Notification bodies are authored as HTML for email. For text channels we
/// strip tags, convert a few block elements to newlines, decode the most common
/// entities, and collapse whitespace. This is a deliberately small, dependency-
/// free reducer — not a full HTML parser — which is sufficient for the simple,
/// template-generated bodies this system emits.
pub fn html_to_text(html: &str) -> String {
    let mut out = String::with_capacity(html.len());
    let mut in_tag = false;
    let mut tag = String::new();

    for c in html.chars() {
        match c {
            '<' => {
                in_tag = true;
                tag.clear();
            }
            '>' => {
                in_tag = false;
                let t = tag.trim().to_lowercase();
                // Block-level / line-break tags become newlines.
                if t == "br" || t == "br/" || t.starts_with("br ")
                    || t == "/p" || t == "/div" || t == "/tr" || t == "/li"
                    || t == "/h1" || t == "/h2" || t == "/h3"
                {
                    out.push('\n');
                }
            }
            _ if in_tag => tag.push(c),
            _ => out.push(c),
        }
    }

    // Decode the handful of entities our templates use.
    let decoded = out
        .replace("&nbsp;", " ")
        .replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'");

    // Collapse runs of blank lines and trim each line.
    let mut lines: Vec<String> = decoded
        .lines()
        .map(|l| l.split_whitespace().collect::<Vec<_>>().join(" "))
        .collect();
    lines.retain(|l| !l.is_empty());
    lines.join("\n")
}

/// Read an environment variable, returning `None` when unset or blank.
fn non_empty_env(key: &str) -> Option<String> {
    std::env::var(key).ok().map(|v| v.trim().to_string()).filter(|v| !v.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_kenyan_local_formats() {
        assert_eq!(normalize_phone("0712345678"), "+254712345678");
        assert_eq!(normalize_phone("712345678"), "+254712345678");
        assert_eq!(normalize_phone("254712345678"), "+254712345678");
        assert_eq!(normalize_phone("+254712345678"), "+254712345678");
        assert_eq!(normalize_phone("0712 345 678"), "+254712345678");
        assert_eq!(normalize_phone("(0712)-345-678"), "+254712345678");
    }

    #[test]
    fn normalizes_safaricom_and_airtel_prefixes() {
        // 01x numbers (Airtel/fixed) are also 9-digit subscriber numbers.
        assert_eq!(normalize_phone("0110000000"), "+254110000000");
        assert_eq!(normalize_phone("110000000"), "+254110000000");
    }

    #[test]
    fn preserves_other_international_numbers() {
        assert_eq!(normalize_phone("+14155238886"), "+14155238886");
        assert_eq!(normalize_phone("+44 20 7946 0958"), "+442079460958");
    }

    #[test]
    fn whatsapp_prefix_is_idempotent() {
        assert_eq!(ensure_whatsapp_prefix("+254712345678"), "whatsapp:+254712345678");
        assert_eq!(ensure_whatsapp_prefix("whatsapp:+254712345678"), "whatsapp:+254712345678");
    }

    #[test]
    fn html_body_reduces_to_plain_text() {
        let html = "<p>Hi Jane,</p><p>Invoice <b>INV-001</b> for <strong>Ksh&nbsp;1,000</strong> is due.</p><br/>Thanks &amp; regards";
        let text = html_to_text(html);
        assert!(text.contains("Hi Jane,"));
        assert!(text.contains("Invoice INV-001 for Ksh 1,000 is due."));
        assert!(text.contains("Thanks & regards"));
        // No angle-bracket tags remain.
        assert!(!text.contains('<') && !text.contains('>'));
        // Paragraphs became separate lines.
        assert!(text.lines().count() >= 2);
    }

    #[test]
    fn providers_are_none_when_unconfigured() {
        // These env vars are not set in the test environment.
        // (If a developer has them set locally, this test is skipped implicitly
        // by only asserting the negative when unset.)
        if std::env::var("AT_API_KEY").is_err() {
            assert!(SmsProvider::from_env().is_none());
        }
        if std::env::var("TWILIO_AUTH_TOKEN").is_err() {
            assert!(WhatsAppProvider::from_env().is_none());
        }
    }
}
