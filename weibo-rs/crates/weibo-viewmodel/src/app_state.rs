//! AppState — 应用全局状态 (纯数据，不含任何 UI 框架类型)
//!
//! 从原 root_vm.rs 的 AppRoot 和 chat_vm.rs 的 ChatData 中提取纯数据。
//! ❌ 不包含: ListState, FocusHandle (GPUI 专属), draft_text, show_emoji_panel, search_text (前端 UI 局部状态)

use weibo_domain::*;
use weibo_model::qr_login::QrLogin;

/// 应用全局状态 (纯数据，不含任何 UI 框架类型)
pub struct AppState {
    /// 当前登录/加载阶段
    pub phase: LoginPhase,
    /// 当前激活 Tab
    pub active_tab: ActiveTab,
    /// 时间线状态
    pub timeline: TimelineState,
    /// 聊天状态
    pub chat: ChatState,
    /// DM 未读数
    pub dm_unread: u64,
    /// QR 登录会话 (仅在登录流程中有值)
    pub qr_session: Option<QrSession>,
}

impl AppState {
    pub fn new() -> Self {
        Self {
            phase: LoginPhase::CheckingCookie,
            active_tab: ActiveTab::Home,
            timeline: TimelineState::new(),
            chat: ChatState::new(),
            dm_unread: 0,
            qr_session: None,
        }
    }
}

impl Default for AppState {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Clone)]
pub struct TimelineState {
    pub items: Vec<TimelineItem>,
    pub title: String,
    pub since_id: String,
    pub feed_list_id: Option<String>,
    pub loading_more: bool,
}

impl TimelineState {
    pub fn new() -> Self {
        Self {
            items: Vec::new(),
            title: String::new(),
            since_id: String::new(),
            feed_list_id: None,
            loading_more: false,
        }
    }
}

impl Default for TimelineState {
    fn default() -> Self {
        Self::new()
    }
}

/// 聊天状态 (纯业务数据)
pub struct ChatState {
    pub contacts: Vec<Contact>,
    pub contacts_loading: bool,
    pub my_uid: String,
    pub selected_uid: Option<String>,
    pub messages: Vec<ChatMessage>,
    pub messages_loading: bool,
    pub oldest_mid: Option<String>,
    pub has_more: bool,
    pub emotions: Vec<Emotion>,
    pub group_info: Option<GroupInfo>,
    // ❌ 不包含:
    //   - ListState (GPUI 专属)
    //   - FocusHandle (GPUI 专属)
    //   - draft_text (前端 UI 局部状态)
    //   - show_emoji_panel (前端 UI 局部状态)
    //   - search_text (前端 UI 局部状态)
}

impl ChatState {
    pub fn new() -> Self {
        Self {
            contacts: Vec::new(),
            contacts_loading: true,
            my_uid: String::new(),
            selected_uid: None,
            messages: Vec::new(),
            messages_loading: false,
            oldest_mid: None,
            has_more: true,
            emotions: Vec::new(),
            group_info: None,
        }
    }
}

impl Default for ChatState {
    fn default() -> Self {
        Self::new()
    }
}

/// QR 登录会话 (仅在登录流程中有值)
pub struct QrSession {
    pub login: QrLogin,
    pub polling: bool,
    /// 确认后存储 alt ticket (用于 exchange)
    pub alt: Option<String>,
    /// 确认后存储 redirect_url (用于 exchange)
    pub redirect_url: Option<String>,
}
