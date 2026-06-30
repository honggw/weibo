//! 认证相关的数据模型

use serde::{Deserialize, Serialize};

/// Login flow phases — drives the UI state machine
#[derive(Clone, Debug, Serialize, Deserialize)]
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
        items: Vec<crate::timeline::TimelineItem>,
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

/// Parsed cookie data for authentication
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CookieData {
    /// Full Cookie header string: "SUB=xxx; SUBP=yyy"
    pub header: String,
    /// Just the SUB value
    pub sub: String,
}

/// QR 码扫码状态 (来自 QR 轮询 API)
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum QrStatus {
    Waiting,
    Scanned,
    Confirmed { alt: String, redirect_url: String },
    Expired,
    Unknown { code: i64, msg: String, raw: serde_json::Value },
}
