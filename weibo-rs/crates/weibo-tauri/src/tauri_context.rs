//! Tauri 对 VMContext 的实现
//!
//! 使用 AppHandle + emit 实现 notify,
//! 使用 tokio::spawn 实现 spawn_task 和 schedule_after.

use std::sync::Arc;
use tokio::sync::RwLock;
use tauri::{AppHandle, Emitter};

use weibo_viewmodel::app_state::AppState;
use weibo_viewmodel::context::VMContext;

/// Tauri 对 VMContext 的实现
pub struct TauriContext {
    app: AppHandle,
    state: Arc<RwLock<AppState>>,
}

impl TauriContext {
    pub fn new(app: AppHandle, state: Arc<RwLock<AppState>>) -> Self {
        Self { app, state }
    }
}

impl VMContext for TauriContext {
    type State = AppState;

    fn notify(&self) {
        // 把状态变更事件 emit 给前端
        // (不发送全量状态, 只发变更事件, 前端按需 invoke 获取详情)
        self.app.emit("state-changed", ()).ok();
    }

    fn spawn_task<F, T, C>(&self, task: F, on_done: C)
    where
        F: std::future::Future<Output = T> + Send + 'static,
        T: Send + 'static,
        C: FnOnce(&mut AppState, T) + Send + 'static,
    {
        let state = self.state.clone();
        let app = self.app.clone();
        tokio::spawn(async move {
            let result = task.await;
            let mut s = state.write().await;
            on_done(&mut s, result);
            app.emit("state-changed", ()).ok();
        });
    }

    fn schedule_after<C>(&self, millis: u64, callback: C)
    where
        C: FnOnce(&mut AppState) + Send + 'static,
    {
        let state = self.state.clone();
        let app = self.app.clone();
        tokio::spawn(async move {
            tokio::time::sleep(std::time::Duration::from_millis(millis)).await;
            let mut s = state.write().await;
            callback(&mut s);
            app.emit("state-changed", ()).ok();
        });
    }
}
