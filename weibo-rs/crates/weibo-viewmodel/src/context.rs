//! VMContext trait — ViewModel 对外界执行环境的唯一抽象。
//!
//! 职责:
//!   1. 通知 UI 层状态变更 (替代 gpui::cx.notify())
//!   2. 调度异步任务并在完成时回写状态 (替代 gpui::cx.spawn + WeakEntity)
//!   3. 延时等待 (替代 gpui::Timer::after)
//!
//! 不同 UI 框架提供各自的实现:
//!   - Tauri: AppHandle + emit
//!   - 测试: MockContext (同步执行，方便断言)
//!
//! 设计: 关联类型版。不使用 dyn trait object (因为泛型方法不 object-safe)，
//! 所有 VM 函数通过泛型 `<C: VMContext>` 参数化。

use std::future::Future;

pub trait VMContext: Send + Sync + 'static {
    type State: Send + 'static;

    /// 通知 UI 层: 状态已变更，需要刷新渲染。
    ///
    /// Tauri 实现: app.emit("state-changed", payload)
    fn notify(&self);

    /// 调度一个异步任务。
    ///
    /// 任务在后台执行，完成后通过 `on_done` 回调更新 ViewModel 状态。
    ///
    /// 类型约束:
    ///   - F: 异步操作本身 (如网络请求)
    ///   - T: 异步操作的返回值
    ///   - C: 状态更新回调
    fn spawn_task<F, T, C>(&self, task: F, on_done: C)
    where
        F: Future<Output = T> + Send + 'static,
        T: Send + 'static,
        C: FnOnce(&mut Self::State, T) + Send + 'static;

    /// 调度一个延时后执行的回调。
    ///
    /// 用于: QR 轮询间隔、加载动画延迟等。
    /// 回调中仅修改状态并调用 notify，如需继续调度，由上层 (Tauri command) 在收到
    /// state-changed 事件后再次调用 ViewModel 函数。
    fn schedule_after<C>(&self, millis: u64, callback: C)
    where
        C: FnOnce(&mut Self::State) + Send + 'static;
}

// ============================================================================
// MockContext — 用于 ViewModel 单元测试
// ============================================================================

#[cfg(test)]
use std::sync::{Arc, Mutex};
#[cfg(test)]
use std::sync::atomic::{AtomicU32, Ordering};

#[cfg(test)]
pub struct MockContext {
    pub state: Arc<Mutex<crate::app_state::AppState>>,
    pub notified: AtomicU32,
}

#[cfg(test)]
impl MockContext {
    pub fn new(state: crate::app_state::AppState) -> Self {
        Self {
            state: Arc::new(Mutex::new(state)),
            notified: AtomicU32::new(0),
        }
    }

    pub fn state(&self) -> std::sync::MutexGuard<'_, crate::app_state::AppState> {
        self.state.lock().unwrap()
    }

    pub fn notified_count(&self) -> u32 {
        self.notified.load(Ordering::Relaxed)
    }
}

#[cfg(test)]
impl VMContext for MockContext {
    type State = crate::app_state::AppState;

    fn notify(&self) {
        self.notified.fetch_add(1, Ordering::Relaxed);
    }

    fn spawn_task<F, T, C>(&self, task: F, on_done: C)
    where
        F: Future<Output = T> + Send + 'static,
        T: Send + 'static,
        C: FnOnce(&mut crate::app_state::AppState, T) + Send + 'static,
    {
        let rt = tokio::runtime::Runtime::new().unwrap();
        let result = rt.block_on(task);
        let mut state = self.state.lock().unwrap();
        on_done(&mut state, result);
        drop(state);
        self.notify();
    }

    fn schedule_after<C>(&self, _millis: u64, callback: C)
    where
        C: FnOnce(&mut crate::app_state::AppState) + Send + 'static,
    {
        let mut state = self.state.lock().unwrap();
        callback(&mut state);
        drop(state);
        self.notify();
    }
}
