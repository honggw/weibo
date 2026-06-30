//! Chat service — contacts list, user info, unread count.
//!
//! APIs (from api.weibo.com_chat.har analysis):
//!   - /webim/2/direct_messages/contacts.json → conversation list (Base64 encoded)
//!   - /webim/query_primary_info.json → current user info
//!   - /webim/query_remark.json → friend nickname mappings
//!   - rm.api.weibo.com/remind/push_count.json → unread counts

use anyhow::Result;
use std::collections::HashMap;

use weibo_domain::Contact;
use weibo_infra::config;
use weibo_infra::cookie_io;
use weibo_infra::http_client;
use weibo_infra::{log_info, log_success};

const SOURCE: &str = "209678993";
const CHAT_BASE: &str = "https://api.weibo.com";

/// Build cookie + XSRF headers for chat API.
fn chat_headers() -> (String, String) {
    let cookie = cookie_io::load_full();
    let xsrf = cookie_io::load_xsrf().unwrap_or_default();
    (cookie, xsrf)
}

/// Fetch the conversation/contact list.
pub async fn fetch_contacts() -> Result<Vec<Contact>> {
    let (cookie, _xsrf) = chat_headers();
    let url = format!(
        "{}/webim/2/direct_messages/contacts.json?special_source=3&add_virtual_user=3,4\
         &is_include_group=0&need_back=0,0&is_include_folder=1&count=50&source={}",
        CHAT_BASE, SOURCE
    );

    log_info!("[chat] 获取会话列表...");
    let client = http_client::build_no_store();
    let resp = client
        .get(&url)
        .header("Cookie", &cookie)
        .header("Referer", format!("{}/chat", CHAT_BASE))
        .header("User-Agent", config::DEFAULT_UA)
        .header("Accept", "application/json, text/plain, */*")
        .timeout(config::REQUEST_TIMEOUT)
        .send()
        .await?;

    let body = resp.text().await?;
    // Contacts API response is Base64 encoded
    let decoded = decode_response(&body);
    let data: serde_json::Value = serde_json::from_str(&decoded)?;

    let contacts: Vec<Contact> = data
        .get("contacts")
        .and_then(|c| c.as_array())
        .map(|arr| arr.iter().filter_map(|c| parse_contact(c)).collect())
        .unwrap_or_default();

    log_success!(
        "[chat] 加载 {} 个会话 (总计 {})",
        contacts.len(),
        data.get("totalNumber")
            .and_then(|v| v.as_u64())
            .unwrap_or(0)
    );
    Ok(contacts)
}

/// Fetch friend nickname/remark mappings.
pub async fn fetch_remarks() -> HashMap<String, String> {
    let (cookie, _xsrf) = chat_headers();
    let url = format!("{}/webim/query_remark.json?source={}", CHAT_BASE, SOURCE);

    let client = http_client::build_no_store();
    match client
        .get(&url)
        .header("Cookie", &cookie)
        .header("Referer", format!("{}/chat", CHAT_BASE))
        .header("User-Agent", config::DEFAULT_UA)
        .timeout(config::REQUEST_TIMEOUT)
        .send()
        .await
    {
        Ok(resp) => {
            if let Ok(data) = resp.json::<serde_json::Value>().await {
                let map: HashMap<String, String> = data
                    .get("remarks")
                    .and_then(|r| r.as_array())
                    .map(|arr| {
                        arr.iter()
                            .filter_map(|r| {
                                let uid = r.get("uid").and_then(|v| v.as_u64())?.to_string();
                                let remark =
                                    r.get("remark").and_then(|v| v.as_str()).unwrap_or("?");
                                Some((uid, remark.to_string()))
                            })
                            .collect()
                    })
                    .unwrap_or_default();
                return map;
            }
        }
        Err(e) => log_info!("[chat] 获取备注失败: {}", e),
    }
    HashMap::new()
}

