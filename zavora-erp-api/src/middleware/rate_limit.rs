//! A small per-IP fixed-window rate limiter for the credential-facing routes
//! (login, registration, password-driven portal auth). It is a backstop against
//! credential stuffing / brute force, not a general traffic shaper — process-
//! local (per instance) and in-memory, which is sufficient for the single-VM
//! deployment. Tunable via `LOGIN_RATE_LIMIT` (max attempts, default 10) and
//! `LOGIN_RATE_WINDOW_SECS` (window, default 60). Set the max to 0 to disable.

use std::collections::HashMap;
use std::net::IpAddr;
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

use axum::{
    extract::ConnectInfo,
    http::StatusCode,
    middleware::Next,
    response::{IntoResponse, Response},
};

struct Window {
    start: Instant,
    count: u32,
}

fn buckets() -> &'static Mutex<HashMap<IpAddr, Window>> {
    static BUCKETS: OnceLock<Mutex<HashMap<IpAddr, Window>>> = OnceLock::new();
    BUCKETS.get_or_init(|| Mutex::new(HashMap::new()))
}

fn limit() -> u32 {
    std::env::var("LOGIN_RATE_LIMIT").ok().and_then(|v| v.parse().ok()).unwrap_or(10)
}

fn window_secs() -> u64 {
    std::env::var("LOGIN_RATE_WINDOW_SECS").ok().and_then(|v| v.parse().ok()).unwrap_or(60)
}

/// Returns true when this IP is over its allowance for the current window.
fn is_rate_limited(ip: IpAddr) -> bool {
    let max = limit();
    if max == 0 {
        return false; // disabled
    }
    let window = Duration::from_secs(window_secs());
    let now = Instant::now();
    let mut map = buckets().lock().unwrap();

    // Opportunistic cleanup so the map doesn't grow unbounded across many IPs.
    if map.len() > 10_000 {
        map.retain(|_, w| now.duration_since(w.start) < window);
    }

    let entry = map.entry(ip).or_insert(Window { start: now, count: 0 });
    if now.duration_since(entry.start) >= window {
        entry.start = now;
        entry.count = 0;
    }
    entry.count += 1;
    entry.count > max
}

/// Axum middleware for the auth routes. 429 with a `Retry-After` when over limit.
pub async fn limit_login(
    ConnectInfo(peer): ConnectInfo<std::net::SocketAddr>,
    req: axum::extract::Request,
    next: Next,
) -> Response {
    if is_rate_limited(peer.ip()) {
        tracing::warn!(peer = %peer.ip(), "login rate limit exceeded");
        return (
            StatusCode::TOO_MANY_REQUESTS,
            [(axum::http::header::RETRY_AFTER, window_secs().to_string())],
            "Too many attempts. Please wait and try again.",
        )
            .into_response();
    }
    next.run(req).await
}
