//! 聊天相关 Tauri IPC 命令

use tauri::State;
use weibo_viewmodel::chat_vm;

use super::ManagedState;

/// 前端调用: 加载联系人列表
#[tauri::command]
pub async fn load_contacts(
    managed: State<'_, ManagedState>,
) -> Result<(), String> {
    chat_vm::load_contacts(&*managed.ctx);
    Ok(())
}

/// 前端调用: 选中联系人, 加载消息历史
#[tauri::command]
pub async fn select_contact(
    managed: State<'_, ManagedState>,
    uid: String,
    is_group: bool,
) -> Result<(), String> {
    let state = managed.state.read().await;
    chat_vm::select_contact(&*managed.ctx, &state, uid, is_group);
    Ok(())
}

/// 前端调用: 发送消息
#[tauri::command]
pub async fn send_message(
    managed: State<'_, ManagedState>,
    uid: String,
    text: String,
    is_group: bool,
) -> Result<(), String> {
    chat_vm::send_message(&*managed.ctx, uid, text, is_group);
    Ok(())
}

/// 前端调用: 加载更早消息
#[tauri::command]
pub async fn load_older_messages(
    managed: State<'_, ManagedState>,
) -> Result<(), String> {
    let state = managed.state.read().await;
    chat_vm::load_older_messages(&*managed.ctx, &state);
    Ok(())
}
