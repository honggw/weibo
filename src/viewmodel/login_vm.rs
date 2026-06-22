//! Login ViewModel — QR code login flow orchestration.
//! Coordinates model::auth_service for each phase, updates AppRoot state.

use gpui::*;
use std::time::Duration;

use crate::domain::LoginPhase;
use crate::model::{auth_service, timeline_service};
use crate::qr_login::QrStatus;
use crate::{log_error, log_info};

use super::root_vm::AppRoot;

/// Spawn the full QR login flow (warmup → QR → poll → exchange → timeline).
pub fn start_login_flow(cx: &Context<AppRoot>, tokio_handle: &tokio::runtime::Handle) {
    let handle = tokio_handle.clone();
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
                    Ok(QrStatus::Waiting) => {
                        this.update(&mut cx, |v, cx| {
                            if let LoginPhase::WaitingScan { ref mut status, .. } = v.phase {
                                *status = "📱 等待扫码...".into();
                            }
                            cx.notify();
                        }).ok();
                    }
                    Ok(QrStatus::Scanned) => {
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
                    Ok(QrStatus::Confirmed { alt, redirect_url }) => {
                        log_info!("[login] 扫码确认! alt={}", &alt[..alt.len().min(40)]);
                        this.update(&mut cx, |v, cx| {
                            v.phase = LoginPhase::Exchanging("✅ 确认成功！获取票据...".into());
                            cx.notify();
                        }).ok();
                        Timer::after(Duration::from_millis(100)).await;

                        match handle.block_on(auth_service::exchange_ticket(&mut login, &alt, &redirect_url)) {
                            Ok(c) => {
                                log_info!("[login] exchange_ticket 成功");
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
                    Ok(QrStatus::Expired) => {
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
            log_info!("[login] 登录完成, 加载首页...");
            this.update(&mut cx, |v, cx| {
                v.phase = LoginPhase::FetchingHome;
                cx.notify();
            }).ok();
            Timer::after(Duration::from_millis(100)).await;

            let (items, title) = handle.block_on(timeline_service::fetch_home_content(&cookie));
            log_info!("[login] fetch_home_content 完成: {} items", items.len());

            this.update(&mut cx, |v, cx| {
                v.phase = LoginPhase::HomeLoaded { items, title };
                cx.notify();
            }).ok();
        }
    }).detach();
}
