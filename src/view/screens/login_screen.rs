//! Login screen — QR display, loading, error states.

use gpui::*;

use crate::view::widgets::{centered_msg, qr_display};
use crate::view::theme;

pub fn render_qr(status: &str, qr_png_bytes: &Option<Vec<u8>>) -> AnyElement {
    qr_display::container(status, qr_png_bytes)
}

pub fn render_centered(text: &str, show_spinner: bool) -> AnyElement {
    centered_msg::render(text, show_spinner)
}

pub fn render_error(message: &str) -> AnyElement {
    let message = message.to_string();
    div()
        .flex().flex_col().size_full().items_center().justify_center().gap_4()
        .child(div().text_size(px(48.0)).child("❌"))
        .child(div().text_size(px(16.0)).text_color(rgb(0xff6b6b)).child(message))
        .child(div().text_size(px(13.0)).text_color(rgb(theme::CLR_MUTED)).child("请查看终端窗口了解详细错误"))
        .child(div().text_size(px(12.0)).text_color(rgb(theme::CLR_MUTED)).child("关闭窗口后重新运行 cargo run 重试"))
        .into_any_element()
}
