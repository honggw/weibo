//! Chat service — contacts list, user info, unread count.
//!
//! APIs (from api.weibo.com_chat.har analysis):
//!   - /webim/2/direct_messages/contacts.json → conversation list (Base64 encoded)
//!   - /webim/query_primary_info.json → current user info
//!   - /webim/query_remark.json → friend nickname mappings
//!   - rm.api.weibo.com/remind/push_count.json → unread counts

use anyhow::Result;
use std::collections::HashMap;

use crate::domain::Contact;
use crate::infra::config;
use crate::infra::cookie_io;
use crate::infra::http_client;
use crate::{log_info, log_success};

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
) -> Vec<crate::domain::ChatMessage> {
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
                            let text = m
                                .get("content")
                                .and_then(|v| v.as_str())
                                .unwrap_or("")
                                .to_string();
                            let ts = m.get("time").and_then(|v| v.as_u64()).unwrap_or(0);
                            // Convert Unix timestamp to readable string
                            use std::time::{Duration, UNIX_EPOCH};
                            let time_str = UNIX_EPOCH
                                .checked_add(Duration::from_secs(ts))
                                .map(|t| format!("{:?}", t))
                                .unwrap_or_default();
                            crate::domain::ChatMessage {
                                id: m
                                    .get("id")
                                    .and_then(|v| v.as_u64())
                                    .map(|v| v.to_string())
                                    .unwrap_or_default(),
                                sender_id: sid.clone(),
                                sender_name: name,
                                text,
                                created_at: time_str,
                                is_self: sid == my_uid,
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
) -> Vec<crate::domain::ChatMessage> {
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
                            crate::domain::ChatMessage {
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
                                text: m
                                    .get("text")
                                    .and_then(|v| v.as_str())
                                    .unwrap_or("")
                                    .to_string(),
                                created_at: m
                                    .get("created_at")
                                    .and_then(|v| v.as_str())
                                    .unwrap_or("")
                                    .to_string(),
                                is_self: sid == my_uid,
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
pub async fn send_message(uid: &str, text: &str, is_group: bool) -> Option<crate::domain::ChatMessage> {
    if is_group {
        return send_group_message(uid, text).await;
    }
    send_dm_message(uid, text).await
}

async fn send_dm_message(uid: &str, text: &str) -> Option<crate::domain::ChatMessage> {
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
        &text[..text.len().min(20)]
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
                return Some(crate::domain::ChatMessage {
                    id,
                    text,
                    created_at,
                    sender_id: my_uid,
                    sender_name: "我".to_string(),
                    is_self: true,
                });
            }
        }
        Err(e) => log_info!("[chat] 发送失败: {}", e),
    }
    None
}

/// Fetch DM unread count.
/// Send group message via /webim/groupchat/send_message.json
async fn send_group_message(gid: &str, text: &str) -> Option<crate::domain::ChatMessage> {
    let (cookie, xsrf) = chat_headers();
    let url = format!("{}/webim/groupchat/send_message.json", CHAT_BASE);
    let client = http_client::build_no_store();
    let encoded_text = url::form_urlencoded::byte_serialize(text.as_bytes()).collect::<String>();
    let params = format!("content={}&id={}&source={}", encoded_text, gid, SOURCE);

    log_info!("[chat] Group send: gid={}, text={}...", gid, &text[..text.len().min(20)]);
    match client.post(&url).header("Cookie", &cookie).header("Referer", format!("{}/chat", CHAT_BASE))
        .header("User-Agent", config::DEFAULT_UA).header("Content-Type", "application/x-www-form-urlencoded")
        .header("X-XSRF-TOKEN", &xsrf).body(params).timeout(config::REQUEST_TIMEOUT).send().await
    {
        Ok(resp) => {
            if let Ok(data) = resp.json::<serde_json::Value>().await {
                let id = data.get("id").and_then(|v| v.as_u64()).map(|v| v.to_string()).unwrap_or_default();
                log_info!("[chat] Group 已发送: id={}", id);
                return Some(crate::domain::ChatMessage {
                    id, text: text.to_string(), created_at: String::new(),
                    sender_id: String::new(), sender_name: "我".to_string(), is_self: true,
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
) -> tokio::sync::mpsc::UnboundedReceiver<crate::infra::ws_client::WsMessage> {
    let cookie = cookie_io::load_full();
    let h1 = tokio_handle.clone();
    let h2 = tokio_handle.clone();
    let (tx, rx) = tokio::sync::mpsc::unbounded_channel();

    let uid_log = uid.clone();
    h1.spawn(async move {
        h2.spawn(async move {
            if let Err(e) = crate::infra::ws_client::connect_and_listen(&uid, &cookie, tx).await {
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
    // If not base64, return as-is
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
    // Group detection: super_group_type>0 OR group_type==3 OR type==2 (e.g. 图片收藏群)
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
        // Strip HTML tags for preview
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
