//! GPUI-based UI views for Weibo PC client.
//!
//! Architecture:
//!   AppRoot — root view, switches between login/home states
//!   Startup: try saved cookies → if valid, load home → else show QR login
//!   Login flow runs on a tokio runtime, updates view via context.spawn()

use gpui::*;
use std::path::PathBuf;
use std::time::Duration;

use crate::qr_login::{QrLogin, QrStatus};
use crate::{log_error, log_info, log_success};

// ============================================================================
// Constants
// ============================================================================

const QR_IMAGE_PATH: &str = "weibo_qr.png";
const COOKIE_FILE: &str = "weibo_cookies.json";
const APP_BG: u32 = 0x1a1a2e;
const CARD_BG: u32 = 0x16213e;
const ACCENT: u32 = 0xe8633a;
const TEXT_PRIMARY: u32 = 0xe8e8e8;
const TEXT_SECONDARY: u32 = 0x888888;

// ============================================================================
// App State
// ============================================================================

#[derive(Clone)]
struct TimelineItem {
    user_name: String,
    text: String,
}

enum AppState {
    /// Checking saved cookies on startup
    CheckingCookie,
    /// Initial: warmup + fetching QR code
    Loading { status: String },
    /// QR code ready, waiting for scan
    WaitingScan { status: String, qr_path: Option<PathBuf> },
    /// Login confirmed, exchanging ticket
    Exchanging { status: String },
    /// Fetching timeline from API
    FetchingHome,
    /// Timeline loaded and displayed
    HomeLoaded { statuses: Vec<TimelineItem>, title: String },
    /// Error state
    Error { message: String },
}

// ============================================================================
// AppRoot — root view managing all states
// ============================================================================

pub struct AppRoot {
    tokio_handle: tokio::runtime::Handle,
    state: AppState,
}

impl AppRoot {
    pub fn new(cx: &mut Context<Self>, tokio_handle: tokio::runtime::Handle) -> Self {
        let this = Self {
            tokio_handle: tokio_handle.clone(),
            state: AppState::CheckingCookie,
        };

        // Try loading saved cookies first — if they exist, verify and skip login
        if let Some(cookie_header) = Self::load_cookie_header_from_file() {
            log_info!(
                "发现已保存的 Cookie ({}), SUB={}..., 尝试验证...",
                COOKIE_FILE,
                &cookie_header[..cookie_header.len().min(50)]
            );
            this.start_cookie_login(cx, cookie_header);
        } else {
            log_info!("未发现 Cookie 文件, 进入扫码登录流程");
            this.start_login_flow(cx);
        }

        this
    }

    // ========================================================================
    // Cookie persistence helpers
    // ========================================================================

    /// Load saved cookies from file and build a Cookie header string.
    /// Returns None if no valid SUB cookie found.
    fn load_cookie_header_from_file() -> Option<String> {
        let data = std::fs::read_to_string(COOKIE_FILE).ok()?;
        let parsed: serde_json::Value = serde_json::from_str(&data).ok()?;

        let cookies: Vec<(String, String)> = parsed
            .get("cookies")?
            .as_array()?
            .iter()
            .filter_map(|c| {
                let name = c.get("name")?.as_str()?;
                let value = c.get("value")?.as_str()?;
                Some((name.to_string(), value.to_string()))
            })
            .collect();

        let sub = cookies.iter().find(|(n, _)| n == "SUB")?;
        if sub.1.is_empty() {
            return None;
        }

        // Only send SUB + SUBP — other cookies may cause issues (curl verifies this works)
        let header = cookies
            .iter()
            .filter(|(n, _)| n == "SUB" || n == "SUBP")
            .map(|(n, v)| format!("{}={}", n, v))
            .collect::<Vec<_>>()
            .join("; ");
        log_info!(
            "从文件加载了 Cookie (SUB={}...), 共 {} 个键",
            &sub.1[..sub.1.len().min(30)],
            header.split(';').count()
        );
        Some(header)
    }

    // ========================================================================
    // Cookie login flow (skip QR, verify saved cookies → load timeline)
    // ========================================================================

