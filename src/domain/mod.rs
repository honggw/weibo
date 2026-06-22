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

/// A single chat message.
#[derive(Clone, Debug)]
pub struct ChatMessage {
    pub id: String,
    pub sender_id: String,
    pub sender_name: String,
    pub text: String,
    pub created_at: String,
    pub is_self: bool,
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
