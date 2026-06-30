//! Centralized application configuration constants.
//! Single source of truth for URLs, timeouts, UA strings, theme colors, etc.
//!
//! Runtime files (logs, QR images, cookies) are stored under `data_dir()`,
//! which must be initialized via `init_data_dir()` at startup.
//! If not initialized, falls back to the current working directory.

use std::path::PathBuf;
use std::sync::OnceLock;
use std::time::Duration;

// ============================================================================
// Data directory — initialized at app startup, fallback to CWD otherwise
// ============================================================================

static DATA_DIR: OnceLock<PathBuf> = OnceLock::new();

/// Initialize the data directory for runtime files (logs, QR images, cookies).
/// Must be called once at app startup. For Tauri, pass `app.path().app_data_dir()`.
pub fn init_data_dir(dir: PathBuf) {
    let _ = DATA_DIR.set(dir);
}

/// Returns the data directory for runtime files.
/// If not initialized, falls back to the current working directory.
pub fn data_dir() -> &'static PathBuf {
    DATA_DIR.get_or_init(|| PathBuf::from("."))
}

/// Build a path under the data directory for a runtime file.
pub fn data_path(filename: &str) -> PathBuf {
    data_dir().join(filename)
}

// ============================================================================
// Network
// ============================================================================

pub const WEIBO_BASE_URL: &str = "https://weibo.com";
pub const PASSPORT_URL: &str = "https://passport.weibo.com";
pub const DEFAULT_UA: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 \
                              (KHTML, like Gecko) Chrome/148.0.0.0 Safari/537.36";
pub const REQUEST_TIMEOUT: Duration = Duration::from_secs(15);
pub const QR_POLL_INTERVAL: Duration = Duration::from_secs(1);
pub const YIELD_DURATION: Duration = Duration::from_millis(100);

// ============================================================================
// Files (bare filenames — resolve to full paths via `data_path()`)
// ============================================================================

pub const COOKIE_FILE: &str = "weibo_cookies.json";
pub const QR_IMAGE_FILE: &str = "weibo_qr.png";
pub const LOG_FILE: &str = "weibo_app.log";

// ============================================================================
// Login
// ============================================================================

pub const QR_LOGIN_URL: &str = "https://passport.weibo.com/sso/signin?entry=miniblog&r=https%3A%2F%2Fweibo.com%2F";
pub const QR_IMAGE_API: &str = "https://passport.weibo.com/sso/v2/qrcode/image";
pub const QR_CHECK_API: &str = "https://passport.weibo.com/sso/v2/qrcode/check";
pub const QR_SCAN_TIMEOUT: Duration = Duration::from_secs(300); // 5 minutes

// ============================================================================
// APIs
// ============================================================================

pub const API_CONFIG: &str = "https://weibo.com/ajax/config/get_config";
pub const API_TIMELINE: &str = "https://weibo.com/ajax/statuses/home_timeline";
pub const API_FRIENDSHIPS: &str = "https://weibo.com/ajax/friendships/friends";
pub const API_MYMBLOG: &str = "https://weibo.com/ajax/statuses/mymblog";
pub const API_HOTSEARCH: &str = "https://weibo.com/ajax/side/hotSearch";

// ============================================================================
// Theme
// ============================================================================

pub const COLOR_BG: u32 = 0x1a1a2e;
pub const COLOR_CARD: u32 = 0x16213e;
pub const COLOR_ACCENT: u32 = 0xe8633a;
pub const COLOR_TEXT_PRIMARY: u32 = 0xe8e8e8;
pub const COLOR_TEXT_SECONDARY: u32 = 0x888888;
pub const COLOR_HEADER_BG: u32 = 0x0f3460;
pub const COLOR_LOGOUT_BTN: u32 = 0x333355;
pub const COLOR_QR_BORDER: u32 = 0x333366;

pub const FONT_FAMILY: &str = "Microsoft YaHei, sans-serif";
pub const FONT_SIZE_TITLE: f32 = 20.0;
pub const FONT_SIZE_SUBTITLE: f32 = 12.0;
pub const FONT_SIZE_BODY: f32 = 16.0;
pub const FONT_SIZE_CARD_USER: f32 = 14.0;
pub const FONT_SIZE_CARD_TEXT: f32 = 13.0;

pub const QR_DISPLAY_SIZE: f32 = 200.0;
pub const QR_CONTAINER_SIZE: f32 = 220.0;

// ============================================================================
// Limits
// ============================================================================

pub const MAX_FOLLOWED_USERS: usize = 20;
pub const MAX_POSTS_PER_USER: usize = 3;
pub const MAX_HOTSEARCH_ITEMS: usize = 15;
pub const MAX_TIMELINE_ITEMS: usize = 20;