    fn start_cookie_login(&self, cx: &Context<Self>, cookie_header: String) {
        let tokio_handle = self.tokio_handle.clone();

        cx.spawn(move |this: WeakEntity<AppRoot>, cx: &mut AsyncApp| {
            let mut cx = cx.clone();
            let cookie_header = cookie_header;
            async move {
            let result: Result<(), anyhow::Error> = tokio_handle.block_on(async {
                // --- Verify cookies ---
                this.update(&mut cx, |v, cx| {
                    v.state = AppState::Loading {
                        status: "检测到已保存的登录，正在验证...".into(),
                    };
                    cx.notify();
                })
                .ok();

                let client = reqwest::Client::builder()
                    .cookie_store(false)
                    .timeout(std::time::Duration::from_secs(15))
                    .build()?;

                let resp = client
                    .get("https://weibo.com/ajax/config/get_config")
                    .header("Cookie", &cookie_header)
                    .header("Referer", "https://weibo.com/")
                    .header(
                        "User-Agent",
                        "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 \
                         (KHTML, like Gecko) Chrome/148.0.0.0 Safari/537.36",
                    )
                    .header("X-Requested-With", "XMLHttpRequest")
                    .header("Accept", "application/json, text/plain, */*")
                    .send()
                    .await?;

                let data: serde_json::Value = resp.json().await?;
                let ok = data.get("ok").and_then(|v| v.as_i64()).unwrap_or(0);

                if ok != 1 {
                    log_info!("[cookie] Cookie 已过期 (ok={})，回退扫码登录", ok);
                    // Cookie expired — fall back to QR login
                    this.update(&mut cx, |v, cx| {
                        v.state = AppState::Loading {
                            status: "登录已过期，正在重新连接...".into(),
                        };
                        cx.notify();
                        // Trigger login flow from within the update
                        v.start_login_flow(cx);
                    })
                    .ok();
                    return Ok(());
                }

                log_success!("[cookie] Cookie 有效，直接加载首页");
                // Cookie valid — directly fetch timeline
                this.update(&mut cx, |v, cx| {
                    v.state = AppState::FetchingHome;
                    cx.notify();
                })
                .ok();

                Self::fetch_timeline_with_cookie(&this, &mut cx, &client, &cookie_header).await?;

                Ok(())
            });

            if let Err(e) = result {
                // Verification error — fall back to QR login
                log_error!(
                    "[cookie] Cookie 验证失败: {:#} — 将回退到扫码登录",
                    e
                );
                this.update(&mut cx, |v, cx| {
                    v.state = AppState::Loading {
                        status: format!("Cookie 已过期，重新连接...({})", e),
                    };
                    cx.notify();
                    v.start_login_flow(cx);
                })
                .ok();
            }
            }
        })
        .detach();
    }

    // ========================================================================
    // QR login flow
    // ========================================================================

