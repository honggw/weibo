//! Chat ViewModel — conversation list + message loading + send.

use crate::domain::{ChatMessage, Contact, Emotion, GroupInfo};
use crate::model::chat_service;
use crate::viewmodel::root_vm::AppRoot;
use crate::log_info;
use gpui::*;

pub struct ChatData {
    pub contacts: Vec<Contact>,
    pub loading: bool,
    pub my_uid: String,
    /// Currently selected contact UID
    pub selected_uid: Option<String>,
    /// Messages for the selected conversation
    pub messages: Vec<ChatMessage>,
    pub messages_loading: bool,
    /// Oldest message ID for pagination (load earlier messages)
    pub oldest_mid: Option<String>,
    /// If true, there are more older messages to load
    pub has_more: bool,
    /// Draft text in the input box
    pub draft_text: String,
    /// Focus handle for the input field (for cursor tracking)
    pub input_focus: Option<FocusHandle>,
    /// List state for message rendering (virtual scroll)
    pub msg_list_state: Option<ListState>,
    /// 表情列表 (懒加载缓存)
    pub emotions: Vec<Emotion>,
    /// 是否显示表情面板
    pub show_emoji_panel: bool,
    /// 会话搜索文本
    pub search_text: String,
    /// 当前群信息 (群聊时)
    pub group_info: Option<GroupInfo>,
}

impl ChatData {
    pub fn new() -> Self {
        Self {
            contacts: Vec::new(), loading: true, my_uid: String::new(),
            selected_uid: None, messages: Vec::new(), messages_loading: false,
            oldest_mid: None, has_more: true, draft_text: String::new(), input_focus: None, msg_list_state: None,
            emotions: Vec::new(), show_emoji_panel: false,
            search_text: String::new(),
            group_info: None,
        }
    }
}

// ---------------------------------------------------------------------------
// ListState helpers — account for TimeSeparator items (see chat_screen::build_list_items)
// ---------------------------------------------------------------------------

/// 计算包含时间分割线的消息列表项总数。
/// 规则与 `chat_screen::build_list_items` 保持一致:
/// 如果两条相邻消息的 timestamp 间隔 > 300 秒 (5分钟), 插入一条分割线。
pub fn count_list_items(msgs: &[ChatMessage]) -> usize {
    let mut separators: usize = 0;
    for (i, msg) in msgs.iter().enumerate() {
        let needs_separator = i == 0
            || (msg.timestamp > 0
                && msgs[i - 1].timestamp > 0
                && msg.timestamp.saturating_sub(msgs[i - 1].timestamp) > 300);
        if needs_separator && msg.timestamp > 0 {
            separators += 1;
        }
    }
    msgs.len() + separators
}

/// 重建 msg_list_state, 使用包含 TimeSeparator 的真实项数。
pub fn rebuild_msg_list_state(chat: &mut ChatData, alignment: ListAlignment) {
    let item_count = count_list_items(&chat.messages);
    chat.msg_list_state = Some(ListState::new(item_count, alignment, px(400.0)));
}

/// Spawn loading contacts list.
pub fn load_chat_data(cx: &mut Context<AppRoot>, handle: &tokio::runtime::Handle) {
    let handle = handle.clone();
    cx.spawn(|this: WeakEntity<AppRoot>, cx: &mut AsyncApp| {
        let mut cx = cx.clone();
        async move {
            let (contacts, my_info) = handle.block_on(async {
                let contacts = chat_service::fetch_contacts().await.unwrap_or_default();
                let my_info = chat_service::fetch_primary_info().await;
                (contacts, my_info)
            });

            this.update(&mut cx, |v, cx| {
                let chat = v.chat_data.get_or_insert_with(ChatData::new);
                chat.contacts = contacts;
                chat.loading = false;
                if let Some((uid, _name)) = my_info {
                    chat.my_uid = uid;
                }
                log_info!("[chat_vm] {} 个会话已加载", chat.contacts.len());
                v.chat_list_state = ListState::new(chat.contacts.len(), ListAlignment::Top, px(60.0));
                cx.notify();
            }).ok();
        }
    }).detach();
}

