//! 微博 (weibo.com) PC 客户端工具
//!
//! 支持:
//!   1. WebView 扫码登录 (推荐 — 可获取完整 Cookie 含 SUB)
//!   2. 纯 HTTP QR 登录 (轻量，但可能缺少 SUB cookie)
//!   3. Cookie 手动输入
//!
//! 用法:
//!   cargo run                  → WebView 扫码登录
//!   cargo run -- http          → 纯 HTTP QR 登录
//!   cargo run -- cookie        → Cookie 登录模式

mod bot_detector;
mod qr_login;
mod webview_login;

use anyhow::Result;
use reqwest::header::{HeaderMap, HeaderValue, REFERER, USER_AGENT};
use std::io::{self, Write};
use std::path::Path;
use std::time::Duration;

const WEIBO_URL: &str = "https://weibo.com";
const DEFAULT_UA: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 \
                           (KHTML, like Gecko) Chrome/148.0.0.0 Safari/537.36";

// ============================================================================
// 工具函数
// ============================================================================

fn api_headers(referer: &str) -> HeaderMap {
    let mut h = HeaderMap::new();
    h.insert(USER_AGENT, HeaderValue::from_static(DEFAULT_UA));
    h.insert(REFERER, HeaderValue::from_str(referer).unwrap());
    h.insert("Accept", HeaderValue::from_static("application/json, text/plain, */*"));
    h.insert("X-Requested-With", HeaderValue::from_static("XMLHttpRequest"));
    h
}

async fn show_hot_search(client: &reqwest::Client) {
    match client
        .get(format!("{}/ajax/side/hotSearch", WEIBO_URL))
        .headers(api_headers(WEIBO_URL))
        .send()
        .await
    {
        Ok(resp) => {
            if let Ok(data) = resp.json::<serde_json::Value>().await {
                let empty = vec![];
                let items = data
                    .get("data")
                    .and_then(|d| d.get("band_list"))
                    .and_then(|b| b.as_array())
                    .unwrap_or(&empty);
                println!();
                println!("{}", "=".repeat(50));
                println!("  🔥 热搜榜");
                println!("{}", "=".repeat(50));
                for (i, item) in items.iter().take(15).enumerate() {
                    let word = item.get("word").and_then(|v| v.as_str()).unwrap_or("?");
                    let num = item.get("num").and_then(|v| v.as_i64()).unwrap_or(0);
                    let hot = item.get("category").and_then(|v| v.as_str()).unwrap_or("");
                    if num > 0 {
                        println!("  {:>2}. {}  (热度 {})", i + 1, word, num);
                    } else if !hot.is_empty() {
                        println!("  {:>2}. {}  [{}]", i + 1, word, hot);
                    } else {
                        println!("  {:>2}. {}", i + 1, word);
                    }
                }
            }
        }
        Err(e) => eprintln!("获取热搜失败: {}", e),
    }
}

async fn show_home_timeline(client: &reqwest::Client) {
    match client
        .get(format!(
            "{}/ajax/statuses/home_timeline?page=1&feature=0",
            WEIBO_URL
        ))
        .headers(api_headers(WEIBO_URL))
        .send()
        .await
    {
        Ok(resp) => {
            if let Ok(data) = resp.json::<serde_json::Value>().await {
                let empty = vec![];
                let statuses = data
                    .get("data")
                    .and_then(|d| d.get("statuses"))
                    .and_then(|s| s.as_array())
                    .unwrap_or(&empty);
                println!();
                println!("{}", "=".repeat(50));
                println!("  📰 首页微博 ({})", statuses.len());
                println!("{}", "=".repeat(50));
                for (i, s) in statuses.iter().take(10).enumerate() {
                    let user = s
                        .get("user")
                        .and_then(|u| u.get("screen_name"))
                        .and_then(|v| v.as_str())
                        .unwrap_or("?");
                    let text = s
                        .get("text_raw")
                        .or_else(|| s.get("text"))
                        .and_then(|v| v.as_str())
                        .unwrap_or("");
                    let short = if text.len() > 70 { &text[..70] } else { text };
                    println!("  [{}/{}] {}: {}", i + 1, statuses.len(), user, short);
                }
            }
        }
        Err(e) => eprintln!("获取首页失败: {}", e),
    }
}

// ============================================================================
// QR 码扫码登录
// ============================================================================

