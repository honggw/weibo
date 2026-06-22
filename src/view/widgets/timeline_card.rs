//! Timeline card widget — renders a single weibo post or hotsearch entry.

use gpui::*;

use crate::domain::TimelineItem;
use crate::view::theme;

pub fn render(item: &TimelineItem) -> impl IntoElement {
    div()
        .flex().flex_col().bg(rgb(theme::CLR_CARD)).rounded_lg().px_4().py_3().gap_1()
        .child(div().text_size(px(14.0)).font_weight(FontWeight::BOLD).text_color(rgb(theme::CLR_ACCENT)).child(item.user_name.clone()))
        .child(div().text_size(px(13.0)).text_color(rgb(theme::CLR_TEXT)).line_height(relative(1.6)).child(item.text.clone()))
}