/// Fetch current user info.
pub async fn fetch_primary_info() -> Option<(String, String)> {
    let (cookie, _xsrf) = chat_headers();
    let url = format!(
        "{}/webim/query_primary_info.json?source={}",
        CHAT_BASE, SOURCE
    );

    let client = http_client::build_no_store();
    match client
        .get(&url)
        .header("Cookie", &cookie)
        .header("Referer", format!("{}/chat", CHAT_BASE))
        .header("User-Agent", config::DEFAULT_UA)
        .timeout(config::REQUEST_TIMEOUT)
        .send()
        .await
    {
        Ok(resp) => {
            if let Ok(data) = resp.json::<serde_json::Value>().await {
                let profile = data.get("profile")?;
                let uid = profile.get("id").and_then(|v| v.as_u64())?.to_string();
                let name = profile
                    .get("screen_name")
                    .and_then(|v| v.as_str())
                    .unwrap_or("?")
                    .to_string();
                return Some((uid, name));
            }
        }
        Err(e) => log_info!("[chat] 获取用户信息失败: {}", e),
    }
    None
}

/// Fetch message history for a group chat.
/// `max_mid`: pass Some(oldest_id) to load earlier messages, None for latest.
pub async fn fetch_group_messages(
    gid: &str,
    my_uid: &str,
    max_mid: Option<&str>,
) -> Vec<weibo_domain::ChatMessage> {
    let (cookie, _xsrf) = chat_headers();
    let mid = max_mid.unwrap_or("0");
    let url = format!(
        "{}/webim/groupchat/query_messages.json?convert_emoji=1&query_sender=1&count=30&id={}&max_mid={}&source={}",
        CHAT_BASE, gid, mid, SOURCE
    );

    let client = http_client::build_no_store();
    match client
        .get(&url)
        .header("Cookie", &cookie)
        .header("Referer", format!("{}/chat", CHAT_BASE))
        .header("User-Agent", config::DEFAULT_UA)
        .timeout(config::REQUEST_TIMEOUT)
        .send()
        .await
    {
        Ok(resp) => {
            if let Ok(data) = resp.json::<serde_json::Value>().await {
                if let Some(arr) = data.get("messages").and_then(|m| m.as_array()) {
                    log_info!("[chat] group fetch: gid={}, {} messages", gid, arr.len());
                    return arr
                        .iter()
                        .map(|m| {
                            let sid = m
                                .get("from_uid")
                                .and_then(|v| v.as_u64())
                                .unwrap_or(0)
                                .to_string();
                            let name = m
                                .get("from_user")
                                .and_then(|u| u.get("screen_name"))
                                .and_then(|v| v.as_str())
                                .unwrap_or("?")
                                .to_string();
                            let avatar = m
                                .get("from_user")
                                .and_then(|u| u.get("profile_image_url"))
                                .and_then(|v| v.as_str())
                                .unwrap_or("")
                                .to_string();
                            let text = m
                                .get("content")
                                .and_then(|v| v.as_str())
                                .unwrap_or("")
                                .to_string();
                            let ts = m.get("time").and_then(|v| v.as_u64()).unwrap_or(0);
                            let time_str = format_timestamp(ts);
                            let type_val = m.get("type").and_then(|v| v.as_u64()).unwrap_or(321);
                            let media_val = m.get("media_type").and_then(|v| v.as_u64()).unwrap_or(0);
                            let role = m.get("from_user_role")
                                .and_then(|v| v.as_u64())
                                .unwrap_or(0) as u8;
                            let fids: Vec<String> = m
                                .get("fids")
                                .and_then(|v| v.as_array())
                                .map(|arr| {
                                    arr.iter()
                                        .filter_map(|v| v.as_u64().map(|n| n.to_string()))
                                        .collect()
                                })
                                .unwrap_or_default();

                            weibo_domain::ChatMessage {
                                id: m
                                    .get("id")
                                    .and_then(|v| v.as_u64())
                                    .map(|v| v.to_string())
                                    .unwrap_or_default(),
                                sender_id: sid.clone(),
                                sender_name: name,
                                sender_avatar: avatar,
                                text,
                                created_at: time_str,
                                timestamp: ts,
                                is_self: sid == my_uid,
                                msg_type: weibo_domain::MsgType::from_api(type_val),
                                media_type: weibo_domain::MediaType::from_api(media_val),
                                fids,
                                role,
                            }
                        })
                        .collect();
                }
            }
        }
        Err(e) => log_info!("[chat] group fetch failed: {}", e),
    }
    Vec::new()
}

