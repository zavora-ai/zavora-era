//! OCR provider selection and the HTTP-backed xberg sidecar provider.
//!
//! The domain crate (`zavora-erp-core`) defines the [`OcrProvider`] trait, the
//! default [`ManualReviewProvider`], and the pure xberg-output mapping. This
//! module — living in the API crate so the domain stays free of network deps —
//! adds the concrete HTTP provider that talks to an `xberg serve` sidecar and a
//! small factory that picks a provider from environment configuration.
//!
//! Configuration (mirrors the M-Pesa "configured per deployment" convention):
//!   * `OCR_PROVIDER` — `manual` (default) or `xberg`.
//!   * `XBERG_URL`    — base URL of the sidecar, e.g. `http://127.0.0.1:8000`.
//!   * `XBERG_OCR_TIMEOUT_SECS` — request timeout (default 30).
//!
//! When `OCR_PROVIDER=xberg` but `XBERG_URL` is missing, we log a warning and
//! fall back to manual review rather than failing startup — the feature still
//! works, just without auto-extraction. At request time, if the sidecar is
//! unreachable or returns an error, the provider returns an empty,
//! zero-confidence result so the capture falls through to mandatory human
//! review instead of failing the upload.

use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use zavora_erp_core::error::ErpResult;
use zavora_erp_core::payments::receipt_capture::OcrResult;
use zavora_erp_core::services::ocr_provider::{
    empty_result, ocr_from_xberg_rest, ManualReviewProvider, OcrInput, OcrProvider,
};

/// Build the configured OCR provider from the environment. Always succeeds:
/// unknown or misconfigured settings degrade to [`ManualReviewProvider`].
pub fn provider_from_env() -> Arc<dyn OcrProvider> {
    match std::env::var("OCR_PROVIDER").unwrap_or_default().trim().to_lowercase().as_str() {
        "xberg" => match std::env::var("XBERG_URL").ok().filter(|u| !u.trim().is_empty()) {
            Some(url) => {
                let timeout = std::env::var("XBERG_OCR_TIMEOUT_SECS")
                    .ok()
                    .and_then(|v| v.trim().parse::<u64>().ok())
                    .filter(|&v| v > 0)
                    .unwrap_or(30);
                tracing::info!(url = %url, timeout_secs = timeout, "OCR provider: xberg sidecar");
                Arc::new(XbergHttpProvider::new(url, timeout))
            }
            None => {
                tracing::warn!(
                    "OCR_PROVIDER=xberg but XBERG_URL is not set; falling back to manual review"
                );
                Arc::new(ManualReviewProvider)
            }
        },
        other => {
            if !other.is_empty() && other != "manual" {
                tracing::warn!(provider = %other, "unknown OCR_PROVIDER; using manual review");
            }
            Arc::new(ManualReviewProvider)
        }
    }
}

/// HTTP-backed provider that POSTs the receipt image to an `xberg serve`
/// sidecar and maps its `Structured` output into our [`OcrResult`].
pub struct XbergHttpProvider {
    client: reqwest::Client,
    base_url: String,
}

impl XbergHttpProvider {
    pub fn new(base_url: String, timeout_secs: u64) -> Self {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(timeout_secs))
            .build()
            .unwrap_or_default();
        Self {
            client,
            base_url: base_url.trim_end_matches('/').to_string(),
        }
    }

    /// The xberg extract endpoint, configured to return the OCR-rich
    /// The xberg REST extract endpoint.
    fn extract_url(&self) -> String {
        format!("{}/extract", self.base_url)
    }
}

#[async_trait]
impl OcrProvider for XbergHttpProvider {
    fn name(&self) -> &'static str {
        "xberg"
    }

    async fn extract(&self, input: &OcrInput) -> ErpResult<OcrResult> {
        let part = reqwest::multipart::Part::bytes(input.bytes.clone())
            .file_name(input.filename.clone())
            .mime_str(&input.mime_type)
            .unwrap_or_else(|_| reqwest::multipart::Part::bytes(input.bytes.clone()));
        // The REST API expects a repeatable `files` field and an optional
        // `config`. force_ocr ensures image/scanned receipts are OCR'd.
        let form = reqwest::multipart::Form::new()
            .part("files", part)
            .text("config", r#"{"force_ocr":true,"ocr":{"language":"eng"}}"#);

        // Any transport/parse failure degrades to an empty result (forces human
        // review) — an OCR outage must never block the user's upload.
        let resp = match self.client.post(self.extract_url()).multipart(form).send().await {
            Ok(r) => r,
            Err(e) => {
                tracing::warn!(error = %e, "xberg OCR request failed; falling back to manual review");
                return Ok(empty_result());
            }
        };

        if !resp.status().is_success() {
            tracing::warn!(status = %resp.status(), "xberg OCR returned non-success; manual review");
            return Ok(empty_result());
        }

        let value: serde_json::Value = match resp.json().await {
            Ok(v) => v,
            Err(e) => {
                tracing::warn!(error = %e, "xberg OCR response was not JSON; manual review");
                return Ok(empty_result());
            }
        };

        // The REST /extract response is `{ results: [ { content, metadata,
        // detected_languages, ... } ], ... }` — recognised text lives in
        // `results[0].content`. Map that into our OcrResult.
        let result = value
            .get("results")
            .and_then(|r| r.as_array())
            .and_then(|a| a.first())
            .cloned()
            .unwrap_or(value);

        Ok(ocr_from_xberg_rest(&result))
    }
}
