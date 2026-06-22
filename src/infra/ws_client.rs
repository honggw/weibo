//! WebSocket client with Bayeux/CometD protocol for Weibo IM real-time push.
//!
//! Protocol (from api.weibo.com_chat.har):
//!   1. Connect to wss://web.im.weibo.com/im
//!   2. Handshake → get clientId
//!   3. Subscribe to /im/{uid}
//!   4. Long-poll connect → receive messages

use anyhow::Result;
use futures_util::{SinkExt, StreamExt};
use tokio::sync::mpsc;
use tokio_tungstenite::connect_async;
use tokio_tungstenite::tungstenite::Message as WsMsg;

use crate::log_info;

/// An incoming push message from the WebSocket.
#[derive(Debug, Clone)]
pub struct WsMessage {
    pub channel: String,
    pub data: serde_json::Value,
}

/// Connect to Weibo IM WebSocket and process messages via callback.
pub async fn connect_and_listen(
    uid: &str,
    cookie: &str,
    tx: mpsc::UnboundedSender<WsMessage>,
) -> Result<()> {
    log_info!("[ws] 连接 WebSocket: wss://web.im.weibo.com/im");

    // Use string URL (auto-generates WebSocket key), add cookie via custom request
    let req = {
        use tokio_tungstenite::tungstenite::client::IntoClientRequest;
        let mut r = "wss://web.im.weibo.com/im".into_client_request()?;
        r.headers_mut().insert("Cookie", cookie.parse().unwrap());
        r.headers_mut().insert("Origin", "https://api.weibo.com".parse().unwrap());
        r
    };

    let (ws_stream, _) = connect_async(req).await?;
    let (mut write, mut read) = ws_stream.split();
    let mut msg_id: u64 = 1;
    let mut client_id = String::new();

    // Step 1: Handshake
    let handshake = serde_json::json!([{
        "id": msg_id.to_string(),
        "version": "1.0",
        "minimumVersion": "1.0",
        "channel": "/meta/handshake",
        "supportedConnectionTypes": ["websocket", "long-polling", "callback-polling"],
    }]);
    write.send(WsMsg::Text(handshake.to_string())).await?;
    msg_id += 1;

    if let Some(Ok(WsMsg::Text(text))) = read.next().await {
        if let Ok(arr) = serde_json::from_str::<Vec<serde_json::Value>>(&text) {
            if let Some(obj) = arr.first() {
                client_id = obj.get("clientId").and_then(|v| v.as_str()).unwrap_or("").to_string();
                log_info!("[ws] handshake OK, clientId={}", &client_id[..client_id.len().min(20)]);
            }
        }
    }

    if client_id.is_empty() {
        anyhow::bail!("WebSocket 握手失败");
    }

    // Step 2: Subscribe to user's IM channel
    let sub_channel = format!("/im/{}", uid);
    let subscribe = serde_json::json!([{
        "id": msg_id.to_string(),
        "channel": "/meta/subscribe",
        "subscription": sub_channel,
        "clientId": client_id,
    }]);
    write.send(WsMsg::Text(subscribe.to_string())).await?;
    msg_id += 1;
    log_info!("[ws] 已订阅 {}", sub_channel);

    // Step 3: Connect loop — receive messages
    let connect_msg = serde_json::json!([{
        "id": msg_id.to_string(),
        "channel": "/meta/connect",
        "connectionType": "websocket",
        "clientId": client_id,
    }]);
    write.send(WsMsg::Text(connect_msg.to_string())).await?;
    log_info!("[ws] 已连接, 开始接收消息...");

    // Read loop
    while let Some(msg) = read.next().await {
        match msg {
            Ok(WsMsg::Text(text)) => {
                if let Ok(arr) = serde_json::from_str::<Vec<serde_json::Value>>(&text) {
                    for item in arr {
                        let channel = item.get("channel").and_then(|v| v.as_str()).unwrap_or("").to_string();
                        // Skip meta responses
                        if channel.starts_with("/meta/") {
                            if channel == "/meta/connect" {
                                // Re-send connect for long-polling
                                msg_id += 1;
                                let reconnect = serde_json::json!([{
                                    "id": msg_id.to_string(),
                                    "channel": "/meta/connect",
                                    "connectionType": "websocket",
                                    "clientId": client_id,
                                }]);
                                write.send(WsMsg::Text(reconnect.to_string())).await?;
                            }
                            continue;
                        }
                        // Real IM message
                        if let Some(data) = item.get("data") {
                            let _ = tx.send(WsMessage {
                                channel: channel.clone(),
                                data: data.clone(),
                            });
                            log_info!("[ws] 收到推送: channel={}", channel);
                        }
                    }
                }
            }
            Ok(WsMsg::Close(_)) => {
                log_info!("[ws] 连接关闭");
                break;
            }
            Ok(other) => {
                log_info!("[ws] 非文本消息: {:?}", other);
            }
            Err(e) => {
                log_info!("[ws] 读取错误: {}", e);
                break;
            }
        }
    }

    Ok(())
}
