//! weibo-viewmodel — ViewModel 层 (纯逻辑 + VMContext trait)
//!
//! 依赖 weibo-domain + weibo-model + tokio (sync only)。
//! **不依赖** 任何 UI 框架 (GPUI, Tauri 等)。

pub mod app_state;
pub mod chat_vm;
pub mod context;
pub mod home_vm;
pub mod login_vm;