/// Fetch message history for a conversation (DM or group).
/// `max_id`: oldest message ID from previous page, None for first page.
pub async fn fetch_messages(
    uid: &str,
    my_uid: &str,
    is_group: bool,
    max_id: Option<&str>,
) -> Vec<weibo_domain::ChatMessage> {
    if is_group {
        return fetch_group_messages(uid, my_uid, max_id).await;
    }
    let (cookie, _xsrf) = chat_headers();
    let url = format!(
        "{}/webim/2/direct_messages/conversation.json?uid={}&source={}&count=30",
        CHAT_BASE, uid, SOURCE
    );

    let client = http_client::build_no_store();
    match client
        .get(&url)
        .header("Cookie", &cookie)
        .header("Referer", format!("{}/chat", CHAT_BASE))
        .header("User-Agent", config::DEFAULT_UA)
        .timeout(config::REQUEST_TIMEOUT)
        .send()
        .await
    {
        Ok(resp) => {
            if let Ok(data) = resp.json::<serde_json::Value>().await {
                if let Some(arr) = data.get("direct_messages").and_then(|m| m.as_array()) {
                    log_info!("[chat] fetch_messages: uid={}, {} messages", uid, arr.len());
                    return arr
                        .iter()
                        .rev()
                        .map(|m| {
                            let sid = m
                                .get("sender_id")
                                .and_then(|v| v.as_u64())
                                .unwrap_or(0)
                                .to_string();
                            let media_val = m.get("media_type").and_then(|v| v.as_u64()).unwrap_or(0);
                            let type_val = m
                                .get("group_chat_message_type")
                                .and_then(|v| v.as_u64())
                                .unwrap_or(321);
                            let fids: Vec<String> = m.get("fids")
                                .and_then(|v| v.as_array())
                                .map(|arr| arr.iter()
                                    .filter_map(|v| v.as_u64().map(|n| n.to_string()))
                                    .collect())
                                .unwrap_or_default();
                            let role = m.get("sender_role")
                                .and_then(|v| v.as_u64())
                                .unwrap_or(0) as u8;
                            let created_at_str = m
                                .get("created_at")
                                .and_then(|v| v.as_str())
                                .unwrap_or("");
                            let timestamp = parse_dm_timestamp(created_at_str);

                            weibo_domain::ChatMessage {
                                id: m
                                    .get("idstr")
                                    .and_then(|v| v.as_str())
                                    .unwrap_or("")
                                    .to_string(),
                                sender_id: sid.clone(),
                                sender_name: m
                                    .get("sender_screen_name")
                                    .and_then(|v| v.as_str())
                                    .unwrap_or("?")
                                    .to_string(),
                                sender_avatar: String::new(),
                                text: m
                                    .get("text")
                                    .and_then(|v| v.as_str())
                                    .unwrap_or("")
                                    .to_string(),
                                created_at: created_at_str.to_string(),
                                timestamp,
                                is_self: sid == my_uid,
                                msg_type: weibo_domain::MsgType::from_api(type_val),
                                media_type: weibo_domain::MediaType::from_api(media_val),
                                fids,
                                role,
                            }
                        })
                        .collect();
                }
            }
        }
        Err(e) => log_info!("[chat] 获取消息失败: {}", e),
    }
    log_info!("[chat] fetch_messages: returning 0 messages");
    Vec::new()
}

/// Send a text message. Routes to DM or group endpoint based on `is_group`.
pub async fn send_message(uid: &str, text: &str, is_group: bool) -> Option<weibo_domain::ChatMessage> {
    if is_group {
        return send_group_message(uid, text).await;
    }
    send_dm_message(uid, text).await
}