    /// Spawn the full login flow (warmup → QR → poll → exchange → save cookies → timeline)
    fn start_login_flow(&self, cx: &Context<Self>) {
        let tokio_handle = self.tokio_handle.clone();

        cx.spawn(|this: WeakEntity<AppRoot>, cx: &mut AsyncApp| {
            let mut cx = cx.clone();
            async move {
            let result: Result<(), anyhow::Error> = tokio_handle.block_on(async {
                // --- Step 1: Init + warmup ---
                this.update(&mut cx, |v, cx| {
                    v.state = AppState::Loading {
                        status: "正在连接微博...".into(),
                    };
                    cx.notify();
                })
                .ok();

                let mut login = QrLogin::new()?;
                log_info!("[login] 正在 warmup...");
                login.warmup().await?;
                log_success!("[login] warmup 完成");

                // --- Step 2: Fetch QR code ---
                this.update(&mut cx, |v, cx| {
                    v.state = AppState::Loading {
                        status: "获取二维码...".into(),
                    };
                    cx.notify();
                })
                .ok();

                log_info!("[login] 获取二维码...");
                login.fetch_qr_code().await?;
                login.download_qr_image().await?;
                log_success!("[login] 二维码已获取");

                let qr_path = std::path::Path::new(QR_IMAGE_PATH);
                login.save_qr_image(qr_path)?;
                log_info!("[login] 二维码已保存: {}", qr_path.display());

                this.update(&mut cx, |v, cx| {
                    v.state = AppState::WaitingScan {
                        status: "📱 请用微博手机客户端扫描二维码".into(),
                        qr_path: Some(qr_path.to_path_buf()),
                    };
                    cx.notify();
                })
                .ok();

                // --- Step 3: Poll scan status ---
                let final_cookie = loop {
                    match login.poll_status().await {
                        Ok(QrStatus::Waiting) => {
                            this.update(&mut cx, |v, cx| {
                                if let AppState::WaitingScan { ref mut status, .. } =
                                    v.state
                                {
                                    *status = "📱 等待扫码...".into();
                                }
                                cx.notify();
                            })
                            .ok();
                            tokio::time::sleep(Duration::from_secs(2)).await;
                        }
                        Ok(QrStatus::Scanned) => {
                            this.update(&mut cx, |v, cx| {
                                v.state = AppState::WaitingScan {
                                    status: "📲 已扫描！请在手机上点击「确认登录」".into(),
                                    qr_path: Some(qr_path.to_path_buf()),
                                };
                                cx.notify();
                            })
                            .ok();
                            tokio::time::sleep(Duration::from_secs(2)).await;
                        }
                        Ok(QrStatus::Confirmed { alt, redirect_url }) => {
                            this.update(&mut cx, |v, cx| {
                                v.state = AppState::Exchanging {
                                    status: "✅ 确认成功！正在获取登录票据...".into(),
                                };
                                cx.notify();
                            })
                            .ok();

                            login.exchange_ticket_with_url(&alt, &redirect_url).await?;

                            let verified = login.verify_login().await.unwrap_or(false);
                            if !verified {
                                this.update(&mut cx, |v, cx| {
                                    v.state = AppState::Error {
                                        message: "登录验证失败，请重试".into(),
                                    };
                                    cx.notify();
                                })
                                .ok();
                                return Ok(());
                            }

                            // --- Save cookies persistently ---
                            if let Err(e) = login
                                .save_cookies_to_file(std::path::Path::new(COOKIE_FILE))
                            {
                                log_error!("[login] 保存 Cookie 失败: {}", e);
                            } else {
                                log_success!("[login] Cookie 已保存到 {}", COOKIE_FILE);
                            }

                            let cookie_header = login.get_cookie_header();
                            break cookie_header;
                        }
                        Ok(QrStatus::Expired) => {
                            this.update(&mut cx, |v, cx| {
                                v.state = AppState::Loading {
                                    status: "⚠️ 二维码过期，重新获取...".into(),
                                };
                                cx.notify();
                            })
                            .ok();
                            login.fetch_qr_code().await?;
                            login.download_qr_image().await?;
                            login.save_qr_image(qr_path)?;
                            this.update(&mut cx, |v, cx| {
                                v.state = AppState::WaitingScan {
                                    status: "📱 请用微博手机客户端扫描二维码".into(),
                                    qr_path: Some(qr_path.to_path_buf()),
                                };
                                cx.notify();
                            })
                            .ok();
                        }
                        Ok(QrStatus::Unknown { code, msg, .. }) => {
                            log_info!("QR poll unknown: {} {}", code, msg);
                            tokio::time::sleep(Duration::from_secs(2)).await;
                        }
                        Err(e) => {
                            log_error!("QR poll error: {}", e);
                            tokio::time::sleep(Duration::from_secs(2)).await;
                        }
                    }
                };

                // --- Step 4: Fetch home timeline ---
                this.update(&mut cx, |v, cx| {
                    v.state = AppState::FetchingHome;
                    cx.notify();
                })
                .ok();

                Self::fetch_timeline_with_cookie(
                    &this,
                    &mut cx,
                    login.client(),
                    &final_cookie,
                )
                .await?;

                // Clean up QR image
                let _ = std::fs::remove_file(qr_path);
                Ok(())
            });

            if let Err(e) = result {
                log_error!(
                    "[login] 登录流程失败: {:#} — 请检查网络连接后重试",
                    e
                );
                this.update(&mut cx, |v, cx| {
                    v.state = AppState::Error {
                        message: format!(
                            "{}\n\n详细错误已输出到终端 (stderr)",
                            e
                        ),
                    };
                    cx.notify();
                })
                .ok();
            }
            }
        })
        .detach();
    }

    // ========================================================================
    // Shared: fetch home timeline
    // ========================================================================

