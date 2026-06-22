//! QR code display widget — renders PNG bytes as an image.

use gpui::*;
use gpui::prelude::*;
use std::sync::Arc;

use crate::view::theme;

pub fn render(png_bytes: &Option<Vec<u8>>) -> impl IntoElement {
    if let Some(bytes) = png_bytes {
        let image = Image::from_bytes(ImageFormat::Png, bytes.clone());
        div().w(px(200.0)).h(px(200.0)).bg(rgb(0xffffff))
            .child(img(ImageSource::Image(Arc::new(image))).object_fit(ObjectFit::Contain))
    } else {
        div().w(px(200.0)).h(px(200.0)).bg(rgb(0xffffff))
            .flex().items_center().justify_center()
            .child(div().text_size(px(14.0)).text_color(rgb(0x000000)).child("加载中..."))
    }
}

pub fn container(status: &str, png_bytes: &Option<Vec<u8>>) -> AnyElement {
    let status = status.to_string();
    div()
        .flex().flex_col().size_full().items_center().justify_center().gap_4().px_4()
        .child(
            div().w(px(220.0)).h(px(220.0)).bg(rgb(0xffffff)).rounded_lg()
                .border_1().border_color(rgb(0x333366))
                .flex().items_center().justify_center()
                .child(render(png_bytes)),
        )
        .child(div().text_size(px(16.0)).text_color(rgb(theme::CLR_TEXT)).text_align(TextAlign::Center).child(status))
        .child(div().text_size(px(13.0)).text_color(rgb(theme::CLR_MUTED)).child("二维码仅限微博手机客户端扫描"))
        .child(div().text_size(px(13.0)).text_color(rgb(theme::CLR_MUTED)).child("打开微博 App → 扫一扫 → 确认登录"))
        .into_any_element()
}
