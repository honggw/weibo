//! Terminal-based QR login mode.
//! Usage: `cargo run -- http`

use anyhow::Result;
use std::io::{self, Write};
use std::time::Duration;

use crate::model::auth_service;
use crate::model::timeline_service;
use crate::qr_login::QrStatus;
use crate::{log_info, log_success};

pub async fn run() -> Result<()> {
    println!();
    println!("{}", "=".repeat(50));
    println!("  微博 QR 码扫码登录 (终端模式)");
    println!("{}", "=".repeat(50));
    println!();

    // --- Prepare QR ---
    print!("  正在连接微博...");
    io::stdout().flush().ok();
    let (mut login, png_bytes) = auth_service::prepare_qr().await?;
    println!(" OK");

    // Save QR to file and open
    let qr_path = std::path::Path::new("weibo_qr.png");
    std::fs::write(qr_path, &png_bytes)?;
    open::that(qr_path).ok();
    println!("  ✅ 二维码已保存到: {}", qr_path.display());
    println!();
    println!("  📱 请用微博手机客户端扫描二维码");
    println!();
    print!("  {} 等待扫码...", "⏳");

    // --- Poll loop ---
    let start = std::time::Instant::now();
    let timeout = Duration::from_secs(300);
    let mut last_status = String::new();

    let cookie = loop {
        if start.elapsed() > timeout {
            println!();
            println!("  ⏰ 超时：5 分钟内未完成扫码");
            return Ok(());
        }

        match auth_service::poll_qr(&login).await {
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

                match auth_service::exchange_ticket(&mut login, &alt, &redirect_url).await {
                    Ok(c) => {
                        log_success!("登录成功");
                        break c;
                    }
                    Err(e) => {
                        println!("  ❌ 登录失败: {}", e);
                        return Ok(());
                    }
                }
            }
            Ok(QrStatus::Expired) => {
                println!();
                println!("  ⚠️ 二维码已过期，重新获取...");
                let (new_login, new_bytes) = auth_service::prepare_qr().await?;
                login = new_login;
                std::fs::write(qr_path, &new_bytes)?;
                open::that(qr_path).ok();
                println!("  ✅ 新的二维码已生成");
                last_status = String::new();
            }
            Ok(other) => {
                log_info!("QR poll: {:?}", other);
                tokio::time::sleep(Duration::from_secs(2)).await;
            }
            Err(e) => {
                eprintln!("  ❌ 轮询错误: {}", e);
                tokio::time::sleep(Duration::from_secs(2)).await;
            }
        }
    };

    // --- Show home ---
    let (items, title, _, _) = timeline_service::fetch_first_page().await;
    println!();
    println!("{}", "=".repeat(50));
    println!("  {}", title);
    println!("{}", "=".repeat(50));
    for (i, item) in items.iter().take(10).enumerate() {
        let short = if item.text.len() > 70 { &item.text[..70] } else { &item.text };
        println!("  [{}/{}] {}: {}", i + 1, items.len(), item.user_name, short);
    }

    let _ = std::fs::remove_file(qr_path);
    Ok(())
}
