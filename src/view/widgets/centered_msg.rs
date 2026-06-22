//! Centered message widget with optional loading spinner.

use gpui::*;
use gpui::prelude::*; // for into_any_element()

use crate::view::theme;

pub fn render(text: &str, show_spinner: bool) -> AnyElement {
    let text = text.to_string();
    div()
        .flex().flex_col().size_full().items_center().justify_center().gap_4()
        .child(div().text_size(px(16.0)).text_color(rgb(theme::CLR_TEXT)).child(text))
        .child(if show_spinner {
            div().text_size(px(32.0)).text_color(rgb(theme::CLR_ACCENT)).child("⏳").into_any_element()
        } else {
            div().into_any_element()
        })
        .into_any_element()
}