    /// Fetch timeline: get followed users' recent posts, fallback to hot search
    async fn fetch_timeline_with_cookie(
        this: &WeakEntity<Self>,
        cx: &mut AsyncApp,
        client: &reqwest::Client,
        cookie_header: &str,
    ) -> Result<(), anyhow::Error> {
        // --- Step 1: Get followed users ---
        log_info!("获取关注列表...");
        match Self::fetch_following_ids(client, cookie_header).await {
            Ok(uids) if !uids.is_empty() => {
                log_info!("获取到 {} 个关注用户, 拉取微博...", uids.len());
                let statuses = Self::fetch_timeline_from_friends(client, cookie_header, &uids).await;
                if !statuses.is_empty() {
                    let count = statuses.len();
                    this.update(cx, |v, cx| {
                        v.state = AppState::HomeLoaded {
                            title: format!("📰 首页时间线 ({}位关注者, {}条)", uids.len().min(20), count),
                            statuses,
                        };
                        cx.notify();
                    }).ok();
                    log_success!("首页时间线加载完成: {} 条微博 (来自 {} 位关注者)",
                        count, uids.len().min(20));
                    return Ok(());
                }
                log_info!("关注者微博为空，回退到热搜榜");
            }
            Err(e) => log_info!("获取关注列表失败: {}, 回退到热搜榜", e),
            _ => log_info!("无关注用户，回退到热搜榜"),
        }
        // --- Fallback: hot search ---
        Self::fetch_hotsearch(this, cx, client).await
    }

    /// Fetch the list of followed user UIDs
    async fn fetch_following_ids(
        client: &reqwest::Client,
        cookie_header: &str,
    ) -> Result<Vec<u64>> {
        let resp = client
            .get("https://weibo.com/ajax/friendships/friends?page=1")
            .header("Cookie", cookie_header)
            .header("Referer", "https://weibo.com/")
            .header("User-Agent", "Mozilla/5.0")
            .header("X-Requested-With", "XMLHttpRequest")
            .timeout(std::time::Duration::from_secs(15))
            .send()
            .await?;
        let data: serde_json::Value = resp.json().await?;
        let uids: Vec<u64> = data
            .get("users")
            .and_then(|u| u.as_array())
            .map(|arr| {
                arr.iter()
                    .take(30) // limit to avoid too many requests
                    .filter_map(|u| u.get("id").and_then(|v| v.as_u64()))
                    .collect()
            })
            .unwrap_or_default();
        Ok(uids)
    }

    /// Fetch recent posts from followed users and combine into a timeline
    async fn fetch_timeline_from_friends(
        client: &reqwest::Client,
        cookie_header: &str,
        uids: &[u64],
    ) -> Vec<TimelineItem> {
        let mut all_items: Vec<TimelineItem> = Vec::new();
        // Take first 20 followed users to keep request count reasonable
        for &uid in uids.iter().take(20) {
            log_info!("拉取用户 {} 的微博...", uid);
            match client
                .get(format!(
                    "https://weibo.com/ajax/statuses/mymblog?uid={}&page=1&feature=0",
                    uid
                ))
                .header("Cookie", cookie_header)
                .header("Referer", "https://weibo.com/")
                .header("User-Agent", "Mozilla/5.0")
                .header("X-Requested-With", "XMLHttpRequest")
                .timeout(std::time::Duration::from_secs(10))
                .send()
                .await
            {
                Ok(resp) => {
                    if let Ok(data) = resp.json::<serde_json::Value>().await {
                        if let Some(list) = data
                            .get("data")
                            .and_then(|d| d.get("list"))
                            .and_then(|l| l.as_array())
                        {
                            for s in list.iter().take(3) {
                                // take up to 3 posts per user
                                let user_name = s
                                    .get("user")
                                    .and_then(|u| u.get("screen_name"))
                                    .and_then(|v| v.as_str())
                                    .unwrap_or("?")
                                    .to_string();
                                let text = s
                                    .get("text_raw")
                                    .or_else(|| s.get("text"))
                                    .and_then(|v| v.as_str())
                                    .unwrap_or("")
                                    .to_string();
                                if !text.is_empty() {
                                    all_items.push(TimelineItem { user_name, text });
                                }
                            }
                        }
                    }
                }
                Err(e) => log_info!("拉取用户 {} 微博失败: {}", uid, e),
            }
        }
        all_items
    }

