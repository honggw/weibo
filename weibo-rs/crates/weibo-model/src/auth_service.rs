//! Authentication service — wraps QrLogin for a clean, testable API.
//!
//! Each method is a small, focused async operation (suitable for short block_on calls).

use anyhow::Result;

use weibo_infra::config;
use weibo_infra::http_client;
use weibo_infra::{log_info, log_success};

use crate::qr_login::QrLogin;
use weibo_domain::QrStatus;

/// Phase 1: Warm up + fetch QR code → returns (QrLogin, png_bytes).
pub async fn prepare_qr() -> Result<(QrLogin, Vec<u8>)> {
    log_info!("[auth] warmup...");
    let mut login = QrLogin::new()?;
    login.warmup().await?;
    log_success!("[auth] warmup 完成");

    log_info!("[auth] 获取二维码...");
    login.fetch_qr_code().await?;
    login.download_qr_image().await?;

    let bytes = login
        .qr_image_bytes()
        .cloned()
        .ok_or_else(|| anyhow::anyhow!("二维码数据为空"))?;
    log_info!("[auth] QR ready: {} bytes", bytes.len());

    // Save to disk for debugging (under data directory)
    let _ = std::fs::write(config::data_path(config::QR_IMAGE_FILE), &bytes);

    Ok((login, bytes))
}

/// Phase 2: Single poll of QR status.
pub async fn poll_qr(login: &QrLogin) -> Result<QrStatus> {
    login.poll_status().await
}

/// Phase 3: Refresh expired QR code → returns new png_bytes.
pub async fn refresh_qr(login: &mut QrLogin) -> Result<Vec<u8>> {
    log_info!("[auth] QR 过期, 刷新...");
    login.fetch_qr_code().await?;
    login.download_qr_image().await?;

    let bytes = login
        .qr_image_bytes()
        .cloned()
        .ok_or_else(|| anyhow::anyhow!("QR 刷新失败"))?;

    let _ = std::fs::write(config::data_path(config::QR_IMAGE_FILE), &bytes);
    Ok(bytes)
}

/// Phase 4: Exchange alt ticket → returns cookie_header string.
pub async fn exchange_ticket(
    login: &mut QrLogin,
    alt: &str,
    redirect_url: &str,
) -> Result<String> {
    login.exchange_ticket_with_url(alt, redirect_url).await?;

    let verified = login.verify_login().await.unwrap_or(false);
    if !verified {
        return Err(anyhow::anyhow!("登录验证失败"));
    }

    // Save cookies persistently (under data directory)
    login
        .save_cookies_to_file(&config::data_path(config::COOKIE_FILE))
        .ok();

    let cookie_header = login.get_cookie_header();
    log_success!("[auth] 登录成功, Cookie 已保存");
    Ok(cookie_header)
}

/// Verify if a cookie header is still valid.
pub async fn verify_cookie(header: &str) -> Result<bool> {
    let client = http_client::build_no_store();
    let resp = client
        .get(config::API_CONFIG)
        .header("Cookie", header)
        .headers(http_client::minimal_headers())
        .send()
        .await?;

    let data: serde_json::Value = resp.json().await?;
    Ok(data.get("ok").and_then(|v| v.as_i64()).unwrap_or(0) == 1)
}

/// Load saved cookie from file → returns header string if valid SUB exists.
pub fn load_saved_cookie() -> Option<String> {
    weibo_infra::cookie_io::load().map(|c| c.header)
}

/// Delete saved cookie file (logout).
pub fn delete_saved_cookie() {
    weibo_infra::cookie_io::delete();
}
