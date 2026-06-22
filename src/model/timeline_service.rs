//! Timeline service — fetch home timeline via allGroups → friendstimeline.
//!
//! Data source (per weibo.com HAR analysis):
//!   1. /ajax/feed/allGroups → get list_id for "全部关注" (gid prefix "10001")
//!   2. /ajax/feed/friendstimeline?list_id={gid}&count=25 → timeline JSON
//!   3. Fallback: hotSearch (public API)

use anyhow::Result;

use crate::domain::TimelineItem;
use crate::infra::config;
use crate::infra::http_client;
use crate::infra::cookie_io;
use crate::{log_error, log_info, log_success};

/// Parse timeline items from friendstimeline response JSON.
fn parse_statuses(data: &serde_json::Value) -> Vec<TimelineItem> {
    data
        .get("statuses")
        .and_then(|s| s.as_array())
        .map(|arr| {
            arr.iter()
                .map(|s| {
                    let user_name = s
                        .get("user")
                        .and_then(|u| u.get("screen_name"))
                        .and_then(|v| v.as_str())
                        .unwrap_or("?")
                        .to_string();
                    let text = s
                        .get("text_raw")
                        .or_else(|| s.get("text"))
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string();
                    TimelineItem { user_name, text }
                })
                .collect()
        })
        .unwrap_or_default()
}

/// Get the "全部关注" list_id from allGroups API.
async fn get_following_list_id(client: &reqwest::Client, cookie_header: &str, xsrf: &str) -> Option<String> {
    let resp = client
        .get("https://weibo.com/ajax/feed/allGroups")
        .header("Cookie", cookie_header)
        .header("Referer", config::WEIBO_BASE_URL)
        .header("User-Agent", config::DEFAULT_UA)
        .header("X-Requested-With", "XMLHttpRequest")
        .header("X-XSRF-TOKEN", xsrf)
        .header("Accept", "application/json, text/plain, */*")
        .timeout(config::REQUEST_TIMEOUT)
        .send()
        .await
        .ok()?;

    let data: serde_json::Value = resp.json().await.ok()?;
    let groups = data.get("groups")?.as_array()?;

    for group in groups {
        if let Some(sub_groups) = group.get("group").and_then(|g| g.as_array()) {
            for g in sub_groups {
                if let Some(gid) = g.get("gid").and_then(|v| v.as_str()) {
                    // "全部关注" uses gid prefix "10001"
                    if gid.starts_with("10001") {
                        log_info!("找到关注分组: gid={}, title={}", gid,
                            g.get("title").and_then(|v| v.as_str()).unwrap_or("?"));
                        return Some(gid.to_string());
                    }
                }
            }
        }
    }

    log_info!("allGroups 中未找到 10001 前缀的分组");
    None
}

/// Fetch home timeline via friendstimeline API.
pub async fn fetch_timeline(cookie_header: &str, xsrf: &str) -> Result<Vec<TimelineItem>> {
    let client = http_client::build_no_store();

    // Step 1: Get list_id
    let list_id = match get_following_list_id(&client, cookie_header, xsrf).await {
        Some(id) => id,
        None => {
            log_info!("无法获取 list_id, 回退到热搜榜");
            return fetch_hotsearch().await;
        }
    };

    // Step 2: Fetch timeline
    log_info!("请求 friendstimeline (list_id={})...", &list_id[..list_id.len().min(20)]);
    let resp = client
        .get(format!(
            "https://weibo.com/ajax/feed/friendstimeline?list_id={}&refresh=0&since_id=0&count=25&fid={}",
            list_id, list_id
        ))
        .header("Cookie", cookie_header)
        .header("Referer", config::WEIBO_BASE_URL)
        .header("User-Agent", config::DEFAULT_UA)
        .header("X-Requested-With", "XMLHttpRequest")
        .header("X-XSRF-TOKEN", xsrf)
        .header("Accept", "application/json, text/plain, */*")
        .timeout(config::REQUEST_TIMEOUT)
        .send()
        .await?;

    let data: serde_json::Value = resp.json().await?;
    let ok = data.get("ok").and_then(|v| v.as_i64()).unwrap_or(0);

    if ok != 1 {
        log_error!("friendstimeline 返回 ok={}", ok);
        return fetch_hotsearch().await;
    }

    let items = parse_statuses(&data);
    log_success!("friendstimeline 加载完成: {} 条微博", items.len());
    Ok(items)
}

/// Fetch hotsearch trends (public API, fallback).
pub async fn fetch_hotsearch() -> Result<Vec<TimelineItem>> {
    log_info!("请求热搜榜 API...");
    let data = http_client::public_get(config::API_HOTSEARCH).await?;

    let band = data
        .get("data")
        .and_then(|d| d.get("realtime").or_else(|| d.get("band_list")))
        .and_then(|b| b.as_array());

    let items: Vec<TimelineItem> = band
        .map(|arr| {
            arr.iter()
                .take(config::MAX_HOTSEARCH_ITEMS)
                .map(|item| {
                    let word = item.get("word").and_then(|v| v.as_str()).unwrap_or("?").to_string();
                    let num = item.get("num").and_then(|v| v.as_i64()).unwrap_or(0);
                    let note = item.get("note").or_else(|| item.get("category")).and_then(|v| v.as_str()).unwrap_or("");
                    let text = if num > 0 && !note.is_empty() {
                        format!("🔥 热度 {} — {}", num, note)
                    } else if num > 0 {
                        format!("🔥 热度 {}", num)
                    } else if !note.is_empty() {
                        format!("[{}]", note)
                    } else {
                        String::new()
                    };
                    TimelineItem { user_name: word, text }
                })
                .collect()
        })
        .unwrap_or_default();

    log_success!("热搜榜加载完成: {} 条", items.len());
    Ok(items)
}

/// Build full cookie header from saved file (all cookies, not just SUB+SUBP).
fn build_full_cookie_header() -> String {
    cookie_io::load_full()
}

/// Extract XSRF token from saved cookies.
fn get_xsrf_token() -> Option<String> {
    cookie_io::load_xsrf()
}

/// Main entry: fetch home content.
/// Returns (items, title).
pub async fn fetch_home_content(_cookie: &str) -> (Vec<TimelineItem>, String) {
    let cookie_header = build_full_cookie_header();
    let xsrf = get_xsrf_token().unwrap_or_default();

    log_info!("加载首页时间线 (friendstimeline)...");
    match fetch_timeline(&cookie_header, &xsrf).await {
        Ok(items) if !items.is_empty() => {
            let title = format!("📰 首页时间线 ({}条)", items.len());
            (items, title)
        }
        Ok(_) => {
            log_info!("friendstimeline 返回空, 回退热搜榜");
            match fetch_hotsearch().await {
                Ok(items) => {
                    let title = format!("🔥 热搜榜 ({}条)", items.len());
                    (items, title)
                }
                Err(e) => {
                    log_error!("热搜榜加载失败: {}", e);
                    (vec![], "加载失败".into())
                }
            }
        }
        Err(e) => {
            log_error!("friendstimeline 失败: {}, 回退热搜榜", e);
            match fetch_hotsearch().await {
                Ok(items) => {
                    let title = format!("🔥 热搜榜 ({}条)", items.len());
                    (items, title)
                }
                Err(e) => {
                    log_error!("热搜榜加载失败: {}", e);
                    (vec![], "加载失败".into())
                }
            }
        }
    }
}
