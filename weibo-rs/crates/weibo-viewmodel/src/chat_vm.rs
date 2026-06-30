//! Chat ViewModel — conversation list + message loading + send.
//! 所有函数使用泛型 `<C: VMContext>`, 不依赖任何 UI 框架。

use weibo_domain::ChatMessage;
use weibo_model::chat_service;
use weibo_infra::log_info;

use crate::app_state::AppState;
use crate::context::VMContext;

/// 加载联系人列表和当前用户信息
pub fn load_contacts<C: VMContext<State = AppState>>(ctx: &C) {
    ctx.spawn_task(
        async {
            let contacts = chat_service::fetch_contacts().await.unwrap_or_default();
            let my_info = chat_service::fetch_primary_info().await;
            (contacts, my_info)
        },
        |state, (contacts, my_info)| {
            state.chat.contacts = contacts;
            state.chat.contacts_loading = false;
            if let Some((uid, _name)) = my_info {
                state.chat.my_uid = uid;
            }
            log_info!("[chat_vm] {} 个会话已加载", state.chat.contacts.len());
        },
    );
}

/// 选中联系人, 加载消息历史
pub fn select_contact<C: VMContext<State = AppState>>(
    ctx: &C,
    state: &AppState,
    uid: String,
    is_group: bool,
) {
    let my_uid = state.chat.my_uid.clone();
    let uid_clone = uid.clone();

    ctx.spawn_task(
        async move {
            chat_service::fetch_messages(&uid, &my_uid, is_group, None).await
        },
        move |state, messages| {
            state.chat.selected_uid = Some(uid_clone);
            state.chat.oldest_mid = messages.first().map(|m| m.id.clone());
            state.chat.has_more = messages.len() >= 30;
            state.chat.messages = messages;
            state.chat.messages_loading = false;
            log_info!(
                "[chat_vm] 加载 {} 条消息, oldest_mid={:?}, has_more={}",
                state.chat.messages.len(),
                state.chat.oldest_mid,
                state.chat.has_more
            );
        },
    );
}

/// 发送消息
pub fn send_message<C: VMContext<State = AppState>>(
    ctx: &C,
    uid: String,
    text: String,
    is_group: bool,
) {
    ctx.spawn_task(
        async move { chat_service::send_message(&uid, &text, is_group).await },
        |state, sent| {
            if let Some(msg) = sent {
                state.chat.messages.push(msg);
            }
        },
    );
}

/// 加载更早的消息 (分页向上滚动)
pub fn load_older_messages<C: VMContext<State = AppState>>(ctx: &C, state: &AppState) {
    let uid = match &state.chat.selected_uid {
        Some(uid) => uid.clone(),
        None => return,
    };
    let my_uid = state.chat.my_uid.clone();
    let oldest_mid = match &state.chat.oldest_mid {
        Some(mid) => mid.clone(),
        None => return,
    };
    let is_group = state
        .chat
        .contacts
        .iter()
        .find(|c| c.user_id == uid)
        .map(|c| c.is_group)
        .unwrap_or(false);

    ctx.spawn_task(
        async move {
            chat_service::fetch_messages(&uid, &my_uid, is_group, Some(&oldest_mid)).await
        },
        move |state, older| {
            state.chat.messages_loading = false;
            let count = older.len();
            if count > 0 {
                state.chat.oldest_mid = older.first().map(|m| m.id.clone());
                state.chat.has_more = count >= 30;
                let mut all = older;
                all.append(&mut state.chat.messages);
                state.chat.messages = all;
                log_info!(
                    "[chat_vm] 加载更早 {} 条消息, 总计 {} 条, has_more={}",
                    count,
                    state.chat.messages.len(),
                    state.chat.has_more
                );
            } else {
                state.chat.has_more = false;
                log_info!("[chat_vm] 没有更早的消息了");
            }
        },
    );
}

