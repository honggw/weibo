//! Home screen — timeline list with title header.

use gpui::*;
use gpui::prelude::*; // for into_any_element()

use crate::domain::TimelineItem;
use crate::view::theme;
use crate::view::widgets::timeline_card;

pub fn render(title: &str, items: &[TimelineItem]) -> AnyElement {
    div()
        .flex().flex_col().size_full().px_3().py_3().gap_3()
        .child(
            div().px_2().py_2().child(
                div().text_size(px(18.0)).font_weight(FontWeight::BOLD).text_color(rgb(theme::CLR_ACCENT)).child(title.to_string()),
            ),
        )
        .children(items.iter().map(|item| timeline_card::render(item).into_any_element()))
        .into_any_element()
}
