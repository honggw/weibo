//! Root screen — top-level layout, delegates to login or home based on phase.

use gpui::*;

use crate::domain::LoginPhase;
use crate::view::theme;
use crate::view::widgets::header_bar;
use crate::viewmodel::root_vm::AppRoot;

use super::home_screen;
use super::login_screen;

pub fn render(phase: &LoginPhase, list_state: &ListState, cx: &mut Context<AppRoot>) -> impl IntoElement {
    let is_logged_in = matches!(phase, LoginPhase::HomeLoaded { .. });

    div()
        .flex().flex_col().size_full()
        .bg(rgb(theme::CLR_BG))
        .text_color(rgb(theme::CLR_TEXT))
        .font_family("Microsoft YaHei, sans-serif")
        .child(header_bar::render(is_logged_in, cx.listener(
            |this, _: &ClickEvent, _window, cx| this.logout(cx),
        )))
        .child(body(phase, list_state))
}

fn body(phase: &LoginPhase, list_state: &ListState) -> AnyElement {
    match phase {
        LoginPhase::CheckingCookie => login_screen::render_centered("正在检查登录状态...", true),
        LoginPhase::Loading(msg) => login_screen::render_centered(msg, true),
        LoginPhase::WaitingScan { status, qr_png_bytes } => login_screen::render_qr(status, qr_png_bytes),
        LoginPhase::Exchanging(msg) => login_screen::render_centered(msg, true),
        LoginPhase::FetchingHome => login_screen::render_centered("正在获取首页...", true),
        LoginPhase::HomeLoaded { items, title } => home_screen::render(title, items, list_state),
        LoginPhase::Error(msg) => login_screen::render_error(msg),
    }
}