    /// Fetch hot search trends
    async fn fetch_hotsearch(
        this: &WeakEntity<Self>,
        cx: &mut AsyncApp,
        client: &reqwest::Client,
    ) -> Result<(), anyhow::Error> {
        log_info!("请求热搜榜 API...");
        let resp = client
            .get("https://weibo.com/ajax/side/hotSearch")
            .header("Referer", "https://weibo.com/")
            .header("User-Agent", "Mozilla/5.0")
            .header("X-Requested-With", "XMLHttpRequest")
            .timeout(std::time::Duration::from_secs(15))
            .send()
            .await?;
        let data: serde_json::Value = resp.json().await?;
        let ok = data.get("ok").and_then(|v| v.as_i64()).unwrap_or(0);
        if ok != 1 {
            log_error!("热搜 API 返回 ok={}", ok);
        }
        // Weibo API changed: use "realtime" (and fallback "band_list")
        let band = data
            .get("data")
            .and_then(|d| d.get("realtime").or_else(|| d.get("band_list")))
            .and_then(|b| b.as_array());
        let statuses: Vec<TimelineItem> = band
            .map(|arr| {
                arr.iter().take(15).map(|item| {
                    let word = item.get("word").and_then(|v| v.as_str()).unwrap_or("?").to_string();
                    let num = item.get("num").and_then(|v| v.as_i64()).unwrap_or(0);
                    let note = item.get("note").or_else(|| item.get("category")).and_then(|v| v.as_str()).unwrap_or("");
                    let text = if num > 0 && !note.is_empty() {
                        format!("🔥 热度 {} — {}", num, note)
                    } else if num > 0 {
                        format!("🔥 热度 {}", num)
                    } else if !note.is_empty() {
                        format!("[{}]", note)
                    } else {
                        String::new()
                    };
                    TimelineItem { user_name: word, text }
                }).collect()
            })
            .unwrap_or_default();
        let count = statuses.len();
        this.update(cx, |v, cx| { v.state = AppState::HomeLoaded {
            title: format!("🔥 热搜榜 ({}条)", count), statuses
        }; cx.notify(); }).ok();
        log_success!("热搜榜加载完成: {} 条热搜", count);
        Ok(())
    }
}

// ============================================================================
// Render
// ============================================================================

impl Render for AppRoot {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .flex()
            .flex_col()
            .size_full()
            .bg(rgb(APP_BG))
            .text_color(rgb(TEXT_PRIMARY))
            .font_family("Microsoft YaHei, sans-serif")
            .child(self.render_header(cx))
            .child(self.render_body(cx))
    }
}

// ============================================================================
// Render Helpers
// ============================================================================

