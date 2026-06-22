//! Root ViewModel — top-level state machine, routes between login and home.

use gpui::*;

use crate::domain::LoginPhase;
use crate::infra::cookie_io;
use crate::model::{auth_service, timeline_service};
use crate::view::screens;
use crate::log_info;

use super::home_vm;
use super::login_vm;

pub struct AppRoot {
    pub phase: LoginPhase,
    pub tokio_handle: tokio::runtime::Handle,
    pub list_state: ListState,
    /// Pagination: last status id for "load more"
    pub since_id: String,
    /// True while loading more items (prevents duplicate triggers)
    pub loading_more: bool,
    /// The list_id for friendstimeline (cached from allGroups)
    pub feed_list_id: Option<String>,
}

impl AppRoot {
    pub fn new(cx: &mut Context<Self>, tokio_handle: tokio::runtime::Handle) -> Self {
        let this = Self {
            phase: LoginPhase::CheckingCookie,
            tokio_handle: tokio_handle.clone(),
            list_state: ListState::new(0, ListAlignment::Top, px(200.0)),
            since_id: String::new(),
            loading_more: false,
            feed_list_id: None,
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

    /// Trigger "load more" when scrolling near the bottom.
    fn try_load_more(&mut self, cx: &mut Context<Self>, visible_end: usize, total: usize) {
        if self.loading_more || self.since_id.is_empty() {
            return;
        }
        // Trigger when within 3 items of the end
        if visible_end + 3 < total {
            return;
        }

        log_info!("[load_more] 滚动到底, 加载更多 (since_id={})...", self.since_id);
        self.loading_more = true;

        let handle = self.tokio_handle.clone();
        let since_id = self.since_id.clone();
        let feed_list_id = self.feed_list_id.clone();

        cx.spawn(|this: WeakEntity<AppRoot>, cx: &mut AsyncApp| {
            let mut cx = cx.clone();
            async move {
                let (new_items, new_since_id) =
                    handle.block_on(timeline_service::load_more(&since_id, &feed_list_id));

                this.update(&mut cx, |v, cx| {
                    if let LoginPhase::HomeLoaded { ref mut items, ref mut title } = v.phase {
                        let old_len = items.len();
                        items.extend(new_items);
                        v.since_id = new_since_id;
                        v.loading_more = false;
                        log_info!("[load_more] 追加 {} 条, 总计 {} 条", items.len() - old_len, items.len());
                        v.list_state = ListState::new(items.len(), ListAlignment::Top, px(200.0));
                        *title = format!("📰 首页时间线 ({}条)", items.len());
                    }
                    cx.notify();
                }).ok();
            }
        }).detach();
    }
}

impl Render for AppRoot {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // Register scroll handler on list_state for infinite scroll
        let total = match &self.phase {
            LoginPhase::HomeLoaded { items, .. } => items.len(),
            _ => 0,
        };
        if total > 0 && !self.since_id.is_empty() {
            let total = total;
            self.list_state.set_scroll_handler(cx.listener(
                move |this, event: &ListScrollEvent, _window, cx| {
                    this.try_load_more(cx, event.visible_range.end, total);
                },
            ));
        }

        screens::root_screen::render(&self.phase, &self.list_state, cx)
    }
}