/// Select a contact and load message history.
pub fn select_contact(cx: &mut Context<AppRoot>, handle: &tokio::runtime::Handle, uid: String, my_uid: String, is_group: bool) {
    let handle = handle.clone();
    let uid_for_group = uid.clone(); // clone before move for group info fetch
    let handle_for_group = handle.clone();
    cx.spawn(move |this: WeakEntity<AppRoot>, cx: &mut AsyncApp| {
        let mut cx = cx.clone();
        async move {
            let messages = handle.block_on(chat_service::fetch_messages(&uid, &my_uid, is_group, None));

            let uid_for_selected = uid.clone();
            let uid_for_report = uid.clone();
            this.update(&mut cx, |v, cx| {
                if let Some(chat) = v.chat_data.as_mut() {
                    chat.selected_uid = Some(uid_for_selected);
                    // Store oldest message ID for pagination
                    chat.oldest_mid = messages.first().map(|m| m.id.clone());
                    chat.has_more = messages.len() >= 30; // API returns up to 30 per page
                    chat.messages = messages;
                    chat.messages_loading = false;
                    let count = chat.messages.len();
                    log_info!("[chat_vm] 加载 {} 条消息, oldest_mid={:?}, has_more={}", count, chat.oldest_mid, chat.has_more);
                    rebuild_msg_list_state(chat, ListAlignment::Bottom);
                }
                cx.notify();
            }).ok();

            // Report read status
            handle.spawn(async move {
                chat_service::report_read(&uid_for_report).await;
            });
        }
    }).detach();

    // Fetch group info in background (separate task, non-blocking)
    if is_group {
        let gid = uid_for_group;
        let h = handle_for_group;
        cx.spawn(|this: WeakEntity<AppRoot>, cx: &mut AsyncApp| {
            let mut cx = cx.clone();
            async move {
                if let Some(info) = h.block_on(chat_service::fetch_group_info(&gid)) {
                    log_info!("[chat_vm] 群信息已获取: {:?}", info.name);
                    this.update(&mut cx, |v, cx| {
                        if let Some(chat) = v.chat_data.as_mut() {
                            chat.group_info = Some(info);
                        }
                        cx.notify();
                    }).ok();
                }
            }
        }).detach();
    }
}

/// Load older messages (pagination — scroll up).
pub fn load_more_messages(cx: &mut Context<AppRoot>, handle: &tokio::runtime::Handle, uid: String, my_uid: String, is_group: bool, oldest_mid: String) {
    let handle = handle.clone();
    cx.spawn(move |this: WeakEntity<AppRoot>, cx: &mut AsyncApp| {
        let mut cx = cx.clone();
        async move {
            let older = handle.block_on(chat_service::fetch_messages(&uid, &my_uid, is_group, Some(&oldest_mid)));

            this.update(&mut cx, |v, cx| {
                if let Some(chat) = v.chat_data.as_mut() {
                    chat.messages_loading = false;
                    let count = older.len();
                    if count > 0 {
                        // Update oldest_mid for next pagination
                        chat.oldest_mid = older.first().map(|m| m.id.clone());
                        chat.has_more = count >= 30;
                        // Prepend older messages (they're in chronological order from API)
                        let mut all = older;
                        all.append(&mut chat.messages);
                        chat.messages = all;
                        // Use Top alignment to maintain scroll position when loading older messages
                        rebuild_msg_list_state(chat, ListAlignment::Top);
                        log_info!("[chat_vm] 加载更早 {} 条消息, 总计 {} 条, has_more={}", count, chat.messages.len(), chat.has_more);
                    } else {
                        chat.has_more = false;
                        log_info!("[chat_vm] 没有更早的消息了");
                    }
                }
                cx.notify();
            }).ok();
        }
    }).detach();
}

/// Send a message and append to the list.
pub fn send_message(cx: &mut Context<AppRoot>, handle: &tokio::runtime::Handle, uid: String, text: String, is_group: bool) {
    let handle = handle.clone();
    cx.spawn(move |this: WeakEntity<AppRoot>, cx: &mut AsyncApp| {
        let mut cx = cx.clone();
        async move {
            let sent = handle.block_on(chat_service::send_message(&uid, &text, is_group));

            this.update(&mut cx, |v, cx| {
                if let Some(chat) = v.chat_data.as_mut() {
                    if let Some(msg) = sent {
                        chat.messages.push(msg);
                        // Rebuild ListState so new message is visible (Accounts for TimeSeparators)
                        rebuild_msg_list_state(chat, ListAlignment::Bottom);
                    }
                }
                cx.notify();
            }).ok();
        }
    }).detach();
}
