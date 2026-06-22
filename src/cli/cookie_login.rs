//! Terminal-based cookie login mode.
//! Usage: `cargo run -- cookie`

use anyhow::Result;
use std::io::{self, Write};

use crate::infra::config;
use crate::model::timeline_service;

const DEFAULT_UA: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 \
                           (KHTML, like Gecko) Chrome/148.0.0.0 Safari/537.36";

pub async fn run() -> Result<()> {
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
    client
        .get(config::WEIBO_BASE_URL)
        .header("Cookie", &cookie_header)
        .header("User-Agent", DEFAULT_UA)
        .send()
        .await?;

    // Verify
    let resp = client
        .get(config::API_CONFIG)
        .header("Cookie", &cookie_header)
        .header("Referer", config::WEIBO_BASE_URL)
        .header("User-Agent", DEFAULT_UA)
        .header("X-Requested-With", "XMLHttpRequest")
        .send()
        .await?;

    let data: serde_json::Value = resp.json().await?;
    let ok = data.get("ok").and_then(|v| v.as_i64()).unwrap_or(0) == 1;

    if ok {
        println!("[OK] 登录成功!");
        let (items, title) = timeline_service::fetch_home_content(&cookie_header).await;
        println!();
        println!("{}", "=".repeat(50));
        println!("  {}", title);
        println!("{}", "=".repeat(50));
        for (i, item) in items.iter().take(10).enumerate() {
            let short = if item.text.len() > 70 { &item.text[..70] } else { &item.text };
            println!("  [{}/{}] {}: {}", i + 1, items.len(), item.user_name, short);
        }
    } else {
        println!("[FAIL] 未登录，请检查 Cookie");
    }

    Ok(())
}
