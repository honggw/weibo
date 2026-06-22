//! Root screen — header, tab bar, and body routing.

use gpui::*;

use crate::domain::{ActiveTab, LoginPhase};
use crate::view::theme;
use crate::view::widgets::header_bar;
use crate::viewmodel::chat_vm::ChatData;
use crate::viewmodel::root_vm::AppRoot;

use super::{chat_screen, home_screen, login_screen};

pub fn render(
    phase: &LoginPhase,
    active_tab: &ActiveTab,
    dm_unread: u64,
    list_state: &ListState,
    chat_data: Option<&ChatData>,
    chat_list_state: &ListState,
    msg_list_state: &ListState,
    cx: &mut Context<AppRoot>,
) -> impl IntoElement {
    let is_logged_in = matches!(phase, LoginPhase::HomeLoaded { .. });

    div()
        .flex().flex_col().size_full()
        .bg(rgb(theme::CLR_BG))
        .text_color(rgb(theme::CLR_TEXT))
        .font_family("Microsoft YaHei, sans-serif")
        .child(header_bar::render(is_logged_in, cx.listener(
            |this, _: &ClickEvent, _window, cx| this.logout(cx),
        )))
        .child(render_tabs(active_tab, dm_unread, cx))
        .child(body(phase, active_tab, dm_unread, list_state, chat_data, chat_list_state, msg_list_state, cx))
}

fn render_tabs(active: &ActiveTab, dm_unread: u64, cx: &mut Context<AppRoot>) -> impl IntoElement {
    let home_active = matches!(active, ActiveTab::Home);
    let chat_active = matches!(active, ActiveTab::Chat);

    div()
        .flex().flex_row()
        .bg(rgb(0x0a1a3a))
        .border_b_1().border_color(rgb(theme::CLR_ACCENT))
        .child(
            div()
                .id("tab-home").cursor_pointer()
                .px_4().py_2()
                .border_b_2()
                .border_color(if home_active { rgb(theme::CLR_ACCENT) } else { rgb(0x00000000) })
                .text_color(if home_active { rgb(theme::CLR_ACCENT) } else { rgb(theme::CLR_MUTED) })
                .text_size(px(14.0)).font_weight(FontWeight::BOLD)
                .child("🏠 首页")
                .on_click(cx.listener(|this, _: &ClickEvent, _window, cx| {
                    this.switch_tab(cx, ActiveTab::Home);
                })),
        )
        .child(
            div()
                .id("tab-chat").cursor_pointer()
                .px_4().py_2()
                .border_b_2()
                .border_color(if chat_active { rgb(theme::CLR_ACCENT) } else { rgb(0x00000000) })
                .text_color(if chat_active { rgb(theme::CLR_ACCENT) } else { rgb(theme::CLR_MUTED) })
                .text_size(px(14.0)).font_weight(FontWeight::BOLD)
                .child(if dm_unread > 0 {
                    format!("💬 聊天 ({})", dm_unread)
                } else {
                    "💬 聊天".into()
                })
                .on_click(cx.listener(|this, _: &ClickEvent, _window, cx| {
                    this.switch_tab(cx, ActiveTab::Chat);
                })),
        )
}

fn body(phase: &LoginPhase, active_tab: &ActiveTab, _dm_unread: u64, list_state: &ListState, chat_data: Option<&ChatData>, chat_list_state: &ListState, msg_list_state: &ListState, cx: &mut Context<AppRoot>) -> AnyElement {
    // If not logged in, show login screen regardless of tab
    if !matches!(phase, LoginPhase::HomeLoaded { .. }) {
        return match phase {
            LoginPhase::WaitingScan { status, qr_png_bytes } => login_screen::render_qr(status, qr_png_bytes),
            LoginPhase::Error(msg) => login_screen::render_error(msg),
            _ => login_screen::render_centered("加载中...", true),
        };
    }

    // Logged in: route by tab
    match active_tab {
        ActiveTab::Home => {
            if let LoginPhase::HomeLoaded { items, title } = phase {
                home_screen::render(title, items, list_state)
            } else {
                login_screen::render_centered("加载中...", true)
            }
        }
        ActiveTab::Chat => chat_screen::render(chat_data, chat_list_state, msg_list_state, cx),
    }
}
