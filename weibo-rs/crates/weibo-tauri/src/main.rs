//! weibo-tauri — 微博 PC 客户端 Tauri 入口
//!
//! Architecture: Tauri (UI shell) → ViewModel (pure logic) → Model (services) → Infra (HTTP/WS)

mod commands;
mod events;
mod tauri_context;

use std::sync::Arc;
use tokio::sync::RwLock;
use tauri::Manager;
use weibo_viewmodel::app_state::AppState;
use tauri_context::TauriContext;
use commands::ManagedState;

fn main() {
    // Initialize rustls crypto provider (required for WebSocket TLS)
    let _ = rustls::crypto::CryptoProvider::install_default(
        rustls::crypto::aws_lc_rs::default_provider()
    );

    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .setup(|app| {
            // Initialize data directory for runtime files (logs, QR, cookies)
            let data_dir = app
                .path()
                .app_data_dir()
                .expect("无法获取应用数据目录");
            let _ = std::fs::create_dir_all(&data_dir);
            weibo_infra::config::init_data_dir(data_dir);

            let state = Arc::new(RwLock::new(AppState::new()));
            let ctx = Arc::new(TauriContext::new(app.handle().clone(), state.clone()));
            app.manage(ManagedState { state, ctx });

            // 应用启动后自动检查 Cookie (使用 Tauri 的 async runtime)
            let managed = app.state::<ManagedState>();
            let state_clone = managed.state.clone();
            let ctx_clone = managed.ctx.clone();
            tauri::async_runtime::spawn(async move {
                let state_guard = state_clone.read().await;
                weibo_viewmodel::home_vm::check_cookie(&*ctx_clone, &state_guard);
            });

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::auth::check_saved_cookie,
            commands::auth::start_qr_login,
            commands::auth::get_state,
            commands::auth::logout,
            commands::auth::get_qr_image,
            commands::auth::poll_qr_once,
            commands::auth::confirm_and_proceed,
            commands::auth::refresh_qr,
            commands::auth::fetch_home,
            commands::chat::load_contacts,
            commands::chat::select_contact,
            commands::chat::send_message,
            commands::chat::load_older_messages,
            commands::timeline::load_more_timeline,
        ])
        .run(tauri::generate_context!())
        .expect("启动 Tauri 应用失败");
}