async fn send_dm_message(uid: &str, text: &str) -> Option<weibo_domain::ChatMessage> {
    let (cookie, xsrf) = chat_headers();
    let my_uid = fetch_primary_info()
        .await
        .map(|(id, _)| id)
        .unwrap_or_default();
    let url = format!("{}/webim/2/direct_messages/new.json", CHAT_BASE);

    let client = http_client::build_no_store();
    let encoded_text = url::form_urlencoded::byte_serialize(text.as_bytes()).collect::<String>();
    let params = format!(
        "text={}&uid={}&source={}&t={}",
        encoded_text,
        uid,
        SOURCE,
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis()
    );

    log_info!(
        "[chat] 发送消息: uid={}, text={}...",
        uid,
        text.chars().take(20).collect::<String>()
    );
    match client
        .post(&url)
        .header("Cookie", &cookie)
        .header("Referer", format!("{}/chat", CHAT_BASE))
        .header("User-Agent", config::DEFAULT_UA)
        .header("Content-Type", "application/x-www-form-urlencoded")
        .header("X-XSRF-TOKEN", &xsrf)
        .body(params)
        .timeout(config::REQUEST_TIMEOUT)
        .send()
        .await
    {
        Ok(resp) => {
            if let Ok(data) = resp.json::<serde_json::Value>().await {
                let id = data
                    .get("idstr")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                let text = data
                    .get("text")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                let created_at = data
                    .get("created_at")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                log_info!("[chat] 消息已发送: id={}", id);
                return Some(weibo_domain::ChatMessage {
                    id,
                    text,
                    created_at,
                    sender_id: my_uid,
                    sender_name: "我".to_string(),
                    sender_avatar: String::new(),
                    timestamp: std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH).unwrap().as_secs(),
                    is_self: true,
                    msg_type: weibo_domain::MsgType::Normal,
                    media_type: weibo_domain::MediaType::Text,
                    fids: vec![],
                    role: 0,
                });
            }
        }
        Err(e) => log_info!("[chat] 发送失败: {}", e),
    }
    None
}

/// Send group message via /webim/groupchat/send_message.json
async fn send_group_message(gid: &str, text: &str) -> Option<weibo_domain::ChatMessage> {
    let (cookie, xsrf) = chat_headers();
    let url = format!("{}/webim/groupchat/send_message.json", CHAT_BASE);
    let client = http_client::build_no_store();
    let encoded_text = url::form_urlencoded::byte_serialize(text.as_bytes()).collect::<String>();
    let params = format!("content={}&id={}&source={}", encoded_text, gid, SOURCE);

    log_info!("[chat] Group send: gid={}, text={}...", gid, text.chars().take(20).collect::<String>());
    match client.post(&url).header("Cookie", &cookie).header("Referer", format!("{}/chat", CHAT_BASE))
        .header("User-Agent", config::DEFAULT_UA).header("Content-Type", "application/x-www-form-urlencoded")
        .header("X-XSRF-TOKEN", &xsrf).body(params).timeout(config::REQUEST_TIMEOUT).send().await
    {
        Ok(resp) => {
            if let Ok(data) = resp.json::<serde_json::Value>().await {
                let id = data.get("id").and_then(|v| v.as_u64()).map(|v| v.to_string()).unwrap_or_default();
                log_info!("[chat] Group 已发送: id={}", id);
                return Some(weibo_domain::ChatMessage {
                    id, text: text.to_string(), created_at: String::new(),
                    sender_id: String::new(), sender_name: "我".to_string(),
                    sender_avatar: String::new(),
                    timestamp: std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH).unwrap().as_secs(),
                    is_self: true,
                    msg_type: weibo_domain::MsgType::Normal,
                    media_type: weibo_domain::MediaType::Text,
                    fids: vec![],
                    role: 0,
                });
            }
        }
        Err(e) => log_info!("[chat] Group 发送失败: {}", e),
    }
    None
}

/// Start WebSocket connection for real-time message push.
/// Returns a receiver for incoming messages (to be polled by ViewModel).
pub fn start_ws(
    uid: String,
    tokio_handle: &tokio::runtime::Handle,
) -> tokio::sync::mpsc::UnboundedReceiver<weibo_infra::ws_client::WsMessage> {
    let cookie = cookie_io::load_full();
    let h1 = tokio_handle.clone();
    let h2 = tokio_handle.clone();
    let (tx, rx) = tokio::sync::mpsc::unbounded_channel();

    let uid_log = uid.clone();
    h1.spawn(async move {
        h2.spawn(async move {
            if let Err(e) = weibo_infra::ws_client::connect_and_listen(&uid, &cookie, tx).await {
                log_info!("[ws] 连接失败: {}", e);
            }
        });
        log_info!("[ws] WebSocket 已启动, uid={}", uid_log);
    });

    rx
}

