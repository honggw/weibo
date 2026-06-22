//! Application shell — GPUI window creation and bootstrap.

use gpui::*;

use crate::viewmodel::root_vm::AppRoot;
use crate::log_info;

/// Start the GPUI application with a window showing the Weibo client.
pub fn run(tokio_handle: tokio::runtime::Handle) {
    log_info!("========================================");
    log_info!("微博 PC 客户端启动 (GPUI mode)");
    log_info!("日志文件: weibo_app.log");
    log_info!("========================================");

    Application::new().run(move |cx: &mut App| {
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(Bounds::new(
                    Point::new(px(200.0), px(100.0)),
                    Size::new(px(480.0), px(780.0)),
                ))),
                titlebar: Some(TitlebarOptions {
                    title: Some("微博 PC 客户端".into()),
                    ..Default::default()
                }),
                focus: true,
                ..Default::default()
            },
            |_window: &mut Window, cx: &mut App| {
                cx.new(|cx: &mut Context<AppRoot>| {
                    AppRoot::new(cx, tokio_handle.clone())
                })
            },
        )
        .unwrap();
    });
}
