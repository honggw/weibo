//! weibo-model — 业务服务层
//!
//! 封装微博 API 的业务逻辑 (auth, chat, timeline, qr_login)。
//! 依赖 weibo-domain + weibo-infra, 不依赖任何 UI 框架。

pub mod auth_service;
pub mod chat_service;
pub mod qr_login;
pub mod timeline_service;

// Re-export QrLogin for convenience
pub use qr_login::QrLogin;