pub async fn fetch_dm_unread() -> u64 {
    let cookie = cookie_io::load_full();
    let url = "https://rm.api.weibo.com/2/remind/push_count.json?trim_null=1&with_dm_group=1&with_chat_group=1&with_dm_unread=1&source=339644097";

    let client = http_client::build_no_store();
    match client
        .get(url)
        .header("Cookie", &cookie)
        .header("Referer", config::WEIBO_BASE_URL)
        .header("User-Agent", config::DEFAULT_UA)
        .timeout(config::REQUEST_TIMEOUT)
        .send()
        .await
    {
        Ok(resp) => {
            if let Ok(data) = resp.json::<serde_json::Value>().await {
                let dm = data.get("dm").and_then(|v| v.as_u64()).unwrap_or(0);
                return dm;
            }
        }
        Err(e) => log_info!("[chat] 获取未读数失败: {}", e),
    }
    0
}

/// Decode Base64 response from contacts API.
fn decode_response(body: &str) -> String {
    use base64::Engine;
    if let Ok(decoded) = base64::engine::general_purpose::STANDARD.decode(body) {
        if let Ok(s) = String::from_utf8(decoded) {
            return s;
        }
    }
    body.to_string()
}

/// Parse a single contact from JSON.
fn parse_contact(c: &serde_json::Value) -> Option<Contact> {
    let user = c.get("user")?;
    let user_id = user
        .get("idstr")
        .or_else(|| user.get("id"))
        .and_then(|v| v.as_str())
        .unwrap_or("0")
        .to_string();
    let screen_name = user
        .get("screen_name")
        .or_else(|| user.get("name"))
        .and_then(|v| v.as_str())
        .unwrap_or("?")
        .to_string();
    let avatar = user
        .get("profile_image_url")
        .or_else(|| user.get("avatar"))
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let unread = c.get("unread_count").and_then(|v| v.as_u64()).unwrap_or(0);
    let is_group = user
        .get("super_group_type")
        .and_then(|v| v.as_u64())
        .unwrap_or(0)
        > 0
        || user.get("group_type").and_then(|v| v.as_u64()).unwrap_or(0) == 3
        || user.get("type").and_then(|v| v.as_u64()).unwrap_or(0) == 2;

    let msg = c.get("message")?;
    let text = msg
        .get("text")
        .or_else(|| msg.get("template"))
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let time = msg
        .get("created_at")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let last_message = if text.is_empty() {
        "[图片]".to_string()
    } else {
        text.replace(|c| c == '<' || c == '>', "")
            .chars()
            .take(50)
            .collect()
    };

    Some(Contact {
        user_id,
        screen_name,
        avatar,
        unread_count: unread,
        last_message,
        last_time: time,
        is_group,
    })
}

/// 将 Unix 时间戳格式化为可读时间字符串 (东八区)。
fn format_timestamp(ts: u64) -> String {
    if ts == 0 {
        return String::new();
    }
    let tz_offset: i64 = 8 * 3600;
    let secs_in_day: i64 = 86400;
    let local_ts = ts as i64 + tz_offset;

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64
        + tz_offset;

    let msg_day = local_ts / secs_in_day;
    let now_day = now / secs_in_day;
    let hour = ((local_ts % secs_in_day) / 3600) as u32;
    let minute = ((local_ts % 3600) / 60) as u32;

    if msg_day == now_day {
        format!("{:02}:{:02}", hour, minute)
    } else if msg_day == now_day - 1 {
        format!("昨天 {:02}:{:02}", hour, minute)
    } else {
        let (month, day) = month_day_from_day_index(msg_day);
        format!("{:02}-{:02} {:02}:{:02}", month, day, hour, minute)
    }
}

