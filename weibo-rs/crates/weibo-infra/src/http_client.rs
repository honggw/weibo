//! HTTP client factory — unified timeout, UA, and common request helpers.

use reqwest::header::{HeaderMap, HeaderValue, REFERER, USER_AGENT};
use reqwest::Client;

use super::config;

/// Build a reqwest Client with standard timeout and headers.
pub fn build() -> Client {
    Client::builder()
        .timeout(config::REQUEST_TIMEOUT)
        .build()
        .expect("failed to build HTTP client")
}

/// Build a Client without cookie_store (for sending explicit Cookie headers).
pub fn build_no_store() -> Client {
    Client::builder()
        .cookie_store(false)
        .timeout(config::REQUEST_TIMEOUT)
        .build()
        .expect("failed to build HTTP client")
}

/// Standard API headers (without Cookie — add separately).
pub fn api_headers() -> HeaderMap {
    let mut h = HeaderMap::new();
    h.insert(USER_AGENT, HeaderValue::from_static(config::DEFAULT_UA));
    h.insert(REFERER, HeaderValue::from_static(config::WEIBO_BASE_URL));
    h.insert("X-Requested-With", HeaderValue::from_static("XMLHttpRequest"));
    h
}

/// Minimal API headers (no Accept — avoids Weibo 404 issue).
pub fn minimal_headers() -> HeaderMap {
    let mut h = HeaderMap::new();
    h.insert(USER_AGENT, HeaderValue::from_static("Mozilla/5.0"));
    h.insert(REFERER, HeaderValue::from_static(config::WEIBO_BASE_URL));
    h
}

/// Perform an authenticated GET request (with Cookie header).
pub async fn auth_get(url: &str, cookie: &str) -> anyhow::Result<serde_json::Value> {
    let client = build_no_store();
    let resp = client
        .get(url)
        .header("Cookie", cookie)
        .headers(minimal_headers())
        .send()
        .await?;
    let data = resp.json().await?;
    Ok(data)
}

/// Perform an unauthenticated GET request (public API).
pub async fn public_get(url: &str) -> anyhow::Result<serde_json::Value> {
    let client = build();
    let resp = client
        .get(url)
        .headers(minimal_headers())
        .send()
        .await?;
    let data = resp.json().await?;
    Ok(data)
}
