//! 时间线相关 Tauri IPC 命令

use tauri::State;
use weibo_viewmodel::home_vm;

use super::ManagedState;

/// 前端调用: 加载更多时间线
#[tauri::command]
pub async fn load_more_timeline(
    managed: State<'_, ManagedState>,
) -> Result<(), String> {
    let state = managed.state.read().await;
    home_vm::load_more_timeline(&*managed.ctx, &state);
    Ok(())
}
