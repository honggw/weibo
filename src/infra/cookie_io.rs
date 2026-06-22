//! Cookie file persistence — load, save, delete `weibo_cookies.json`.

use std::collections::HashMap;
use std::path::Path;

use crate::domain::CookieData;
use crate::{log_error, log_info};

use super::config;

/// Load cookies from file, return None if file doesn't exist or has no SUB.
pub fn load() -> Option<CookieData> {
    let path = config::COOKIE_FILE;
    let data = std::fs::read_to_string(path).ok()?;
    let parsed: serde_json::Value = serde_json::from_str(&data).ok()?;

    let cookie_map: HashMap<String, String> = parsed
        .get("cookies")?
        .as_array()?
        .iter()
        .filter_map(|c| {
            let name = c.get("name")?.as_str()?;
            let value = c.get("value")?.as_str()?;
            Some((name.to_string(), value.to_string()))
        })
        .collect();

    let sub = cookie_map.get("SUB")?;
    if sub.is_empty() {
        return None;
    }

    // Build header from SUB + SUBP only (other cookies may invalidate session)
    let mut parts = vec![format!("SUB={}", sub)];
    if let Some(subp) = cookie_map.get("SUBP") {
        parts.push(format!("SUBP={}", subp));
    }
    let header = parts.join("; ");

    log_info!(
        "从文件加载 Cookie (SUB={}...), 共 {} 个键",
        &sub[..sub.len().min(30)],
        header.split(';').count()
    );

    Some(CookieData {
        header,
        sub: sub.clone(),
    })
}

/// Save cookies from a QrLogin session to file.
/// `cookie_json` should be the raw JSON output from QrLogin::save_cookies_to_file.
pub fn save_from_json(json: &str) {
    if let Err(e) = std::fs::write(config::COOKIE_FILE, json) {
        log_error!("保存 Cookie 到文件失败: {}", e);
    } else {
        log_info!("Cookie 已保存到 {}", config::COOKIE_FILE);
    }
}

/// Delete the cookie file (logout).
pub fn delete() {
    if let Err(e) = std::fs::remove_file(config::COOKIE_FILE) {
        log_info!("删除 Cookie 文件失败 (可能不存在): {}", e);
    } else {
        log_info!("已删除 Cookie 文件 {}", config::COOKIE_FILE);
    }
}

/// Check if cookie file exists.
pub fn exists() -> bool {
    Path::new(config::COOKIE_FILE).exists()
}

/// Load ALL cookies from file as a full Cookie header string (for friendstimeline API).
pub fn load_full() -> String {
    let data = match std::fs::read_to_string(config::COOKIE_FILE) {
        Ok(d) => d,
        Err(_) => return String::new(),
    };
    let parsed: serde_json::Value = match serde_json::from_str(&data) {
        Ok(p) => p,
        Err(_) => return String::new(),
    };

    let parts: Vec<String> = parsed
        .get("cookies")
        .and_then(|c| c.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|c| {
                    let name = c.get("name")?.as_str()?;
                    let value = c.get("value")?.as_str()?;
                    Some(format!("{}={}", name, value))
                })
                .collect()
        })
        .unwrap_or_default();

    let header = parts.join("; ");
    log_info!("构建完整 Cookie header: {} 个键", parts.len());
    header
}

/// Extract XSRF-TOKEN value from saved cookies.
pub fn load_xsrf() -> Option<String> {
    let data = std::fs::read_to_string(config::COOKIE_FILE).ok()?;
    let parsed: serde_json::Value = serde_json::from_str(&data).ok()?;
    parsed
        .get("cookies")
        .and_then(|c| c.as_array())
        .and_then(|arr| {
            arr.iter().find_map(|c| {
                if c.get("name")?.as_str()? == "XSRF-TOKEN" {
                    c.get("value")?.as_str().map(String::from)
                } else {
                    None
                }
            })
        })
}