/// 切换 Tab: 如果是 Chat tab 则触发加载
pub fn switch_tab<C: VMContext<State = AppState>>(
    ctx: &C,
    state: &mut AppState,
    tab: weibo_domain::ActiveTab,
) {
    state.active_tab = tab;
    if tab == weibo_domain::ActiveTab::Chat
        && state.chat.contacts.is_empty()
        && state.chat.contacts_loading
    {
        load_contacts(ctx);
    }
    ctx.notify();
}

/// 处理 WebSocket 推送消息
pub fn handle_ws_message(state: &mut AppState, msg: weibo_infra::ws_client::WsMessage) {
    let sender_id = msg
        .data
        .get("from_uid")
        .or_else(|| msg.data.get("sender_id"))
        .and_then(|v| v.as_u64())
        .map(|v| v.to_string())
        .unwrap_or_default();
    let sender_name = msg
        .data
        .get("from_user")
        .and_then(|u| u.get("screen_name"))
        .or_else(|| msg.data.get("sender_screen_name"))
        .and_then(|v| v.as_str())
        .unwrap_or("?")
        .to_string();
    let text = msg
        .data
        .get("content")
        .or_else(|| msg.data.get("text"))
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let gid = msg
        .data
        .get("gid")
        .and_then(|v| v.as_u64())
        .map(|v| v.to_string());
    let msg_uid = gid.unwrap_or_else(|| sender_id.clone());

    if text.is_empty() {
        return;
    }

    let is_self = sender_id == state.chat.my_uid;
    let fids: Vec<String> = msg
        .data
        .get("fids")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_u64().map(|n| n.to_string()))
                .collect()
        })
        .unwrap_or_default();

    let new_msg = ChatMessage {
        id: String::new(),
        sender_id: sender_id.clone(),
        sender_name,
        sender_avatar: msg
            .data
            .get("from_user")
            .and_then(|u| u.get("profile_image_url"))
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string(),
        text: text.clone(),
        created_at: String::new(),
        timestamp: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs(),
        is_self,
        msg_type: weibo_domain::MsgType::from_api(
            msg.data.get("type").and_then(|v| v.as_u64()).unwrap_or(321),
        ),
        media_type: weibo_domain::MediaType::from_api(
            msg.data.get("media_type").and_then(|v| v.as_u64()).unwrap_or(0),
        ),
        fids,
        role: msg
            .data
            .get("from_user_role")
            .and_then(|v| v.as_u64())
            .unwrap_or(0) as u8,
    };

    // If this contact is currently selected, append to message list
    if state.chat.selected_uid.as_ref() == Some(&msg_uid) {
        state.chat.messages.push(new_msg);
        log_info!(
            "[ws] 已追加消息到当前会话: {}",
            text.chars().take(20).collect::<String>()
        );
    }

    // Update contact preview
    if let Some(contact) = state.chat.contacts.iter_mut().find(|c| c.user_id == msg_uid) {
        contact.last_message = text.chars().take(50).collect();
        if !is_self {
            contact.unread_count += 1;
        }
    }

    // 非自己的消息播放提示音
    if !is_self {
        weibo_infra::audio::play_notification();
    }

    log_info!("[ws] 推送处理完成: channel={}, contact={}", msg.channel, msg_uid);
}

/// 获取未读 DM 计数
pub fn fetch_dm_unread_count<C: VMContext<State = AppState>>(ctx: &C) {
    ctx.spawn_task(
        async { chat_service::fetch_dm_unread().await },
        |state, count| {
            state.dm_unread = count;
        },
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::context::MockContext;

    #[test]
    fn test_switch_tab_to_home_notifies() {
        let ctx = MockContext::new(AppState::new());
        {
            let mut state = ctx.state.lock().unwrap();
            switch_tab(&ctx, &mut state, weibo_domain::ActiveTab::Home);
        }
        let state = ctx.state();
        assert!(matches!(state.active_tab, weibo_domain::ActiveTab::Home));
        assert!(ctx.notified_count() > 0);
    }
}
