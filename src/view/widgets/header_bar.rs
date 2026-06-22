//! Header bar widget — app title + optional logout button.

use gpui::*;
use gpui::prelude::*;

use crate::view::theme;

pub fn render<F>(is_logged_in: bool, on_logout: F) -> impl IntoElement
where
    F: Fn(&ClickEvent, &mut Window, &mut App) + 'static,
{
    div()
        .flex().flex_row().items_center().justify_between()
        .px_4().py_3()
        .bg(rgb(theme::CLR_HEADER))
        .border_b_1().border_color(rgb(theme::CLR_ACCENT))
        .child(
            div().flex().flex_row().items_center().gap_2()
                .child(div().text_size(px(20.0)).font_weight(FontWeight::BOLD).text_color(rgb(theme::CLR_ACCENT)).child("微博"))
                .child(div().text_size(px(12.0)).text_color(rgb(theme::CLR_MUTED)).child("PC 客户端")),
        )
        .when(is_logged_in, |this| {
            this.child(
                div().id("logout-btn")
                    .px_3().py_1().rounded_full().bg(rgb(theme::CLR_BTN)).cursor_pointer()
                    .text_size(px(13.0)).text_color(rgb(theme::CLR_MUTED))
                    .child("登出")
                    .on_click(on_logout),
            )
        })
}
