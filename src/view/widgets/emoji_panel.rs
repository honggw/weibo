//! Emoji panel widget — grid of clickable emoji items.

use gpui::*;
use crate::domain::Emotion;
use crate::view::theme;
use crate::viewmodel::root_vm::AppRoot;

pub fn render(emotions: &[Emotion], cx: &mut Context<AppRoot>) -> impl IntoElement {
    let emotions_owned: Vec<Emotion> = emotions.to_vec();
    let cols = 8; // 每行 8 个表情

    div()
        .flex().flex_col().w_full().max_h(px(200.0))
        .bg(rgb(0x0d1b36)).border_t_1().border_color(rgb(0x1a2a4a))
        .px_2().py_2()
        .children(
            emotions_owned.chunks(cols).enumerate().map(|(row_idx, row)| {
                let row_items: Vec<Emotion> = row.to_vec();
                div().flex().flex_row().gap_1()
                    .children(row_items.into_iter().enumerate().map(|(col_idx, em)| {
                        let phrase = em.phrase.clone();
                        let display = phrase.trim_matches(|c| c == '[' || c == ']').to_string();
                        div()
                            .id(("emoji", row_idx * cols + col_idx))
                            .cursor_pointer()
                            .px_1().py_1().rounded_md()
                            .hover(|s| s.bg(rgb(0x1a2a4a)))
                            .text_size(px(12.0)).text_color(rgb(theme::CLR_TEXT))
                            .child(display)
                            .on_click(cx.listener(move |this, _: &ClickEvent, _window, cx| {
                                if let Some(chat) = this.chat_data.as_mut() {
                                    chat.draft_text.push_str(&phrase);
                                    chat.show_emoji_panel = false;
                                }
                                cx.notify();
                            }))
                    }))
            })
        )
}
