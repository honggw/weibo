//! 微博 (weibo.com) PC 客户端
//!
//! Architecture (MVVM):
//!   gpui_views (ViewModel) → model (Services) → infra (HTTP, IO) → domain (Models)
//!   view/widgets (View) reads domain state for pure GPUI rendering
//!
//! Usage:
//!   cargo run                  → GPUI 图形界面 (默认)
//!   cargo run -- http          → 终端 QR 登录
//!   cargo run -- cookie        → 终端 Cookie 登录

mod cli;
mod domain;
mod gpui_views;
mod infra;
mod legacy;
#[macro_use]
mod logger;
mod model;
mod qr_login;
mod view;

use gpui::AppContext;

// ============================================================================
// GPUI 图形界面入口
// ============================================================================

fn gpui_mode() {
    // Install panic hook to capture crash info in log
    std::panic::set_hook(Box::new(|info| {
        let msg = if let Some(s) = info.payload().downcast_ref::<&str>() {
            s.to_string()
        } else if let Some(s) = info.payload().downcast_ref::<String>() {
            s.clone()
        } else {
            "unknown panic".to_string()
        };
        let location = info.location().map(|l| format!("{}:{}", l.file(), l.line())).unwrap_or_default();
        log_error!("!!! PANIC at {}: {}", location, msg);
        // Also write directly to stderr in case logger failed
        eprintln!("!!! PANIC at {}: {}", location, msg);
    }));
    let tokio_rt = Box::leak(Box::new(
        tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .expect("failed to create tokio runtime"),
    ));
    let tokio_handle = tokio_rt.handle().clone();

    log_info!("========================================");
    log_info!("微博 PC 客户端启动 (GPUI mode)");
    log_info!("日志文件: weibo_app.log");
    log_info!("========================================");

    gpui::Application::new().run(move |cx: &mut gpui::App| {
        cx.open_window(
            gpui::WindowOptions {
                window_bounds: Some(gpui::WindowBounds::Windowed(gpui::Bounds::new(
                    gpui::Point::new(gpui::px(200.0), gpui::px(100.0)),
                    gpui::Size::new(gpui::px(480.0), gpui::px(780.0)),
                ))),
                titlebar: Some(gpui::TitlebarOptions {
                    title: Some("微博 PC 客户端".into()),
                    ..Default::default()
                }),
                focus: true,
                ..Default::default()
            },
            |_window: &mut gpui::Window, cx: &mut gpui::App| {
                cx.new(|cx: &mut gpui::Context<gpui_views::AppRoot>| {
                    gpui_views::AppRoot::new(cx, tokio_handle.clone())
                })
            },
        )
        .unwrap();
    });
}

// ============================================================================
// 入口
// ============================================================================

fn main() {
    let args: Vec<String> = std::env::args().collect();

    match args.get(1).map(|s| s.as_str()) {
        Some("cookie") => {
            let rt = tokio::runtime::Runtime::new().expect("tokio runtime");
            rt.block_on(async {
                if let Err(e) = cli::cookie_login::run().await {
                    eprintln!("错误: {}", e);
                }
            });
        }
        Some("http") => {
            let rt = tokio::runtime::Runtime::new().expect("tokio runtime");
            rt.block_on(async {
                if let Err(e) = cli::qr_login::run().await {
                    eprintln!("错误: {}", e);
                }
            });
        }
        _ => gpui_mode(),
    }
}
