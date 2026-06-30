//! Tauri IPC commands — 薄封装, 调用 weibo-viewmodel 函数

pub mod auth;
pub mod chat;
pub mod timeline;

use std::sync::Arc;
use tokio::sync::RwLock;
use weibo_viewmodel::app_state::AppState;
use crate::tauri_context::TauriContext;

/// 通过 Tauri 的 State 管理共享数据
pub struct ManagedState {
    pub state: Arc<RwLock<AppState>>,
    pub ctx: Arc<TauriContext>,
}
