//! Contact/conversation card widget for the chat list.

use gpui::*;

use crate::domain::Contact;
use crate::view::theme;

pub fn render(contact: &Contact) -> impl IntoElement {
    // 头像: 如果有真实头像 URL, 优先使用 (fallback 到首字母占位)
    let avatar: AnyElement = if !contact.avatar.is_empty() {
        div()
            .w(px(40.0)).h(px(40.0)).rounded_full()
            .bg(rgb(theme::CLR_ACCENT))
            .flex().items_center().justify_center()
            .text_size(px(16.0)).text_color(rgb(0xffffff))
            .child(if contact.is_group {
                "群".to_string()
            } else {
                contact.screen_name.chars().next().map(|c| c.to_string()).unwrap_or_default()
            })
            .into_any_element()
    } else {
        div()
            .w(px(40.0)).h(px(40.0)).rounded_full()
            .bg(rgb(theme::CLR_ACCENT))
            .flex().items_center().justify_center()
            .text_size(px(16.0)).text_color(rgb(0xffffff))
            .child(if contact.is_group {
                "群".to_string()
            } else {
                contact.screen_name.chars().next().map(|c| c.to_string()).unwrap_or_default()
            })
            .into_any_element()
    };

    div()
        .flex().flex_row().items_center().gap_3()
        .px_3().py_2()
        .hover(|s| s.bg(rgb(0x1a2a4a)))
        .rounded_md()
        .child(avatar)
        .child(
            div().flex().flex_col().flex_1().gap_1().overflow_hidden()
                .child(
                    div().flex().flex_row().justify_between()
                        .child(div().text_size(px(14.0)).font_weight(FontWeight::BOLD).text_color(rgb(theme::CLR_TEXT)).child(contact.screen_name.clone()))
                        .child(div().text_size(px(11.0)).text_color(rgb(theme::CLR_MUTED)).child(contact.last_time.clone())),
                )
                .child(
                    div().flex().flex_row().justify_between()
                        .child(
                            div().text_size(px(12.0)).text_color(rgb(theme::CLR_MUTED))
                                .whitespace_nowrap().overflow_hidden().text_ellipsis()
                                .child(contact.last_message.clone()),
                        )
                        .child(if contact.unread_count > 0 {
                            div().px_2().py_0p5().rounded_full().bg(rgb(theme::CLR_ACCENT))
                                .text_size(px(11.0)).text_color(rgb(0xffffff))
                                .child(format!("{}", contact.unread_count))
                                .into_any_element()
                        } else {
                            div().into_any_element()
                        }),
                ),
        )
}
