//! Timeline service — fetch following list, aggregate posts, fallback to hotsearch.

use anyhow::Result;

use crate::domain::TimelineItem;
use crate::infra::config;
use crate::infra::http_client;
use crate::{log_error, log_info, log_success};

/// Fetch the list of followed user UIDs.
pub async fn fetch_following_ids(cookie: &str) -> Result<Vec<u64>> {
    let data = http_client::auth_get(
        &format!("{}?page=1", config::API_FRIENDSHIPS),
        cookie,
    )
    .await?;

    let uids: Vec<u64> = data
        .get("users")
        .and_then(|u| u.as_array())
        .map(|arr| {
            arr.iter()
                .take(config::MAX_FOLLOWED_USERS)
                .filter_map(|u| u.get("id").and_then(|v| v.as_u64()))
                .collect()
        })
        .unwrap_or_default();

    Ok(uids)
}

/// Fetch recent posts from followed users and combine into a timeline.
pub async fn fetch_from_friends(cookie: &str, uids: &[u64]) -> Vec<TimelineItem> {
    let mut all_items: Vec<TimelineItem> = Vec::new();

    for &uid in uids.iter().take(config::MAX_FOLLOWED_USERS) {
        match http_client::auth_get(
            &format!(
                "{}?uid={}&page=1&feature=0",
                config::API_MYMBLOG,
                uid
            ),
            cookie,
        )
        .await
        {
            Ok(data) => {
                if let Some(list) = data
                    .get("data")
                    .and_then(|d| d.get("list"))
                    .and_then(|l| l.as_array())
                {
                    for s in list.iter().take(config::MAX_POSTS_PER_USER) {
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
                        if !text.is_empty() {
                            all_items.push(TimelineItem { user_name, text });
                        }
                    }
                }
            }
            Err(e) => log_info!("拉取用户 {} 微博失败: {}", uid, e),
        }
    }

    all_items
}

/// Fetch hotsearch trends (public API, no auth needed).
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
                    let word = item
                        .get("word")
                        .and_then(|v| v.as_str())
                        .unwrap_or("?")
                        .to_string();
                    let num = item.get("num").and_then(|v| v.as_i64()).unwrap_or(0);
                    let note = item
                        .get("note")
                        .or_else(|| item.get("category"))
                        .and_then(|v| v.as_str())
                        .unwrap_or("");

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

/// Main entry: fetch home content (timeline preferred, hotsearch fallback).
/// Returns (items, title).
pub async fn fetch_home_content(cookie: &str) -> (Vec<TimelineItem>, String) {
    // Try following-based timeline first
    log_info!("获取关注列表...");
    match fetch_following_ids(cookie).await {
        Ok(uids) if !uids.is_empty() => {
            log_info!("获取到 {} 个关注用户, 拉取微博...", uids.len());
            let items = fetch_from_friends(cookie, &uids).await;
            if !items.is_empty() {
                let count = items.len();
                let title = format!(
                    "📰 首页时间线 ({}位关注者, {}条)",
                    uids.len().min(config::MAX_FOLLOWED_USERS),
                    count
                );
                log_success!("首页时间线: {} 条", count);
                return (items, title);
            }
        }
        Ok(_) => log_info!("无关注用户，回退到热搜榜"),
        Err(e) => log_info!("获取关注列表失败: {}, 回退到热搜榜", e),
    }

    // Fallback to hotsearch
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