async fn qr_login_mode() -> Result<()> {
    use qr_login::{QrLogin, QrStatus};

    println!();
    println!("{}", "=".repeat(50));
    println!("  微博 QR 码扫码登录");
    println!("{}", "=".repeat(50));
    println!();
    println!("  正在连接微博...");

    let mut login = QrLogin::new()?;
    login.warmup().await?;
    login.fetch_qr_code().await?;
    login.download_qr_image().await?;

    let qr_path = Path::new("weibo_qr.png");
    login.save_qr_image(qr_path)?;

    println!("  ✅ 二维码已保存到: {}", qr_path.display());
    println!("     图片查看器已打开（如未自动打开请手动双击）");
    println!();
    println!("  📱 请用微博手机客户端扫描二维码");
    println!();
    println!("  {} 等待扫码...", "⏳");

    let start = std::time::Instant::now();
    let timeout = Duration::from_secs(300);
    let mut last_status = String::new();

    loop {
        if start.elapsed() > timeout {
            println!();
            println!("  ⏰ 超时：5 分钟内未完成扫码");
            break;
        }

        match login.poll_status().await {
            Ok(QrStatus::Waiting) => {
                if last_status != "waiting" {
                    print!(".");
                    io::stdout().flush().ok();
                }
                last_status = "waiting".into();
                tokio::time::sleep(Duration::from_secs(2)).await;
            }
            Ok(QrStatus::Scanned) => {
                if last_status != "scanned" {
                    println!();
                    println!("  📲 已扫描！请在手机上点击「确认登录」");
                }
                last_status = "scanned".into();
                tokio::time::sleep(Duration::from_secs(2)).await;
            }
            Ok(QrStatus::Confirmed { alt, redirect_url }) => {
                println!();
                println!("  ✅ 确认成功！正在获取登录票据...");
                login.exchange_ticket_with_url(&alt, &redirect_url).await?;

                // 验证登录
                match login.verify_login().await {
                    Ok(true) => {
                        println!("  🎉 登录成功！");
                        login.save_cookies_to_file(Path::new("weibo_cookies.json"))?;
                        println!("  💾 Cookies 已保存到 weibo_cookies.json");
                        println!();

                        // 展示热点
                        show_hot_search(login.client()).await;
                        show_home_timeline(login.client()).await;

                        // 清理 QR 图片
                        let _ = std::fs::remove_file(qr_path);
                        return Ok(());
                    }
                    Ok(false) => {
                        println!("  ❌ 登录验证失败，Cookie 可能无效");
                    }
                    Err(e) => {
                        println!("  ❌ 验证登录时出错: {}", e);
                    }
                }
                break;
            }
            Ok(QrStatus::Expired) => {
                println!();
                println!("  ⚠️ 二维码已过期，重新获取...");
                login.fetch_qr_code().await?;
                login.download_qr_image().await?;
                login.save_qr_image(qr_path)?;
                println!("  ✅ 新的二维码已生成");
                last_status = String::new();
            }
            Ok(QrStatus::Unknown { code, msg, .. }) => {
                eprintln!("  ⚠️ 未知状态: {} {} (继续等待...)", code, msg);
                tokio::time::sleep(Duration::from_secs(2)).await;
            }
            Err(e) => {
                eprintln!("  ❌ 轮询错误: {}", e);
                tokio::time::sleep(Duration::from_secs(2)).await;
            }
        }
    }

    Ok(())
}

// ============================================================================
// Cookie 登录模式 (备用)
// ============================================================================

async fn cookie_login_mode() -> Result<()> {
    println!("微博 Cookie 登录");
    println!("{}", "=".repeat(50));
    println!();
    println!("请先在浏览器中登录 https://weibo.com/");
    println!("然后获取 Cookie:");
    println!("  F12 -> Application -> Cookies -> weibo.com -> 复制 SUB 的值");
    println!();

    print!("粘贴 Cookie (SUB值 或完整Cookie字符串): ");
    io::stdout().flush().unwrap();
    let mut cookie_input = String::new();
    io::stdin().read_line(&mut cookie_input)?;
    let cookie_input = cookie_input.trim();

    if cookie_input.is_empty() {
        println!("未输入 Cookie");
        return Ok(());
    }

    let cookie_header = if cookie_input.contains('=') {
        cookie_input.to_string()
    } else {
        format!("SUB={}", cookie_input)
    };

    let client = reqwest::Client::builder().cookie_store(true).build()?;
    // 通过请求来 set cookie
    client
        .get(WEIBO_URL)
        .header("Cookie", &cookie_header)
        .header(USER_AGENT, HeaderValue::from_static(DEFAULT_UA))
        .send()
        .await?;

    // 验证
    let resp = client
        .get(format!(
            "{}/ajax/statuses/home_timeline?page=1&feature=0",
            WEIBO_URL
        ))
        .headers(api_headers(WEIBO_URL))
        .send()
        .await?;
    let data: serde_json::Value = resp.json().await?;
    let ok = data.get("ok").and_then(|v| v.as_i64()).unwrap_or(0) == 1;

    if ok {
        println!("[OK] 登录成功!");
        show_hot_search(&client).await;
        show_home_timeline(&client).await;
    } else {
        println!("[FAIL] 未登录，请检查 Cookie");
    }

    Ok(())
}

