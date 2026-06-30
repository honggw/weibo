//! Home ViewModel — cookie verification + timeline loading orchestration.
//! Fast path when saved cookies exist: verify → fetch home directly.
//!
//! 所有函数使用泛型 `<C: VMContext>` 而非 dyn trait object。

use weibo_domain::LoginPhase;
use weibo_model::auth_service;
use weibo_model::timeline_service;
use weibo_infra::{log_info, log_success};

use crate::app_state::AppState;
use crate::context::VMContext;
use crate::login_vm;

/// 检查已保存的 Cookie 是否有效。
/// 有效 → FetchingHome → HomeLoaded
/// 无效 → 回退到 QR 扫码登录
pub fn check_cookie<C: VMContext<State = AppState>>(ctx: &C, _state: &AppState) {
    let cookie = match auth_service::load_saved_cookie() {
        Some(c) => c,
        None => {
            log_info!("未发现 Cookie, 进入扫码登录");
            login_vm::start_login_flow(ctx);
            return;
        }
    };

    log_info!("发现已保存的 Cookie, 尝试验证...");

    ctx.spawn_task(
        async move { auth_service::verify_cookie(&cookie).await.unwrap_or(false) },
        move |state, valid| {
            if !valid {
                log_info!("Cookie 已过期, 回退扫码登录");
                state.phase = LoginPhase::Loading("Cookie 已过期, 重新连接...".into());
                // Can't call ctx from callback — login flow will be triggered
                // by frontend on seeing the Loading state
                return;
            }
            log_success!("Cookie 有效, 加载首页");
            state.phase = LoginPhase::FetchingHome;
        },
    );
}

/// 加载首页内容 (FetchingHome 状态下由前端调用)
pub fn fetch_home<C: VMContext<State = AppState>>(ctx: &C) {
    ctx.spawn_task(
        async { timeline_service::fetch_first_page().await },
        |state, (items, title, feed_list_id, since_id)| {
            state.timeline.items = items;
            state.timeline.title = title;
            state.timeline.feed_list_id = feed_list_id;
            state.timeline.since_id = since_id;
            state.phase = LoginPhase::HomeLoaded {
                items: state.timeline.items.clone(),
                title: state.timeline.title.clone(),
            };
            log_info!("[cookie] HomeLoaded 已设置 ✅");
        },
    );
}

/// 登出: 删除 Cookie, 回到扫码登录界面
pub fn logout<C: VMContext<State = AppState>>(ctx: &C) {
    log_info!("用户点击登出");
    weibo_infra::cookie_io::delete();
    ctx.spawn_task(
        async { () },
        |state, _val| {
            *state = AppState::new();
            state.phase = LoginPhase::Loading("正在登出...".into());
        },
    );
}

/// 加载更多时间线内容 (分页)
pub fn load_more_timeline<C: VMContext<State = AppState>>(ctx: &C, state: &AppState) {
    let since_id = state.timeline.since_id.clone();
    let feed_list_id = state.timeline.feed_list_id.clone();

    if since_id.is_empty() || state.timeline.loading_more {
        return;
    }

    log_info!("[load_more] 加载更多 (since_id={})...", since_id);

    ctx.spawn_task(
        async move { timeline_service::load_more(&since_id, &feed_list_id).await },
        move |state, (new_items, new_since_id)| {
            let old_len = state.timeline.items.len();
            state.timeline.items.extend(new_items);
            state.timeline.since_id = new_since_id;
            state.timeline.loading_more = false;
            log_info!(
                "[load_more] 追加 {} 条, 总计 {} 条",
                state.timeline.items.len() - old_len,
                state.timeline.items.len()
            );
            state.timeline.title = format!("📰 首页时间线 ({}条)", state.timeline.items.len());
            if let LoginPhase::HomeLoaded { ref mut items, ref mut title } = state.phase {
                *items = state.timeline.items.clone();
                *title = state.timeline.title.clone();
            }
        },
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::context::MockContext;

    #[test]
    fn test_logout_resets_state() {
        let ctx = MockContext::new(AppState::new());
        logout(&ctx);
        let state = ctx.state();
        assert!(matches!(state.phase, LoginPhase::Loading(_)));
        assert!(ctx.notified_count() > 0);
    }
}
