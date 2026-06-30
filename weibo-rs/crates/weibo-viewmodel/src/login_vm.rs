//! Login ViewModel — QR code login flow orchestration.
//! Coordinates model::auth_service for each phase, updates AppState via VMContext.
//!
//! Full flow:
//!   start_login_flow → (prepare QR) → WaitingScan → frontend calls poll_qr_once →
//!   → poll result → (waiting → frontend polls again) / (scanned → frontend polls again) /
//!   → (confirmed → confirm_and_proceed → exchange → fetch home → HomeLoaded) /
//!   → (expired → refresh_qr → WaitingScan)
//!
//! 注意: 所有函数都被 frontend/Tauri command 主动调用;
//! 使用泛型 `<C: VMContext>` 而非 `dyn VMContext` 因为 trait 方法有泛型参数。

use weibo_domain::{LoginPhase, QrStatus, TimelineItem};
use weibo_model::auth_service;
use weibo_infra::{log_info, log_error};

use crate::app_state::{AppState, QrSession};
use crate::context::VMContext;

/// 启动 QR 扫码登录: prepare → 显示 QR → 通知前端开始轮询
pub fn start_login_flow<C: VMContext<State = AppState>>(ctx: &C) {
    ctx.spawn_task(
        async { auth_service::prepare_qr().await },
        |state, result| match result {
            Ok((login, png_bytes)) => {
                state.phase = LoginPhase::WaitingScan {
                    status: "📱 请用微博手机客户端扫描二维码".into(),
                    qr_png_bytes: Some(png_bytes),
                };
                state.qr_session = Some(QrSession {
                    login,
                    polling: true,
                    alt: None,
                    redirect_url: None,
                });
            }
            Err(e) => {
                state.phase = LoginPhase::Error(format!("连接失败: {}", e));
            }
        },
    );
}

/// 单次 QR 轮询: 由前端周期性调用 (每秒一次)
/// 需要传入当前 state 以读取 login 实例
pub fn poll_qr_once<C: VMContext<State = AppState>>(ctx: &C, state: &AppState) {
    // 从 state 中提取 login 实例 (clone 用于 async + callback)
    let qr_session = match &state.qr_session {
        Some(s) if s.polling => s,
        _ => return,
    };

    let login_for_async = qr_session.login.clone();
    let login_for_cb = login_for_async.clone();

    ctx.spawn_task(
        async move { auth_service::poll_qr(&login_for_async).await },
        move |state, result| {
            match result {
                Ok(QrStatus::Confirmed { alt, redirect_url }) => {
                    state.phase = LoginPhase::Exchanging("✅ 确认成功！获取票据...".into());
                    // Store confirmation data in session for confirm_and_proceed
                    state.qr_session = Some(QrSession {
                        login: login_for_cb,
                        polling: false,
                        alt: Some(alt),
                        redirect_url: Some(redirect_url),
                    });
                }
                Ok(QrStatus::Scanned) => {
                    let qr_png = match &state.phase {
                        LoginPhase::WaitingScan { qr_png_bytes, .. } => qr_png_bytes.clone(),
                        _ => None,
                    };
                    state.phase = LoginPhase::WaitingScan {
                        status: "📲 已扫描！请在手机上点击「确认登录」".into(),
                        qr_png_bytes: qr_png,
                    };
                    state.qr_session = Some(QrSession {
                        login: login_for_cb,
                        polling: true,
                        alt: None,
                        redirect_url: None,
                    });
                }
                Ok(QrStatus::Waiting) => {
                    if let LoginPhase::WaitingScan { ref mut status, .. } = state.phase {
                        *status = "📱 等待扫码...".into();
                    }
                    state.qr_session = Some(QrSession {
                        login: login_for_cb,
                        polling: true,
                        alt: None,
                        redirect_url: None,
                    });
                }
                Ok(QrStatus::Expired) => {
                    state.phase = LoginPhase::Loading("⚠️ 二维码过期, 刷新...".into());
                    // Keep login for refresh_qr
                    state.qr_session = Some(QrSession {
                        login: login_for_cb,
                        polling: false,
                        alt: None,
                        redirect_url: None,
                    });
                }
                Ok(other) => {
                    log_info!("QR poll: {:?}", other);
                    state.qr_session = Some(QrSession {
                        login: login_for_cb,
                        polling: true,
                        alt: None,
                        redirect_url: None,
                    });
                }
                Err(e) => {
                    log_error!("QR poll error: {}", e);
                    state.qr_session = Some(QrSession {
                        login: login_for_cb,
                        polling: true,
                        alt: None,
                        redirect_url: None,
                    });
                }
            }
        },
    );
}

/// 确认后自动交换票据 + 加载首页 (一个 spawn_task 完成后续所有流程)
/// 由前端在收到 Exchanging 状态后调用
pub fn confirm_and_proceed<C: VMContext<State = AppState>>(ctx: &C, state: &AppState) {
    let (mut login, alt, redirect_url) = match &state.qr_session {
        Some(QrSession { login, alt: Some(a), redirect_url: Some(r), .. }) => {
            (login.clone(), a.clone(), r.clone())
        }
        _ => return,
    };

    ctx.spawn_task(
        async move {
            let cookie = auth_service::exchange_ticket(&mut login, &alt, &redirect_url).await?;
            let (items, title, feed_list_id, since_id) =
                weibo_model::timeline_service::fetch_first_page().await;
            Ok((cookie, items, title, feed_list_id, since_id))
        },
        |state, result: Result<(String, Vec<TimelineItem>, String, Option<String>, String), anyhow::Error>| {
            match result {
                Ok((_cookie, items, title, feed_list_id, since_id)) => {
                    state.timeline.items = items;
                    state.timeline.title = title;
                    state.timeline.feed_list_id = feed_list_id;
                    state.timeline.since_id = since_id;
                    state.phase = LoginPhase::HomeLoaded {
                        items: state.timeline.items.clone(),
                        title: state.timeline.title.clone(),
                    };
                    state.qr_session = None;
                }
                Err(e) => {
                    state.phase = LoginPhase::Error(format!("登录失败: {}", e));
                    state.qr_session = None;
                }
            }
        },
    );
}

/// 刷新过期的 QR 码
pub fn refresh_qr<C: VMContext<State = AppState>>(ctx: &C, state: &AppState) {
    let login = match &state.qr_session {
        Some(s) => s.login.clone(),
        None => return,
    };
    let mut login_for_async = login.clone();

    ctx.spawn_task(
        async move { auth_service::refresh_qr(&mut login_for_async).await },
        move |state, result| match result {
            Ok(png) => {
                state.phase = LoginPhase::WaitingScan {
                    status: "📱 请用微博手机客户端扫描二维码".into(),
                    qr_png_bytes: Some(png),
                };
                state.qr_session = Some(QrSession {
                    login,
                    polling: true,
                    alt: None,
                    redirect_url: None,
                });
            }
            Err(e) => {
                log_info!("QR 刷新失败: {}", e);
                state.phase = LoginPhase::Error(format!("QR 刷新失败: {}", e));
            }
        },
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::context::MockContext;

    /// Verify that MockContext satisfies VMContext trait bounds
    #[test]
    fn test_mock_context_trait_satisfied() {
        let ctx = MockContext::new(AppState::new());
        ctx.notify();
        assert_eq!(ctx.notified_count(), 1);
    }

    /// Verify AppState initial state
    #[test]
    fn test_app_state_default() {
        let state = AppState::new();
        assert!(matches!(state.phase, LoginPhase::CheckingCookie));
        assert!(matches!(state.active_tab, weibo_domain::ActiveTab::Home));
    }
}
