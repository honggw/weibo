//! WebView 辅助登录模块
//!
//! 最终方案: WebView 负责完整登录流程 (处理 wbBotDetector + JS 生成 SUB)
//! 然后通过 document.cookie 获取非 httpOnly cookies，
//! 其余 httpOnly cookies 通过 WebView 内 fetch 自动携带。

use anyhow::{Context, Result};
use std::sync::mpsc;
use wry::WebViewBuilder;
use tao::event_loop::{EventLoopBuilder, ControlFlow, EventLoopProxy};
use tao::platform::run_return::EventLoopExtRunReturn;
use tao::platform::windows::EventLoopBuilderExtWindows;
use tao::window::WindowBuilder;
use tao::event::{Event, WindowEvent};

const WEIBO_LOGIN_URL: &str =
    "https://passport.weibo.com/sso/signin?entry=miniblog&r=https%3A%2F%2Fweibo.com%2F";

const UA: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 \
                  (KHTML, like Gecko) Chrome/148.0.0.0 Safari/537.36";

/// 检测登录完成，发送 IPC
const LOGIN_DETECT: &str = r#"
(function() {
    if (window.location.hostname === 'weibo.com' &&
        !window.location.pathname.includes('login') &&
        !window.location.pathname.includes('passport')) {
        window.ipc.postMessage(JSON.stringify({
            type: 'logged_in',
            cookies: document.cookie,
            url: window.location.href
        }));
    }
})();
"#;

/// 在 weibo.com 页面执行 API 调用，通过 IPC 返回结果
const FETCH_TIMELINE: &str = r#"
(async function() {
    try {
        const r = await fetch('/ajax/statuses/home_timeline?page=1&feature=0', {
            headers: {'X-Requested-With': 'XMLHttpRequest', 'Referer': 'https://weibo.com/'},
            credentials: 'include'
        });
        const d = await r.json();
        window.ipc.postMessage(JSON.stringify({type:'api_result', ok: d.ok, data: JSON.stringify(d).substring(0,300)}));
    } catch(e) {
        window.ipc.postMessage(JSON.stringify({type:'api_result', ok: false, error: e.toString()}));
    }
})();
"#;

#[derive(Debug, Clone)]
pub struct LoginResult {
    pub cookies: String,
    pub api_ok: bool,
}

/// WebView 完整登录流程
pub fn run() -> Result<LoginResult> {
    let (tx, rx) = mpsc::channel::<LoginResult>();

    let mut event_loop = EventLoopBuilder::<String>::with_user_event()
        .with_any_thread(true)
        .build();
    let proxy: EventLoopProxy<String> = event_loop.create_proxy();

    let window = WindowBuilder::new()
        .with_title("微博登录 - 请扫码")
        .with_inner_size(tao::dpi::LogicalSize::new(480.0, 700.0))
        .build(&event_loop)?;

    let tx_ipc = tx.clone();
    let proxy_ipc = proxy.clone();
    let mut logged_in = false;

    let _webview = WebViewBuilder::new()
        .with_user_agent(UA)
        .with_url(WEIBO_LOGIN_URL)
        .with_initialization_script(LOGIN_DETECT)
        .with_ipc_handler(move |request| {
            if let Ok(msg) = serde_json::from_str::<serde_json::Value>(request.body()) {
                match msg.get("type").and_then(|v| v.as_str()) {
                    Some("logged_in") => {
                        let cookies = msg.get("cookies").and_then(|v| v.as_str()).unwrap_or("").to_string();
                        // 稍后在这里注入 API 调用
                        let _ = tx_ipc.send(LoginResult { cookies, api_ok: false });
                        let _ = proxy_ipc.send_event("logged_in".into());
                    }
                    Some("api_result") => {
                        let ok = msg.get("ok").and_then(|v| v.as_i64()).unwrap_or(0) == 1;
                        let _ = tx_ipc.send(LoginResult { cookies: String::new(), api_ok: ok });
                        let _ = proxy_ipc.send_event("done".into());
                    }
                    _ => {}
                }
            }
        })
        .build(&window)?;

    let tx_close = tx.clone();
    let proxy_close = proxy.clone();

    event_loop.run_return(move |event, _, control_flow| {
        *control_flow = ControlFlow::Wait;

        match event {
            Event::WindowEvent { event: WindowEvent::CloseRequested, .. } => {
                let _ = tx_close.send(LoginResult { cookies: String::new(), api_ok: false });
                let _ = proxy_close.send_event("done".into());
            }
            Event::UserEvent(msg) => {
                if msg == "done" {
                    *control_flow = ControlFlow::Exit;
                } else if msg == "logged_in" {
                    // 注入 API 验证
                    let _ = _webview.evaluate_script(FETCH_TIMELINE);
                }
            }
            _ => {}
        }
    });

    // 取第一个结果 (logged_in 或 close)
    let first = rx.recv_timeout(std::time::Duration::from_secs(300))
        .context("登录超时 5 分钟")?;

    // 如果 logged_in 触发了，等待 api_result
    if !first.cookies.is_empty() {
        // 等待第二个结果 (api_result)
        match rx.recv_timeout(std::time::Duration::from_secs(15)) {
            Ok(second) => Ok(second),
            Err(_) => Ok(first),  // API 验证超时但登录成功
        }
    } else {
        Ok(first)
    }
}
