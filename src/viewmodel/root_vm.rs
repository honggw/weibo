//! Root ViewModel — top-level state machine, routes between login and home.

use gpui::*;

use crate::domain::LoginPhase;
use crate::infra::cookie_io;
use crate::model::auth_service;
use crate::view::screens;
use crate::log_info;

use super::home_vm;
use super::login_vm;

pub struct AppRoot {
    pub phase: LoginPhase,
    pub tokio_handle: tokio::runtime::Handle,
}

impl AppRoot {
    pub fn new(cx: &mut Context<Self>, tokio_handle: tokio::runtime::Handle) -> Self {
        let this = Self {
            phase: LoginPhase::CheckingCookie,
            tokio_handle: tokio_handle.clone(),
        };

        if let Some(cookie) = auth_service::load_saved_cookie() {
            log_info!("发现已保存的 Cookie, 尝试验证...");
            home_vm::start_cookie_flow(cx, &this.tokio_handle, cookie);
        } else {
            log_info!("未发现 Cookie, 进入扫码登录");
            login_vm::start_login_flow(cx, &this.tokio_handle);
        }

        this
    }

    pub fn logout(&mut self, cx: &mut Context<Self>) {
        log_info!("用户点击登出");
        cookie_io::delete();
        self.phase = LoginPhase::Loading("正在登出...".into());
        cx.notify();
        login_vm::start_login_flow(cx, &self.tokio_handle);
    }
}

impl Render for AppRoot {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        screens::root_screen::render(&self.phase, cx)
    }
}
