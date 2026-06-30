//! Timeline service — fetch home timeline via allGroups → friendstimeline.
//!
//! Data source (per weibo.com HAR analysis):
//!   1. /ajax/feed/allGroups → get list_id for "全部关注" (gid prefix "10001")
//!   2. /ajax/feed/friendstimeline?list_id={gid}&count=25 → timeline JSON
//!   3. Fallback: hotSearch (public API)

use anyhow::Result;

use weibo_domain::TimelineItem;
use weibo_infra::config;
use weibo_infra::http_client;
use weibo_infra::cookie_io;
use weibo_infra::{log_error, log_info, log_success};

/// Parsed timeline result: items + pagination info.
pub struct TimelineResult {
    pub items: Vec<TimelineItem>,
    /// Last status ID for "load more"
    pub since_id: String,
    /// The feed list_id (for subsequent pagination calls)
    pub feed_list_id: String,
}

/// Parse timeline items + extract since_id from last status.
fn parse_statuses(data: &serde_json::Value) -> TimelineResult {
    let items: Vec<TimelineItem> = data
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
        .unwrap_or_default();

    let since_id = data
        .get("statuses")
        .and_then(|s| s.as_array())
        .and_then(|arr| arr.last())
        .and_then(|s| s.get("id").or_else(|| s.get("idstr")))
        .and_then(|v| v.as_u64())
        .map(|id| id.to_string())
        .unwrap_or_default();

    TimelineResult { items, since_id, feed_list_id: String::new() }
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
pub async fn fetch_timeline(
    cookie_header: &str,
    xsrf: &str,
    feed_list_id: &Option<String>,
    since_id: &str,
) -> Result<TimelineResult> {
    let client = http_client::build_no_store();

    // Step 1: Get list_id (only on first page)
    let list_id = if let Some(ref id) = feed_list_id {
        id.clone()
    } else {
        match get_following_list_id(&client, cookie_header, xsrf).await {
            Some(id) => id,
            None => {
                log_info!("无法获取 list_id");
                return Err(anyhow::anyhow!("无法获取 list_id"));
            }
        }
    };

    // Step 2: Fetch timeline
    let since = if since_id.is_empty() { "0" } else { since_id };
    log_info!("请求 friendstimeline (list_id={}, since_id={})...",
        &list_id[..list_id.len().min(20)], since);
    let resp = client
        .get(format!(
            "https://weibo.com/ajax/feed/friendstimeline?list_id={}&refresh=0&since_id={}&count=25&fid={}",
            list_id, since, list_id
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
        return Err(anyhow::anyhow!("friendstimeline ok={}", ok));
    }

    let mut result = parse_statuses(&data);
    result.feed_list_id = list_id;
    log_success!("friendstimeline: {} 条, since_id={}", result.items.len(), result.since_id);
    Ok(result)
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

/// Build full cookie header from saved file.
fn build_full_cookie_header() -> String {
    cookie_io::load_full()
}

/// Extract XSRF token from saved cookies.
fn get_xsrf_token() -> Option<String> {
    cookie_io::load_xsrf()
}

/// Main entry: fetch first page of home content.
pub async fn fetch_first_page() -> (Vec<TimelineItem>, String, Option<String>, String) {
    let cookie_header = build_full_cookie_header();
    let xsrf = get_xsrf_token().unwrap_or_default();

    log_info!("加载首页时间线 (friendstimeline)...");
    match fetch_timeline(&cookie_header, &xsrf, &None, "").await {
        Ok(result) if !result.items.is_empty() => {
            let title = format!("📰 首页时间线 ({}条)", result.items.len());
            let feed_list_id = result.feed_list_id.clone();
            (result.items, title, Some(feed_list_id), result.since_id)
        }
        Ok(_) => fallback_hotsearch().await,
        Err(e) => {
            log_error!("friendstimeline 失败: {}, 回退热搜榜", e);
            fallback_hotsearch().await
        }
    }
}

/// Load more items (pagination).
pub async fn load_more(
    since_id: &str,
    feed_list_id: &Option<String>,
) -> (Vec<TimelineItem>, String) {
    let cookie_header = build_full_cookie_header();
    let xsrf = get_xsrf_token().unwrap_or_default();

    match fetch_timeline(&cookie_header, &xsrf, feed_list_id, since_id).await {
        Ok(result) => {
            log_info!("加载更多: {} 条, new_since_id={}", result.items.len(), result.since_id);
            (result.items, result.since_id)
        }
        Err(e) => {
            log_error!("加载更多失败: {}", e);
            (vec![], since_id.to_string())
        }
    }
}

async fn fallback_hotsearch() -> (Vec<TimelineItem>, String, Option<String>, String) {
    match fetch_hotsearch().await {
        Ok(items) => {
            let title = format!("🔥 热搜榜 ({}条)", items.len());
            (items, title, None, String::new())
        }
        Err(e) => {
            log_error!("热搜榜加载失败: {}", e);
            (vec![], "加载失败".into(), None, String::new())
        }
    }
}
