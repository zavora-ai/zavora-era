//! Minimal Zavora ERA API client for the Amos UI's live business snapshot.
//! (Agent tool calls go through mcp-erp; this client only feeds the UI panel.)

use anyhow::{Result, anyhow};
use serde_json::{Value, json};
use std::time::{Duration, Instant};
use tokio::sync::RwLock;

const TOKEN_MAX_AGE: Duration = Duration::from_secs(12 * 60);

pub struct ErpClient {
    http: reqwest::Client,
    base: String,
    email: String,
    password: String,
    token: RwLock<Option<(String, Instant)>>,
}

impl ErpClient {
    pub fn from_env() -> Result<Self> {
        let base = std::env::var("ZAVORA_API_URL").unwrap_or_else(|_| "http://localhost:8080".into());
        Ok(Self {
            http: reqwest::Client::new(),
            base: format!("{}/api/v1", base.trim_end_matches('/')),
            email: std::env::var("ZAVORA_EMAIL").map_err(|_| anyhow!("ZAVORA_EMAIL not set"))?,
            password: std::env::var("ZAVORA_PASSWORD").map_err(|_| anyhow!("ZAVORA_PASSWORD not set"))?,
            token: RwLock::new(None),
        })
    }

    async fn token(&self) -> Result<String> {
        if let Some((t, at)) = self.token.read().await.as_ref() {
            if at.elapsed() < TOKEN_MAX_AGE {
                return Ok(t.clone());
            }
        }
        let body: Value = self
            .http
            .post(format!("{}/auth/login", self.base))
            .json(&json!({"email": self.email, "password": self.password}))
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?;
        let token = body["access_token"]
            .as_str()
            .ok_or_else(|| anyhow!("login response missing access_token"))?
            .to_string();
        *self.token.write().await = Some((token.clone(), Instant::now()));
        Ok(token)
    }

    pub async fn dashboard(&self) -> Result<Value> {
        let token = self.token().await?;
        Ok(self
            .http
            .get(format!("{}/dashboard", self.base))
            .header("Authorization", format!("Bearer {token}"))
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?)
    }
}
