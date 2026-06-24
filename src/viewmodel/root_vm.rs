//! Root ViewModel — top-level state machine, routes between login and home.

use gpui::*;

use crate::domain::{ActiveTab, ChatMessage, LoginPhase};
use crate::infra::cookie_io;
use crate::infra::ws_client::WsMessage;
use crate::model::{auth_service, chat_service, timeline_service};
use crate::view::screens;
use crate::log_info;

use super::chat_vm::ChatData;
use super::home_vm;
use super::login_vm;

pub struct AppRoot {
    pub phase: LoginPhase,
    pub tokio_handle: tokio::runtime::Handle,
    pub list_state: ListState,
    /// Pagination: last status id for "load more"
    pub since_id: String,
    /// True while loading more items (prevents duplicate triggers)
    pub loading_more: bool,
    /// The list_id for friendstimeline (cached from allGroups)
    pub feed_list_id: Option<String>,
    /// Currently active tab
    pub active_tab: ActiveTab,
    /// DM unread count
    pub dm_unread: u64,
    /// Whether DM fetch has been initiated
    dm_fetched: bool,
    /// Whether WebSocket has been started
    ws_started: bool,
    /// Chat data (lazy-loaded when chat tab is opened)
    pub chat_data: Option<ChatData>,
    /// List state for chat contact list
    pub chat_list_state: ListState,
    /// List state for chat messages
    pub msg_list_state: ListState,
}

impl AppRoot {
    pub fn new(cx: &mut Context<Self>, tokio_handle: tokio::runtime::Handle) -> Self {
        let this = Self {
            phase: LoginPhase::CheckingCookie,
            tokio_handle: tokio_handle.clone(),
            list_state: ListState::new(0, ListAlignment::Top, px(200.0)),
            since_id: String::new(),
            loading_more: false,
            feed_list_id: None,
            active_tab: ActiveTab::Home,
            dm_unread: 0,
            dm_fetched: false,
            ws_started: false,
            chat_data: None,
            chat_list_state: ListState::new(0, ListAlignment::Top, px(60.0)),
            msg_list_state: ListState::new(0, ListAlignment::Top, px(50.0)),
        };

        if let Some(cookie) = auth_service::load_saved_cookie() {
            log_info!("发现已保存的 Cookie, 尝试验证...");
            home_vm::start_cookie_flow(cx, &this.tokio_handle, cookie);
        } else {
            log_info!("未发现 Cookie, 进入扫码登录");
            login_vm::start_login_flow(cx, &this.tokio_handle);
        }

        this
    }

    pub fn switch_tab(&mut self, cx: &mut Context<Self>, tab: ActiveTab) {
        self.active_tab = tab;
        if tab == ActiveTab::Chat {
            crate::viewmodel::chat_vm::load_chat_data(cx, &self.tokio_handle);
        }
        cx.notify();
    }

    /// Handle an incoming WebSocket push message.
    fn handle_ws_message(chat: &mut ChatData, msg: WsMessage) {
        let channel = &msg.channel;
        // Parse message data: expected fields from groupchat or DM push
        let sender_id = msg.data.get("from_uid").or_else(|| msg.data.get("sender_id"))
            .and_then(|v| v.as_u64()).map(|v| v.to_string()).unwrap_or_default();
        let sender_name = msg.data.get("from_user").and_then(|u| u.get("screen_name"))
            .or_else(|| msg.data.get("sender_screen_name"))
            .and_then(|v| v.as_str()).unwrap_or("?").to_string();
        let text = msg.data.get("content").or_else(|| msg.data.get("text"))
            .and_then(|v| v.as_str()).unwrap_or("").to_string();
        let gid = msg.data.get("gid").and_then(|v| v.as_u64()).map(|v| v.to_string());
        let msg_uid = gid.or_else(|| Some(sender_id.clone()));
        let contact_uid = msg_uid.unwrap_or_default();

        if text.is_empty() { return; }

        let is_self = sender_id == chat.my_uid;
        // 解析 fids 为 JSON 数组 (API 返回的是 Array 而非 String)
        let fids: Vec<String> = msg.data.get("fids")
            .and_then(|v| v.as_array())
            .map(|arr| arr.iter()
                .filter_map(|v| v.as_u64().map(|n| n.to_string()))
                .collect())
            .unwrap_or_default();
        let new_msg = ChatMessage {
            id: String::new(),
            sender_id,
            sender_name,
            sender_avatar: msg.data.get("from_user")
                .and_then(|u| u.get("profile_image_url"))
                .and_then(|v| v.as_str()).unwrap_or("").to_string(),
            text: text.clone(),
            created_at: String::new(),
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_secs(),
            is_self,
            msg_type: crate::domain::MsgType::from_api(
                msg.data.get("type").and_then(|v| v.as_u64()).unwrap_or(321)
            ),
            media_type: crate::domain::MediaType::from_api(
                msg.data.get("media_type").and_then(|v| v.as_u64()).unwrap_or(0)
            ),
            fids,
            role: msg.data.get("from_user_role")
                .and_then(|v| v.as_u64())
                .unwrap_or(0) as u8,
        };

        // If this contact is currently selected, append to message list
        if chat.selected_uid.as_ref() == Some(&contact_uid) {
            chat.messages.push(new_msg);
            // Incrementally update ListState (preserves cached item heights)
            crate::viewmodel::chat_vm::update_msg_list_state_append(chat);
            log_info!("[ws] 已追加消息到当前会话: {}", text.chars().take(20).collect::<String>());
        }

        // Update contact preview (last message) in the list
        if let Some(contact) = chat.contacts.iter_mut().find(|c| c.user_id == contact_uid) {
            contact.last_message = text.chars().take(50).collect();
            if !is_self {
                contact.unread_count += 1;
            }
        }

        // 非自己的消息播放提示音
        if !is_self {
            crate::infra::audio::play_notification();
        }

        log_info!("[ws] 推送处理完成: channel={}, contact={}", channel, contact_uid);
    }

