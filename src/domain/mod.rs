//! Domain models — pure data structures with zero dependencies.
//! These types are used across all layers (infra → model → viewmodel → view).

use std::fmt;

// ============================================================================
// Tabs
// ============================================================================

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ActiveTab {
    Home,
    Chat,
}

// ============================================================================
// Chat
// ============================================================================

/// A conversation contact in the DM list.
#[derive(Clone, Debug)]
pub struct Contact {
    pub user_id: String,
    pub screen_name: String,
    pub avatar: String,
    pub unread_count: u64,
    pub last_message: String,
    pub last_time: String,
    pub is_group: bool,
}

/// 消息类型枚举 (来自 HAR 中 type 字段)
#[derive(Clone, Debug, PartialEq)]
pub enum MsgType {
    /// 普通消息 (type=321)
    Normal,
    /// 系统消息: 入群通知等 (type=322)
    System,
    /// 撤回消息 (type=344)
    Recall,
    /// 其他未知类型
    Other(u64),
}

impl MsgType {
    pub fn from_api(type_val: u64) -> Self {
        match type_val {
            321 => MsgType::Normal,
            322 => MsgType::System,
            344 => MsgType::Recall,
            v => MsgType::Other(v),
        }
    }
}

/// 媒体类型枚举 (来自 HAR 中 media_type 字段)
#[derive(Clone, Debug, PartialEq)]
pub enum MediaType {
    /// 纯文本 (media_type=0)
    Text,
    /// 图片 (media_type=1, 有 fids 字段)
    Image,
    /// 引用/转发 (media_type=14, content 中包含引用块)
    Quote,
    /// 其他
    Other(u64),
}

impl MediaType {
    pub fn from_api(val: u64) -> Self {
        match val {
            0 => MediaType::Text,
            1 => MediaType::Image,
            14 => MediaType::Quote,
            v => MediaType::Other(v),
        }
    }
}

/// 单条聊天消息
#[derive(Clone, Debug)]
pub struct ChatMessage {
    pub id: String,
    pub sender_id: String,
    pub sender_name: String,
    /// 发送者头像 URL (来自 from_user.profile_image_url)
    pub sender_avatar: String,
    pub text: String,
    pub created_at: String,
    /// Unix 时间戳 (秒), 用于时间分组和格式化
    pub timestamp: u64,
    pub is_self: bool,
    /// 消息类型: Normal / System / Recall
    pub msg_type: MsgType,
    /// 媒体类型: Text / Image / Quote
    pub media_type: MediaType,
    /// 图片消息的文件 ID 列表 (media_type=1 时非空)
    /// 用于拼接缩略图 URL: https://upload.api.weibo.com/2/mss/msget_thumbnail?fid={}&high=240&width=240&source=209678993
    pub fids: Vec<String>,
    /// 消息发送者在群中的角色 (0=普通, 1=管理员, 4=群主)
    pub role: u8,
}

/// 微博表情
#[derive(Clone, Debug)]
pub struct Emotion {
    /// 表情文本标记, 如 "[不愧是你]"
    pub phrase: String,
    /// 表情图片 URL
    pub url: String,
}

/// 群信息
#[derive(Clone, Debug)]
pub struct GroupInfo {
    pub id: String,
    pub name: String,
    pub owner_uid: String,
    pub member_count: u64,
    pub members: Vec<GroupMember>,
}

/// 群成员
#[derive(Clone, Debug)]
pub struct GroupMember {
    pub uid: String,
    pub screen_name: String,
    pub avatar: String,
    pub is_admin: bool,
}

// ============================================================================
// Timeline
// ============================================================================

/// A single timeline item (post / hot-search entry)
#[derive(Clone, Debug)]
pub struct TimelineItem {
    pub user_name: String,
    pub text: String,
}

// ============================================================================
// Login
// ============================================================================

/// Login flow phases — drives the UI state machine
#[derive(Clone, Debug)]
pub enum LoginPhase {
    /// Checking saved cookies on startup
    CheckingCookie,
    /// Generic loading with status message
    Loading(String),
    /// QR code ready, waiting for scan
    WaitingScan {
        status: String,
        /// Raw PNG bytes for in-window QR display
        qr_png_bytes: Option<Vec<u8>>,
    },
    /// Login confirmed, exchanging ticket
    Exchanging(String),
    /// Fetching timeline from API
    FetchingHome,
    /// Timeline loaded and displayed
    HomeLoaded {
        items: Vec<TimelineItem>,
        title: String,
    },
    /// Error state
    Error(String),
}

impl LoginPhase {
    /// Short name for logging
    pub fn name(&self) -> &'static str {
        match self {
            LoginPhase::CheckingCookie => "CheckingCookie",
            LoginPhase::Loading(_) => "Loading",
            LoginPhase::WaitingScan { .. } => "WaitingScan",
            LoginPhase::Exchanging(_) => "Exchanging",
            LoginPhase::FetchingHome => "FetchingHome",
            LoginPhase::HomeLoaded { .. } => "HomeLoaded",
            LoginPhase::Error(_) => "Error",
        }
    }
}

// ============================================================================
// Cookie
// ============================================================================

/// Parsed cookie data for authentication
#[derive(Clone, Debug)]
pub struct CookieData {
    /// Full Cookie header string: "SUB=xxx; SUBP=yyy"
    pub header: String,
    /// Just the SUB value
    pub sub: String,
}

// ============================================================================
// Errors
// ============================================================================

/// Unified application error type
pub enum AppError {
    Network(String),
    Auth(String),
    Parse(String),
    Io(String),
}

impl fmt::Display for AppError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AppError::Network(msg) => write!(f, "网络错误: {}", msg),
            AppError::Auth(msg) => write!(f, "认证错误: {}", msg),
            AppError::Parse(msg) => write!(f, "解析错误: {}", msg),
            AppError::Io(msg) => write!(f, "IO错误: {}", msg),
        }
    }
}

impl From<anyhow::Error> for AppError {
    fn from(e: anyhow::Error) -> Self {
        AppError::Network(e.to_string())
    }
}
