//! ViewModel — state management and flow orchestration.
//!
//! AppRoot holds UI state (LoginPhase) and coordinates:
//!   - model::auth_service for login/cookie operations
//!   - model::timeline_service for content fetching
//!   - view::widgets for pure GPUI rendering
//!
//! Rendering is delegated to view::widgets (stateless functions).

use gpui::*;
use std::time::Duration;

use crate::domain::LoginPhase;
use crate::infra::cookie_io;
use crate::model::{auth_service, timeline_service};
use crate::view::widgets;
use crate::{log_error, log_info, log_success};

// ============================================================================
// AppRoot — ViewModel
// ============================================================================

pub struct AppRoot {
    phase: LoginPhase,
    tokio_handle: tokio::runtime::Handle,
}

impl AppRoot {
    pub fn new(cx: &mut Context<Self>, tokio_handle: tokio::runtime::Handle) -> Self {
        let this = Self {
            phase: LoginPhase::CheckingCookie,
            tokio_handle: tokio_handle.clone(),
        };

        if let Some(cookie) = auth_service::load_saved_cookie() {
            log_info!("发现已保存的 Cookie, 尝试验证...");
            this.start_cookie_flow(cx, cookie);
        } else {
            log_info!("未发现 Cookie, 进入扫码登录");
            this.start_login_flow(cx);
        }

        this
    }

    // ========================================================================
    // Cookie flow
    // ========================================================================

