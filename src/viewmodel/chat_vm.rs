//! Chat ViewModel — conversation list + message loading + send.

use crate::domain::{ChatMessage, Contact};
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
}

impl ChatData {
    pub fn new() -> Self {
        Self {
            contacts: Vec::new(), loading: true, my_uid: String::new(),
            selected_uid: None, messages: Vec::new(), messages_loading: false,
            oldest_mid: None, has_more: true, draft_text: String::new(), input_focus: None, msg_list_state: None,
        }
    }
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
    cx.spawn(move |this: WeakEntity<AppRoot>, cx: &mut AsyncApp| {
        let mut cx = cx.clone();
        async move {
            let messages = handle.block_on(chat_service::fetch_messages(&uid, &my_uid, is_group, None));

            this.update(&mut cx, |v, cx| {
                if let Some(chat) = v.chat_data.as_mut() {
                    chat.selected_uid = Some(uid);
                    // Store oldest message ID for pagination
                    chat.oldest_mid = messages.first().map(|m| m.id.clone());
                    chat.has_more = messages.len() >= 30; // API returns up to 30 per page
                    chat.messages = messages;
                    chat.messages_loading = false;
                    let count = chat.messages.len();
                    log_info!("[chat_vm] 加载 {} 条消息, oldest_mid={:?}, has_more={}", count, chat.oldest_mid, chat.has_more);
                    chat.msg_list_state = Some(ListState::new(count, ListAlignment::Top, px(50.0)));
                }
                cx.notify();
            }).ok();
        }
    }).detach();
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
                    let count = older.len();
                    if count > 0 {
                        // Update oldest_mid for next pagination
                        chat.oldest_mid = older.first().map(|m| m.id.clone());
                        chat.has_more = count >= 30;
                        // Prepend older messages (they're in chronological order from API)
                        let mut all = older;
                        all.append(&mut chat.messages);
                        chat.messages = all;
                        chat.msg_list_state = Some(ListState::new(chat.messages.len(), ListAlignment::Top, px(50.0)));
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
                    }
                }
                cx.notify();
            }).ok();
        }
    }).detach();
}
