//! 事件序列化定义 — 从 AppState 提取需要发送给前端的快照数据

use serde::Serialize;
use weibo_viewmodel::app_state::AppState;

/// 状态快照: 前端初始化或同步时返回
#[derive(Serialize)]
pub struct StateSnapshot {
    pub phase: String,
    pub phase_data: PhaseData,
    pub active_tab: String,
    pub dm_unread: u64,
    pub timeline_title: String,
    pub timeline_items: Vec<TimelineItemSnapshot>,
    pub has_more_timeline: bool,
}

#[derive(Serialize)]
pub struct PhaseData {
    pub status: Option<String>,
    pub has_qr: bool,
}

#[derive(Serialize)]
pub struct TimelineItemSnapshot {
    pub user_name: String,
    pub text: String,
}

impl From<&AppState> for StateSnapshot {
    fn from(state: &AppState) -> Self {
        let (phase_name, status) = match &state.phase {
            weibo_domain::LoginPhase::CheckingCookie => ("checking_cookie", Some("检查已保存的登录状态...".into())),
            weibo_domain::LoginPhase::Loading(msg) => ("loading", Some(msg.clone())),
            weibo_domain::LoginPhase::WaitingScan { status: s, .. } => ("waiting_scan", Some(s.clone())),
            weibo_domain::LoginPhase::Exchanging(msg) => ("exchanging", Some(msg.clone())),
            weibo_domain::LoginPhase::FetchingHome => ("fetching_home", Some("加载首页...".into())),
            weibo_domain::LoginPhase::HomeLoaded { title, .. } => ("home_loaded", Some(title.clone())),
            weibo_domain::LoginPhase::Error(msg) => ("error", Some(msg.clone())),
        };

        StateSnapshot {
            phase: phase_name.into(),
            phase_data: PhaseData {
                status,
                has_qr: matches!(state.phase, weibo_domain::LoginPhase::WaitingScan { .. }),
            },
            active_tab: match state.active_tab {
                weibo_domain::ActiveTab::Home => "home".into(),
                weibo_domain::ActiveTab::Chat => "chat".into(),
            },
            dm_unread: state.dm_unread,
            timeline_title: state.timeline.title.clone(),
            timeline_items: state
                .timeline
                .items
                .iter()
                .map(|i| TimelineItemSnapshot {
                    user_name: i.user_name.clone(),
                    text: i.text.clone(),
                })
                .collect(),
            has_more_timeline: !state.timeline.since_id.is_empty(),
        }
    }
}
