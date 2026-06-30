//! weibo-domain — 纯数据模型 (零外部依赖，仅 serde 序列化)
//!
//! 这些类型被所有其他层引用：infra → model → viewmodel → tauri

pub mod auth;
pub mod chat;
pub mod error;
pub mod tabs;
pub mod timeline;

// 重新导出常用类型
pub use auth::{CookieData, LoginPhase, QrStatus};
pub use chat::{ChatMessage, Contact, Emotion, GroupInfo, GroupMember, MediaType, MsgType};
pub use error::AppError;
pub use tabs::ActiveTab;
pub use timeline::TimelineItem;