    fn start_cookie_flow(&self, cx: &Context<Self>, cookie: String) {
        let handle = self.tokio_handle.clone();
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
                        v.start_login_flow(cx);
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

    // ========================================================================
    // QR login flow
    // ========================================================================

    fn start_login_flow(&self, cx: &Context<Self>) {
        let handle = self.tokio_handle.clone();
        cx.spawn(|this: WeakEntity<AppRoot>, cx: &mut AsyncApp| {
            let mut cx = cx.clone();
            async move {
                // --- Prepare QR ---
                let (mut login, png_bytes) = match handle.block_on(auth_service::prepare_qr()) {
                    Ok(v) => v,
                    Err(e) => {
                        log_error!("QR 准备失败: {}", e);
                        this.update(&mut cx, |v, cx| {
                            v.phase = LoginPhase::Error(format!("连接失败: {}", e));
                            cx.notify();
                        }).ok();
                        return;
                    }
                };

                this.update(&mut cx, |v, cx| {
                    v.phase = LoginPhase::WaitingScan {
                        status: "📱 请用微博手机客户端扫描二维码".into(),
                        qr_png_bytes: Some(png_bytes),
                    };
                    cx.notify();
                }).ok();
                Timer::after(Duration::from_millis(100)).await;

                // --- Poll loop ---
                let cookie = loop {
                    let status = handle.block_on(auth_service::poll_qr(&login));
                    match status {
                        Ok(crate::qr_login::QrStatus::Waiting) => {
                            this.update(&mut cx, |v, cx| {
                                if let LoginPhase::WaitingScan { ref mut status, .. } = v.phase {
                                    *status = "📱 等待扫码...".into();
                                }
                                cx.notify();
                            }).ok();
                        }
                        Ok(crate::qr_login::QrStatus::Scanned) => {
                            this.update(&mut cx, |v, cx| {
                                let qr = match &v.phase {
                                    LoginPhase::WaitingScan { qr_png_bytes, .. } => qr_png_bytes.clone(),
                                    _ => None,
                                };
                                v.phase = LoginPhase::WaitingScan {
                                    status: "📲 已扫描！请在手机上点击「确认登录」".into(),
                                    qr_png_bytes: qr,
                                };
                                cx.notify();
                            }).ok();
                        }
                        Ok(crate::qr_login::QrStatus::Confirmed { alt, redirect_url }) => {
                            log_info!("[login] 扫码确认! alt={}, redirect={}", &alt[..alt.len().min(40)], &redirect_url[..redirect_url.len().min(60)]);
                            this.update(&mut cx, |v, cx| {
                                v.phase = LoginPhase::Exchanging("✅ 确认成功！获取票据...".into());
                                cx.notify();
                            }).ok();
                            Timer::after(Duration::from_millis(100)).await;

                            log_info!("[login] 开始 exchange_ticket...");
                            match handle.block_on(
                                auth_service::exchange_ticket(&mut login, &alt, &redirect_url)
                            ) {
                                Ok(c) => {
                                    log_info!("[login] exchange_ticket 成功, cookie={}...", &c[..c.len().min(30)]);
                                    break c;
                                }
                                Err(e) => {
                                    log_error!("票据交换失败: {}", e);
                                    this.update(&mut cx, |v, cx| {
                                        v.phase = LoginPhase::Error(format!("登录失败: {}", e));
                                        cx.notify();
                                    }).ok();
                                    return;
                                }
                            }
                        }
                        Ok(crate::qr_login::QrStatus::Expired) => {
                            this.update(&mut cx, |v, cx| {
                                v.phase = LoginPhase::Loading("⚠️ 二维码过期, 刷新...".into());
                                cx.notify();
                            }).ok();
                            match handle.block_on(auth_service::refresh_qr(&mut login)) {
                                Ok(png) => {
                                    this.update(&mut cx, |v, cx| {
                                        v.phase = LoginPhase::WaitingScan {
                                            status: "📱 请用微博手机客户端扫描二维码".into(),
                                            qr_png_bytes: Some(png),
                                        };
                                        cx.notify();
                                    }).ok();
                                }
                                Err(e) => log_error!("QR 刷新失败: {}", e),
                            }
                        }
                        Ok(other) => log_info!("QR poll: {:?}", other),
                        Err(e) => log_error!("QR poll error: {}", e),
                    }
                    Timer::after(Duration::from_secs(1)).await;
                };

                // --- Fetch home ---
                log_info!("[login] 登录完成, 开始加载首页...");
                match this.update(&mut cx, |v, cx| {
                    v.phase = LoginPhase::FetchingHome;
                    cx.notify();
                }) {
                    Ok(_) => log_info!("[login] FetchingHome 状态已设置"),
                    Err(e) => log_error!("[login] 设置 FetchingHome 失败: {}", e),
                }
                Timer::after(Duration::from_millis(100)).await;

                log_info!("[login] 调用 fetch_home_content (block_on)...");
                let (items, title) = handle.block_on(timeline_service::fetch_home_content(&cookie));
                log_info!("[login] fetch_home_content 完成: {} items, title={}", items.len(), title);

                match this.update(&mut cx, |v, cx| {
                    v.phase = LoginPhase::HomeLoaded { items, title };
                    cx.notify();
                }) {
                    Ok(_) => log_info!("[login] HomeLoaded 状态已设置 ✅"),
                    Err(e) => log_error!("[login] 设置 HomeLoaded 失败: {}", e),
                }
            }
        }).detach();
    }

    // ========================================================================
    // Logout
    // ========================================================================

    fn logout(&mut self, cx: &mut Context<Self>) {
        log_info!("用户点击登出");
        cookie_io::delete();
        self.phase = LoginPhase::Loading("正在登出...".into());
        cx.notify();
        self.start_login_flow(cx);
    }
}

// ============================================================================
// Render — delegates to view::widgets
// ============================================================================

impl Render for AppRoot {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let is_logged_in = matches!(self.phase, LoginPhase::HomeLoaded { .. });

        div()
            .flex().flex_col().size_full()
            .bg(rgb(widgets::CLR_BG))
            .text_color(rgb(widgets::CLR_TEXT))
            .font_family("Microsoft YaHei, sans-serif")
            .child(widgets::header_bar(is_logged_in, cx.listener(
                |this, _: &ClickEvent, _window, cx| this.logout(cx),
            )))
            .child(widgets::body(&self.phase))
    }
}