impl AppRoot {
    /// App header bar
    fn render_header(&self, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .flex()
            .flex_row()
            .items_center()
            .justify_between()
            .px_4()
            .py_3()
            .bg(rgb(0x0f3460))
            .border_b_1()
            .border_color(rgb(ACCENT))
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap_2()
                    .child(
                        div()
                            .text_size(px(20.0))
                            .font_weight(FontWeight::BOLD)
                            .text_color(rgb(ACCENT))
                            .child("微博"),
                    )
                    .child(
                        div()
                            .text_size(px(12.0))
                            .text_color(rgb(TEXT_SECONDARY))
                            .child("PC 客户端"),
                    ),
            )
    }

    /// Main content area — switches on state
    fn render_body(&self, _cx: &mut Context<Self>) -> AnyElement {
        match &self.state {
            AppState::CheckingCookie => {
                Self::render_centered("正在检查登录状态...", true).into_any_element()
            }
            AppState::Loading { status } => {
                Self::render_centered(status, true).into_any_element()
            }
            AppState::WaitingScan { status, qr_path } => {
                Self::render_qr_screen(status, qr_path)
            }
            AppState::Exchanging { status } => {
                Self::render_centered(status, true).into_any_element()
            }
            AppState::FetchingHome => {
                Self::render_centered("正在获取首页...", true).into_any_element()
            }
            AppState::HomeLoaded { statuses, title } => Self::render_timeline(title, statuses),
            AppState::Error { message } => Self::render_error(message),
        }
    }

    /// Centered status text with optional spinner
    fn render_centered(text: &str, show_spinner: bool) -> AnyElement {
        let text = text.to_string();
        div()
            .flex()
            .flex_col()
            .size_full()
            .items_center()
            .justify_center()
            .gap_4()
            .child(
                div()
                    .text_size(px(16.0))
                    .text_color(rgb(TEXT_PRIMARY))
                    .child(text),
            )
            .child(if show_spinner {
                div()
                    .text_size(px(32.0))
                    .text_color(rgb(ACCENT))
                    .child("⏳")
                    .into_any_element()
            } else {
                div().into_any_element()
            })
            .into_any_element()
    }

    /// QR code screen: image + status + hint
    fn render_qr_screen(status: &str, qr_path: &Option<PathBuf>) -> AnyElement {
        let status = status.to_string();
        div()
            .flex()
            .flex_col()
            .size_full()
            .items_center()
            .justify_center()
            .gap_4()
            .px_4()
            .child(
                div()
                    .w(px(220.0))
                    .h(px(220.0))
                    .bg(rgb(0xffffff))
                    .rounded_lg()
                    .border_1()
                    .border_color(rgb(0x333366))
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(if let Some(ref path) = qr_path {
                        div()
                            .w(px(200.0))
                            .h(px(200.0))
                            .bg(rgb(0xffffff))
                            .child(
                                img(path.clone())
                                    .object_fit(ObjectFit::Contain),
                            )
                            .into_any_element()
                    } else {
                        div()
                            .text_size(px(14.0))
                            .text_color(rgb(0x000000))
                            .child("加载中...")
                            .into_any_element()
                    }),
            )
            .child(
                div()
                    .text_size(px(16.0))
                    .text_color(rgb(TEXT_PRIMARY))
                    .text_align(TextAlign::Center)
                    .child(status),
            )
            .child(
                div()
                    .text_size(px(13.0))
                    .text_color(rgb(TEXT_SECONDARY))
                    .child("二维码仅限微博手机客户端扫描"),
            )
            .child(
                div()
                    .text_size(px(13.0))
                    .text_color(rgb(TEXT_SECONDARY))
                    .child("打开微博 App → 扫一扫 → 确认登录"),
            )
            .into_any_element()
    }

    /// Home timeline display
    fn render_timeline(title: &str, statuses: &[TimelineItem]) -> AnyElement {
        div()
            .flex()
            .flex_col()
            .size_full()
            .px_3()
            .py_3()
            .gap_3()
            .child(
                div()
                    .px_2()
                    .py_2()
                    .child(
                        div()
                            .text_size(px(18.0))
                            .font_weight(FontWeight::BOLD)
                            .text_color(rgb(ACCENT))
                            .child(title.to_string()),
                    ),
            )
            .children(
                statuses
                    .iter()
                    .map(|item| {
                        div()
                            .flex()
                            .flex_col()
                            .bg(rgb(CARD_BG))
                            .rounded_lg()
                            .px_4()
                            .py_3()
                            .gap_1()
                            .child(
                                div()
                                    .text_size(px(14.0))
                                    .font_weight(FontWeight::BOLD)
                                    .text_color(rgb(ACCENT))
                                    .child(item.user_name.clone()),
                            )
                            .child(
                                div()
                                    .text_size(px(13.0))
                                    .text_color(rgb(TEXT_PRIMARY))
                                    .line_height(relative(1.6))
                                    .child(item.text.clone()),
                            )
                            .into_any_element()
                    }),
            )
            .into_any_element()
    }

    /// Error screen with retry hint
    fn render_error(message: &str) -> AnyElement {
        let message = message.to_string();
        div()
            .flex()
            .flex_col()
            .size_full()
            .items_center()
            .justify_center()
            .gap_4()
            .child(div().text_size(px(48.0)).child("❌"))
            .child(
                div()
                    .text_size(px(16.0))
                    .text_color(rgb(0xff6b6b))
                    .child(message),
            )
            .child(
                div()
                    .text_size(px(13.0))
                    .text_color(rgb(TEXT_SECONDARY))
                    .child("请查看终端窗口了解详细错误信息"),
            )
            .child(
                div()
                    .text_size(px(12.0))
                    .text_color(rgb(TEXT_SECONDARY))
                    .child("关闭窗口后重新运行 cargo run 重试"),
            )
            .into_any_element()
    }
}
