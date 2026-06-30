//! 认证相关 Tauri IPC 命令

use tauri::State;
use weibo_viewmodel::login_vm;
use weibo_viewmodel::home_vm;
use weibo_domain::LoginPhase;
use base64::Engine;

use super::ManagedState;
use crate::events::StateSnapshot;
use serde::Serialize;

/// 前端调用: 检查保存的 Cookie
#[tauri::command]
pub async fn check_saved_cookie(
    managed: State<'_, ManagedState>,
) -> Result<(), String> {
    let state = managed.state.read().await;
    home_vm::check_cookie(&*managed.ctx, &state);
    Ok(())
}

/// 前端调用: 启动扫码登录
#[tauri::command]
pub async fn start_qr_login(
    managed: State<'_, ManagedState>,
) -> Result<(), String> {
    login_vm::start_login_flow(&*managed.ctx);
    Ok(())
}

/// 前端调用: 获取当前状态快照 (前端初始化/同步用)
#[tauri::command]
pub async fn get_state(
    managed: State<'_, ManagedState>,
) -> Result<StateSnapshot, String> {
    let state = managed.state.read().await;
    Ok(StateSnapshot::from(&*state))
}

/// 前端调用: 登出
#[tauri::command]
pub async fn logout(
    managed: State<'_, ManagedState>,
) -> Result<(), String> {
    home_vm::logout(&*managed.ctx);
    Ok(())
}

/// 前端调用: 获取 QR 码图片 (base64 编码)
#[tauri::command]
pub async fn get_qr_image(
    managed: State<'_, ManagedState>,
) -> Result<QrImageResponse, String> {
    let state = managed.state.read().await;
    match &state.phase {
        LoginPhase::WaitingScan { qr_png_bytes, status, .. } => {
            let b64 = qr_png_bytes.as_ref()
                .map(|bytes| base64::engine::general_purpose::STANDARD.encode(bytes));
            Ok(QrImageResponse {
                has_qr: b64.is_some(),
                qr_base64: b64,
                status: status.clone(),
            })
        }
        _ => Ok(QrImageResponse {
            has_qr: false,
            qr_base64: None,
            status: "QR 码不可用".into(),
        }),
    }
}

/// 前端调用: 单次 QR 轮询
#[tauri::command]
pub async fn poll_qr_once(
    managed: State<'_, ManagedState>,
) -> Result<(), String> {
    let state = managed.state.read().await;
    login_vm::poll_qr_once(&*managed.ctx, &state);
    Ok(())
}

/// 前端调用: QR 确认后交换票据 + 加载首页
#[tauri::command]
pub async fn confirm_and_proceed(
    managed: State<'_, ManagedState>,
) -> Result<(), String> {
    let state = managed.state.read().await;
    login_vm::confirm_and_proceed(&*managed.ctx, &state);
    Ok(())
}

/// 前端调用: 刷新过期 QR 码
#[tauri::command]
pub async fn refresh_qr(
    managed: State<'_, ManagedState>,
) -> Result<(), String> {
    let state = managed.state.read().await;
    login_vm::refresh_qr(&*managed.ctx, &state);
    Ok(())
}

/// 前端调用: 加载首页 (Cookie 验证成功后)
#[tauri::command]
pub async fn fetch_home(
    managed: State<'_, ManagedState>,
) -> Result<(), String> {
    home_vm::fetch_home(&*managed.ctx);
    Ok(())
}

#[derive(Serialize)]
pub struct QrImageResponse {
    pub has_qr: bool,
    pub qr_base64: Option<String>,
    pub status: String,
}
