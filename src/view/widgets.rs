//! Reusable, stateless GPUI rendering widgets.
//! Each function takes domain data as input and returns `impl IntoElement` or `AnyElement`.

use gpui::*;
use gpui::prelude::*;
use std::sync::Arc;

use crate::domain::{LoginPhase, TimelineItem};

// ============================================================================
// Theme
// ============================================================================

pub const CLR_BG: u32 = 0x1a1a2e;
pub const CLR_CARD: u32 = 0x16213e;
pub const CLR_ACCENT: u32 = 0xe8633a;
pub const CLR_TEXT: u32 = 0xe8e8e8;
pub const CLR_MUTED: u32 = 0x888888;
pub const CLR_HEADER: u32 = 0x0f3460;
pub const CLR_BTN: u32 = 0x333355;

// ============================================================================
// Header bar
// ============================================================================

pub fn header_bar<F>(is_logged_in: bool, on_logout: F) -> impl IntoElement
where
    F: Fn(&ClickEvent, &mut Window, &mut App) + 'static,
{
    div()
        .flex()
        .flex_row()
        .items_center()
        .justify_between()
        .px_4()
        .py_3()
        .bg(rgb(CLR_HEADER))
        .border_b_1()
        .border_color(rgb(CLR_ACCENT))
        .child(
            div()
                .flex().flex_row().items_center().gap_2()
                .child(div().text_size(px(20.0)).font_weight(FontWeight::BOLD).text_color(rgb(CLR_ACCENT)).child("微博"))
                .child(div().text_size(px(12.0)).text_color(rgb(CLR_MUTED)).child("PC 客户端")),
        )
        .when(is_logged_in, |this| {
            this.child(
                div()
                    .id("logout-btn")
                    .px_3().py_1().rounded_full().bg(rgb(CLR_BTN)).cursor_pointer()
                    .text_size(px(13.0)).text_color(rgb(CLR_MUTED))
                    .child("登出")
                    .on_click(on_logout),
            )
        })
}

// ============================================================================
// Body dispatcher
// ============================================================================

pub fn body(phase: &LoginPhase) -> AnyElement {
    match phase {
        LoginPhase::CheckingCookie => centered_msg("正在检查登录状态...", true),
        LoginPhase::Loading(msg) => centered_msg(msg, true),
        LoginPhase::WaitingScan { status, qr_png_bytes } => qr_screen(status, qr_png_bytes),
        LoginPhase::Exchanging(msg) => centered_msg(msg, true),
        LoginPhase::FetchingHome => centered_msg("正在获取首页...", true),
        LoginPhase::HomeLoaded { items, title } => timeline(title, items),
        LoginPhase::Error(msg) => error_screen(msg),
    }
}

// ============================================================================
// Centered message + spinner
// ============================================================================

pub fn centered_msg(text: &str, show_spinner: bool) -> AnyElement {
    let text = text.to_string();
    div()
        .flex().flex_col().size_full().items_center().justify_center().gap_4()
        .child(div().text_size(px(16.0)).text_color(rgb(CLR_TEXT)).child(text))
        .child(if show_spinner {
            div().text_size(px(32.0)).text_color(rgb(CLR_ACCENT)).child("⏳").into_any_element()
        } else {
            div().into_any_element()
        })
        .into_any_element()
}

// ============================================================================
// QR code screen
// ============================================================================

pub fn qr_screen(status: &str, qr_bytes: &Option<Vec<u8>>) -> AnyElement {
    let status = status.to_string();
    div()
        .flex().flex_col().size_full().items_center().justify_center().gap_4().px_4()
        .child(
            div()
                .w(px(220.0)).h(px(220.0)).bg(rgb(0xffffff)).rounded_lg()
                .border_1().border_color(rgb(0x333366))
                .flex().items_center().justify_center()
                .child(if let Some(bytes) = qr_bytes {
                    let image = Image::from_bytes(ImageFormat::Png, bytes.clone());
                    div().w(px(200.0)).h(px(200.0)).bg(rgb(0xffffff))
                        .child(img(ImageSource::Image(Arc::new(image))).object_fit(ObjectFit::Contain))
                        .into_any_element()
                } else {
                    div().text_size(px(14.0)).text_color(rgb(0x000000)).child("加载中...").into_any_element()
                }),
        )
        .child(div().text_size(px(16.0)).text_color(rgb(CLR_TEXT)).text_align(TextAlign::Center).child(status))
        .child(div().text_size(px(13.0)).text_color(rgb(CLR_MUTED)).child("二维码仅限微博手机客户端扫描"))
        .child(div().text_size(px(13.0)).text_color(rgb(CLR_MUTED)).child("打开微博 App → 扫一扫 → 确认登录"))
        .into_any_element()
}

// ============================================================================
// Timeline list
// ============================================================================

pub fn timeline(title: &str, items: &[TimelineItem]) -> AnyElement {
    div()
        .flex().flex_col().size_full().px_3().py_3().gap_3()
        .child(
            div().px_2().py_2().child(
                div().text_size(px(18.0)).font_weight(FontWeight::BOLD).text_color(rgb(CLR_ACCENT)).child(title.to_string()),
            ),
        )
        .children(items.iter().map(|item| timeline_card(item)))
        .into_any_element()
}

fn timeline_card(item: &TimelineItem) -> AnyElement {
    div()
        .flex().flex_col().bg(rgb(CLR_CARD)).rounded_lg().px_4().py_3().gap_1()
        .child(div().text_size(px(14.0)).font_weight(FontWeight::BOLD).text_color(rgb(CLR_ACCENT)).child(item.user_name.clone()))
        .child(div().text_size(px(13.0)).text_color(rgb(CLR_TEXT)).line_height(relative(1.6)).child(item.text.clone()))
        .into_any_element()
}

// ============================================================================
// Error screen
// ============================================================================

pub fn error_screen(message: &str) -> AnyElement {
    let message = message.to_string();
    div()
        .flex().flex_col().size_full().items_center().justify_center().gap_4()
        .child(div().text_size(px(48.0)).child("❌"))
        .child(div().text_size(px(16.0)).text_color(rgb(0xff6b6b)).child(message))
        .child(div().text_size(px(13.0)).text_color(rgb(CLR_MUTED)).child("请查看终端窗口了解详细错误"))
        .child(div().text_size(px(12.0)).text_color(rgb(CLR_MUTED)).child("关闭窗口后重新运行 cargo run 重试"))
        .into_any_element()
}
