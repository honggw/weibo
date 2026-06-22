//! Home screen — scrollable timeline list with title header.

use gpui::*;

use crate::domain::TimelineItem;
use crate::view::theme;
use crate::view::widgets::timeline_card;

pub fn render(title: &str, items: &[TimelineItem], list_state: &ListState) -> AnyElement {
    // Clone items for the render closure (called per-frame by GPUI)
    let items: Vec<TimelineItem> = items.to_vec();
    let item_count = items.len();
    let title = title.to_string();

    div()
        .flex().flex_col().size_full()
        .child(
            div().px_3().py_2().child(
                div().text_size(px(18.0)).font_weight(FontWeight::BOLD).text_color(rgb(theme::CLR_ACCENT)).child(title),
            ),
        )
        .child(
            list(list_state.clone(), move |ix, _window, _cx| {
                if ix < item_count {
                    let item = &items[ix];
                    timeline_card::render(item).into_any_element()
                } else {
                    div().into_any_element()
                }
            })
            .w_full()
            .flex_1(),
        )
        .into_any_element()
}