// ============================================================================
// 入口
// ============================================================================

// ============================================================================
// WebView 登录模式
// ============================================================================

async fn hybrid_login_mode() -> Result<()> {
    use qr_login::{QrLogin, QrStatus};
    use bot_detector;

    println!();
    println!("{}", "=".repeat(50));
    println!("  微博纯 Rust QR 码登录");
    println!("{}", "=".repeat(50));
    println!();

    // Step 1: 获取 rid
    println!("  [1/4] 获取风控参数 (rid)...");
    let mut login = QrLogin::new()?;
    login.warmup().await?;
    let rid = bot_detector::get_rid(login.client()).await?;
    println!("  [OK] rid={}", rid);
    login.set_rid(rid);

    // Step 2: 获取 QR 码
    println!("  [2/4] 获取二维码...");
    login.fetch_qr_code().await?;
    login.download_qr_image().await?;
    let qr_path = std::path::Path::new("weibo_qr.png");
    login.save_qr_image(qr_path)?;
    println!("  [OK] 二维码: {}", qr_path.display());

    // Step 3: 等待扫码
    println!();
    println!("  [3/4] 请用微博手机客户端扫描二维码...");
    println!("  📱 等待扫码确认 (最多 5 分钟)");

    let start = std::time::Instant::now();
    let timeout = std::time::Duration::from_secs(300);

    loop {
        if start.elapsed() > timeout {
            println!();
            println!("  ⏰ 超时");
            break;
        }

        match login.poll_status().await {
            Ok(QrStatus::Waiting) => {
                print!(".");
                std::io::Write::flush(&mut std::io::stdout()).ok();
                tokio::time::sleep(std::time::Duration::from_secs(2)).await;
            }
            Ok(QrStatus::Scanned) => {
                println!();
                println!("  📲 已扫描！请在手机上点击「确认登录」");
                tokio::time::sleep(std::time::Duration::from_secs(2)).await;
            }
            Ok(QrStatus::Confirmed { alt, redirect_url }) => {
                println!();
                println!("  [4/4] 确认成功！正在交换登录票据...");
                login.exchange_ticket_with_url(&alt, &redirect_url).await?;

                match login.verify_login().await {
                    Ok(true) => {
                        println!("  🎉 登录成功！SUB cookie 已获取");
                        login.save_cookies_to_file(std::path::Path::new("weibo_cookies.json"))?;
                        println!("  💾 Cookies → weibo_cookies.json");
                        show_hot_search(login.client()).await;
                        show_home_timeline(login.client()).await;
                        let _ = std::fs::remove_file(qr_path);
                        return Ok(());
                    }
                    Ok(false) => {
                        println!("  ❌ 登录验证失败 (Cookie 可能无效)");
                    }
                    Err(e) => {
                        println!("  ❌ 验证错误: {}", e);
                    }
                }
                break;
            }
            Ok(QrStatus::Expired) => {
                println!();
                println!("  ⚠️ 二维码过期，重新获取...");
                login.fetch_qr_code().await?;
                login.download_qr_image().await?;
                login.save_qr_image(qr_path)?;
            }
            other => {
                tokio::time::sleep(std::time::Duration::from_secs(2)).await;
            }
        }
    }

    Ok(())
}

// ============================================================================
// 入口
// ============================================================================

#[tokio::main]
async fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().collect();

    match args.get(1).map(|s| s.as_str()) {
        Some("cookie") => cookie_login_mode().await,
        Some("http") => qr_login_mode().await,
        _ => hybrid_login_mode().await,  // 默认: WebView + HTTP 混合
    }
}