    pub fn logout(&mut self, cx: &mut Context<Self>) {
        log_info!("用户点击登出");
        cookie_io::delete();
        self.phase = LoginPhase::Loading("正在登出...".into());
        cx.notify();
        login_vm::start_login_flow(cx, &self.tokio_handle);
    }

    /// Fetch DM unread count periodically
    pub fn fetch_dm_count(cx: &mut Context<Self>, handle: &tokio::runtime::Handle) {
        let handle = handle.clone();
        cx.spawn(|this: WeakEntity<AppRoot>, cx: &mut AsyncApp| {
            let mut cx = cx.clone();
            async move {
                let count = handle.block_on(chat_service::fetch_dm_unread());
                this.update(&mut cx, |v, cx| {
                    v.dm_unread = count;
                    cx.notify();
                }).ok();
            }
        }).detach();
    }

    /// Trigger "load more" when scrolling near the bottom.
    fn try_load_more(&mut self, cx: &mut Context<Self>, visible_end: usize, total: usize) {
        if self.loading_more || self.since_id.is_empty() {
            return;
        }
        // Trigger when within 3 items of the end
        if visible_end + 3 < total {
            return;
        }

        log_info!("[load_more] 滚动到底, 加载更多 (since_id={})...", self.since_id);
        self.loading_more = true;

        let handle = self.tokio_handle.clone();
        let since_id = self.since_id.clone();
        let feed_list_id = self.feed_list_id.clone();

        cx.spawn(|this: WeakEntity<AppRoot>, cx: &mut AsyncApp| {
            let mut cx = cx.clone();
            async move {
                let (new_items, new_since_id) =
                    handle.block_on(timeline_service::load_more(&since_id, &feed_list_id));

                this.update(&mut cx, |v, cx| {
                    if let LoginPhase::HomeLoaded { ref mut items, ref mut title } = v.phase {
                        let old_len = items.len();
                        items.extend(new_items);
                        v.since_id = new_since_id;
                        v.loading_more = false;
                        log_info!("[load_more] 追加 {} 条, 总计 {} 条", items.len() - old_len, items.len());
                        v.list_state = ListState::new(items.len(), ListAlignment::Top, px(200.0));
                        *title = format!("📰 首页时间线 ({}条)", items.len());
                    }
                    cx.notify();
                }).ok();
            }
        }).detach();
    }
}

impl Render for AppRoot {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // Register scroll handler on list_state for infinite scroll
        let total = match &self.phase {
            LoginPhase::HomeLoaded { items, .. } => items.len(),
            _ => 0,
        };
        if total > 0 && !self.since_id.is_empty() {
            let total = total;
            self.list_state.set_scroll_handler(cx.listener(
                move |this, event: &ListScrollEvent, _window, cx| {
                    this.try_load_more(cx, event.visible_range.end, total);
                },
            ));
        }

        // One-time init when logged in
        if matches!(self.phase, LoginPhase::HomeLoaded { .. }) {
            if !self.dm_fetched {
                self.dm_fetched = true;
                Self::fetch_dm_count(cx, &self.tokio_handle);
            }
            if !self.ws_started {
                self.ws_started = true;
                // Only start WebSocket if we have a valid user ID
                if let Some(uid) = self.chat_data.as_ref().and_then(|c| if c.my_uid.is_empty() { None } else { Some(c.my_uid.clone()) }) {
                    log_info!("[ws] 启动 WebSocket, uid={}", uid);
                    let mut rx = crate::model::chat_service::start_ws(uid, &self.tokio_handle);
                    // Spawn GPUI task to poll WS messages and update chat_data
                    cx.spawn(|this: WeakEntity<AppRoot>, cx: &mut AsyncApp| {
                        let mut cx = cx.clone();
                        async move {
                            while let Some(msg) = rx.recv().await {
                                if let Err(_) = this.update(&mut cx, |v, cx| {
                                    let chat = v.chat_data.get_or_insert_with(ChatData::new);
                                    Self::handle_ws_message(chat, msg);
                                    cx.notify();
                                }) {
                                    break; // Entity released
                                }
                            }
                        }
                    }).detach();
                } else {
                    log_info!("[ws] 跳过启动 WebSocket: my_uid 不可用");
                }
            }
        }

        screens::root_screen::render(
            &self.phase,
            &self.active_tab,
            self.dm_unread,
            &self.list_state,
            self.chat_data.as_ref(),
            &self.chat_list_state,
            &self.msg_list_state,
            cx,
        )
    }
}