fn month_day_from_day_index(day_index: i64) -> (u32, u32) {
    let mut remaining = day_index;
    let mut year: i64 = 1970;
    loop {
        let days_in_year = if is_leap(year) { 366 } else { 365 };
        if remaining < days_in_year {
            break;
        }
        remaining -= days_in_year;
        year += 1;
    }
    let months_days: [i64; 12] = if is_leap(year) {
        [31, 29, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
    } else {
        [31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
    };
    let mut month: u32 = 1;
    for md in months_days.iter() {
        if remaining < *md {
            break;
        }
        remaining -= *md;
        month += 1;
    }
    let day = (remaining + 1) as u32;
    (month, day)
}

fn is_leap(year: i64) -> bool {
    (year % 4 == 0 && year % 100 != 0) || (year % 400 == 0)
}

/// 解析 DM 接口的 created_at 字符串为 Unix 时间戳。
pub fn parse_dm_timestamp(created_at: &str) -> u64 {
    let parts: Vec<&str> = created_at.split_whitespace().collect();
    if parts.len() < 6 {
        return 0;
    }
    let month: i64 = match parts.get(1).copied().unwrap_or("") {
        "Jan" => 1, "Feb" => 2, "Mar" => 3, "Apr" => 4,
        "May" => 5, "Jun" => 6, "Jul" => 7, "Aug" => 8,
        "Sep" => 9, "Oct" => 10, "Nov" => 11, "Dec" => 12,
        _ => return 0,
    };
    let day: i64 = parts.get(2).and_then(|s| s.parse().ok()).unwrap_or(0);
    if day == 0 { return 0; }

    let time: Vec<&str> = parts.get(3).unwrap_or(&"").split(':').collect();
    if time.len() < 3 { return 0; }
    let hour: i64 = time[0].parse().unwrap_or(0);
    let minute: i64 = time[1].parse().unwrap_or(0);
    let second: i64 = time[2].parse().unwrap_or(0);

    let year: i64 = parts.get(5).and_then(|s| s.parse().ok()).unwrap_or(0);
    if year < 2000 { return 0; }

    let mut days: i64 = 0;
    for y in 1970..year {
        days += if is_leap(y) { 366 } else { 365 };
    }
    let months_days: [i64; 12] = if is_leap(year) {
        [31, 29, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
    } else {
        [31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
    };
    for m in 0..(month - 1) as usize {
        days += months_days[m];
    }
    days += day - 1;

    let tz_offset: i64 = 8 * 3600;
    let timestamp = (days * 86400) + (hour * 3600) + (minute * 60) + second - tz_offset;
    if timestamp < 0 { 0 } else { timestamp as u64 }
}

/// 获取微博表情列表
pub async fn fetch_emotions() -> Vec<weibo_domain::Emotion> {
    let (cookie, _xsrf) = chat_headers();
    let url = format!("{}/webim/emotions.json?source={}", CHAT_BASE, SOURCE);

    let client = http_client::build_no_store();
    match client
        .get(&url)
        .header("Cookie", &cookie)
        .header("Referer", format!("{}/chat", CHAT_BASE))
        .header("User-Agent", config::DEFAULT_UA)
        .timeout(config::REQUEST_TIMEOUT)
        .send()
        .await
    {
        Ok(resp) => {
            if let Ok(arr) = resp.json::<Vec<serde_json::Value>>().await {
                return arr
                    .iter()
                    .filter_map(|e| {
                        let phrase = e.get("phrase")?.as_str()?.to_string();
                        let url = e.get("url")?.as_str()?.to_string();
                        Some(weibo_domain::Emotion { phrase, url })
                    })
                    .collect();
            }
        }
        Err(e) => log_info!("[chat] 获取表情失败: {}", e),
    }
    Vec::new()
}

/// 上报已读状态
pub async fn report_read(uid: &str) {
    let (cookie, xsrf) = chat_headers();
    let url = format!("{}/webim/report.json", CHAT_BASE);
    let client = http_client::build_no_store();

    let data_json = serde_json::json!({
        "type": 2,
        "uid": uid,
    });
    let params = format!(
        "data={}&source={}",
        url::form_urlencoded::byte_serialize(data_json.to_string().as_bytes())
            .collect::<String>(),
        SOURCE
    );

    match client
        .post(&url)
        .header("Cookie", &cookie)
        .header("Referer", format!("{}/chat", CHAT_BASE))
        .header("User-Agent", config::DEFAULT_UA)
        .header("Content-Type", "application/x-www-form-urlencoded")
        .header("X-XSRF-TOKEN", &xsrf)
        .body(params)
        .timeout(config::REQUEST_TIMEOUT)
        .send()
        .await
    {
        Ok(_) => log_info!("[chat] 已读上报: uid={}", uid),
        Err(e) => log_info!("[chat] 已读上报失败: {}", e),
    }
}

/// 获取群详情
pub async fn fetch_group_info(gid: &str) -> Option<weibo_domain::GroupInfo> {
    let (cookie, _xsrf) = chat_headers();
    let url = format!(
        "{}/webim/query_group.json?is_pc=1&query_member=1&sort_by_jp=1&query_member_count=5000&id={}&source={}",
        CHAT_BASE, gid, SOURCE
    );

    let client = http_client::build_no_store();
    match client
        .get(&url)
        .header("Cookie", &cookie)
        .header("Referer", format!("{}/chat", CHAT_BASE))
        .header("User-Agent", config::DEFAULT_UA)
        .timeout(config::REQUEST_TIMEOUT)
        .send()
        .await
    {
        Ok(resp) => {
            if let Ok(data) = resp.json::<serde_json::Value>().await {
                let id = data
                    .get("id")
                    .and_then(|v| v.as_u64())
                    .map(|v| v.to_string())
                    .unwrap_or_default();
                let name = data
                    .get("name")
                    .and_then(|v| v.as_str())
                    .unwrap_or("?")
                    .to_string();
                let owner_uid = data
                    .get("owner_uid")
                    .and_then(|v| v.as_u64())
                    .map(|v| v.to_string())
                    .unwrap_or_default();
                let member_count = data
                    .get("member_count")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(0);
                let members: Vec<weibo_domain::GroupMember> = data
                    .get("member_infos")
                    .or_else(|| data.get("members"))
                    .and_then(|m| m.as_array())
                    .map(|arr| {
                        arr.iter()
                            .filter_map(|m| {
                                if let Some(uid_num) = m.as_u64() {
                                    let uid = uid_num.to_string();
                                    return Some(weibo_domain::GroupMember {
                                        uid,
                                        screen_name: String::new(),
                                        avatar: String::new(),
                                        is_admin: false,
                                    });
                                }
                                let uid = m
                                    .get("uid")
                                    .and_then(|v| v.as_u64())
                                    .map(|v| v.to_string())
                                    .unwrap_or_default();
                                if uid.is_empty() {
                                    return None;
                                }
                                let screen_name = m
                                    .get("screen_name")
                                    .and_then(|v| v.as_str())
                                    .unwrap_or("?")
                                    .to_string();
                                let avatar = m
                                    .get("profile_image_url")
                                    .and_then(|v| v.as_str())
                                    .unwrap_or("")
                                    .to_string();
                                let is_admin = m
                                    .get("is_admin")
                                    .and_then(|v| v.as_bool())
                                    .unwrap_or(false);
                                Some(weibo_domain::GroupMember {
                                    uid,
                                    screen_name,
                                    avatar,
                                    is_admin,
                                })
                            })
                            .collect()
                    })
                    .unwrap_or_default();

                log_info!(
                    "[chat] 获取群信息: name={}, members={}",
                    name,
                    members.len()
                );
                return Some(weibo_domain::GroupInfo {
                    id,
                    name,
                    owner_uid,
                    member_count,
                    members,
                });
            }
        }
        Err(e) => log_info!("[chat] 获取群信息失败: {}", e),
    }
    None
}

// ---------------------------------------------------------------------------
// Image helpers
// ---------------------------------------------------------------------------

/// 根据 fid 拼接微博图片缩略图 URL.
pub fn thumb_url(fid: &str, gid: Option<&str>) -> String {
    let base = "https://upload.api.weibo.com/2/mss/msget_thumbnail";
    if let Some(g) = gid {
        format!(
            "{base}?fid={fid}&high=240&width=240&gid={g}&size=240,240&source={SOURCE}"
        )
    } else {
        format!(
            "{base}?fid={fid}&high=240&width=240&size=240,240&source={SOURCE}"
        )
    }
}

/// 下载图片 bytes
pub async fn download_image(url: &str) -> Option<Vec<u8>> {
    let (cookie, _xsrf) = chat_headers();
    let client = http_client::build_no_store();
    match client
        .get(url)
        .header("Cookie", &cookie)
        .header("Referer", format!("{}/chat", CHAT_BASE))
        .header("User-Agent", config::DEFAULT_UA)
        .timeout(config::REQUEST_TIMEOUT)
        .send()
        .await
    {
        Ok(resp) => {
            if resp.status().is_success() {
                resp.bytes().await.ok().map(|b| b.to_vec())
            } else {
                log_info!("[chat] 下载图片失败: status={}", resp.status());
                None
            }
        }
        Err(e) => {
            log_info!("[chat] 下载图片失败: {}", e);
            None
        }
    }
}
