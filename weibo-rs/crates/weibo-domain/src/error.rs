//! 统一错误类型

use std::fmt;

/// Unified application error type
#[derive(Debug)]
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
