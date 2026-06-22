//! Home ViewModel — cookie verification + timeline loading orchestration.
//! Fast path when saved cookies exist: verify → fetch home directly.

use gpui::*;
use std::time::Duration;

use crate::domain::LoginPhase;
use crate::model::{auth_service, timeline_service};
use crate::{log_error, log_info, log_success};

use super::login_vm;
use super::root_vm::AppRoot;

/// Spawn cookie verification flow. Falls back to QR login if expired.
pub fn start_cookie_flow(
    cx: &Context<AppRoot>,
    tokio_handle: &tokio::runtime::Handle,
    cookie: String,
) {
    let handle = tokio_handle.clone();
    cx.spawn(move |this: WeakEntity<AppRoot>, cx: &mut AsyncApp| {
        let mut cx = cx.clone();
        async move {
            // --- Verify cookie ---
            let valid = handle.block_on(auth_service::verify_cookie(&cookie)).unwrap_or(false);
            if !valid {
                log_info!("Cookie 已过期, 回退扫码登录");
                this.update(&mut cx, |v, cx| {
                    v.phase = LoginPhase::Loading("Cookie 已过期, 重新连接...".into());
                    cx.notify();
                    login_vm::start_login_flow(cx, &handle);
                }).ok();
                return;
            }
            log_success!("Cookie 有效, 加载首页");

            // --- Fetch home ---
            this.update(&mut cx, |v, cx| {
                v.phase = LoginPhase::FetchingHome;
                cx.notify();
            }).ok();
            Timer::after(Duration::from_millis(100)).await;

            let (items, title) = handle.block_on(timeline_service::fetch_home_content(&cookie));
            log_info!("[cookie] fetch_home_content 完成: {} items", items.len());

            match this.update(&mut cx, |v, cx| {
                v.phase = LoginPhase::HomeLoaded { items, title };
                cx.notify();
            }) {
                Ok(_) => log_info!("[cookie] HomeLoaded 已设置 ✅"),
                Err(e) => log_error!("[cookie] 设置 HomeLoaded 失败: {}", e),
            }
        }
    }).detach();
}
