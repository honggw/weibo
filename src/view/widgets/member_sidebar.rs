//! Group member sidebar widget — displays group info and member list.

use gpui::*;

use crate::domain::{GroupInfo, GroupMember};
use crate::view::theme;

pub fn render(info: &GroupInfo) -> impl IntoElement {
    let members: Vec<GroupMember> = info.members.clone();

    div()
        .flex().flex_col().w(px(180.0)).h_full()
        .bg(rgb(0x0d1b36))
        .border_l_1().border_color(rgb(0x1a2a4a))
        // Header
        .child(
            div().px_3().py_2().border_b_1().border_color(rgb(0x1a2a4a))
                .child(
                    div().flex().flex_col().gap_1()
                        .child(
                            div()
                                .text_size(px(14.0))
                                .font_weight(FontWeight::BOLD)
                                .text_color(rgb(theme::CLR_TEXT))
                                .child(info.name.clone()),
                        )
                        .child(
                            div()
                                .text_size(px(11.0))
                                .text_color(rgb(theme::CLR_MUTED))
                                .child(format!("{} 名成员", info.member_count)),
                        ),
                ),
        )
        // Member list
        .child(
            div().flex().flex_col().flex_1()
                .children(members.iter().map(|m| render_member(m))),
        )
}

fn render_member(member: &GroupMember) -> impl IntoElement {
    let avatar_char = member
        .screen_name
        .chars()
        .next()
        .map(|c| c.to_string())
        .unwrap_or_default();

    div()
        .flex().flex_row().items_center().gap_2()
        .px_3().py_2()
        .hover(|s| s.bg(rgb(0x1a2a4a)))
        .child(
            // Avatar placeholder
            div()
                .w(px(32.0)).h(px(32.0)).rounded_full()
                .bg(rgb(theme::CLR_ACCENT))
                .flex().items_center().justify_center()
                .text_size(px(12.0)).text_color(rgb(0xffffff))
                .flex_shrink_0()
                .child(avatar_char),
        )
        .child(
            div().flex().flex_col().flex_1()
                .child(
                    div().flex().flex_row().items_center().gap_1()
                        .child(
                            div()
                                .text_size(px(12.0))
                                .text_color(rgb(theme::CLR_TEXT))
                                .child(member.screen_name.clone()),
                        )
                        .child(if member.is_admin {
                            div()
                                .px_1().rounded_sm()
                                .bg(rgb(0x4a9eff))
                                .text_size(px(10.0))
                                .text_color(rgb(0xffffff))
                                .child("管理")
                                .into_any_element()
                        } else {
                            div().into_any_element()
                        }),
                ),
        )
}
