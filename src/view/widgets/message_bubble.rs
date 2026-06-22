//! Message bubble widget — renders a single chat message.

use gpui::*;
use gpui::prelude::*;

use crate::domain::ChatMessage;
use crate::view::theme;

pub fn render(msg: &ChatMessage) -> impl IntoElement {
    let is_self = msg.is_self;
    let bubble_color = if is_self { rgb(theme::CLR_ACCENT) } else { rgb(0x2a3a5a) };
    let text_color = if is_self { rgb(0xffffff) } else { rgb(theme::CLR_TEXT) };

    div()
        .flex().flex_row().w_full().px_2().py_1()
        .when(is_self, |d| d.justify_end())
        .when(!is_self, |d| d.justify_start())
        .child(
            div()
                .flex().flex_col().gap_1()
                .max_w(px(300.0))
                .child(
                    div()
                        .text_size(px(11.0)).text_color(rgb(theme::CLR_MUTED))
                        .px_1()
                        .child(if is_self { "我".to_string() } else { msg.sender_name.clone() }),
                )
                .child(
                    div()
                        .px_3().py_2().rounded_lg()
                        .bg(bubble_color)
                        .text_size(px(13.0)).text_color(text_color)
                        .child(msg.text.clone()),
                ),
        )
}
