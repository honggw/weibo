//! 微博 (weibo.com) PC 客户端
//!
//! Architecture (MVVM per REFACTOR.md):
//!   viewmodel/ (state machines) → model/ (services) → infra/ (http, io) → domain/ (models)
//!   view/ (pure GPUI rendering) reads domain state
//!
//! Usage:
//!   cargo run                  → GPUI 图形界面 (默认)
//!   cargo run -- http          → 终端 QR 登录
//!   cargo run -- cookie        → 终端 Cookie 登录

mod cli;
mod domain;
mod infra;
mod legacy;
#[macro_use]
mod logger;
mod model;
mod qr_login;
mod view;
mod viewmodel;

fn main() {
    // Initialize rustls crypto provider (required for WebSocket TLS)
    let _ = rustls::crypto::CryptoProvider::install_default(
        rustls::crypto::aws_lc_rs::default_provider()
    );
    let args: Vec<String> = std::env::args().collect();

    match args.get(1).map(|s| s.as_str()) {
        Some("cookie") => {
            let rt = tokio::runtime::Runtime::new().expect("tokio runtime");
            rt.block_on(async {
                if let Err(e) = cli::cookie_login::run().await {
                    eprintln!("错误: {}", e);
                }
            });
        }
        Some("http") => {
            let rt = tokio::runtime::Runtime::new().expect("tokio runtime");
            rt.block_on(async {
                if let Err(e) = cli::qr_login::run().await {
                    eprintln!("错误: {}", e);
                }
            });
        }
        _ => {
            // GPUI graphical mode
            let tokio_rt = Box::leak(Box::new(
                tokio::runtime::Builder::new_multi_thread()
                    .enable_all()
                    .build()
                    .expect("failed to create tokio runtime"),
            ));
            let tokio_handle = tokio_rt.handle().clone();

            // Install panic hook for crash diagnostics
            std::panic::set_hook(Box::new(|info| {
                let msg = if let Some(s) = info.payload().downcast_ref::<&str>() {
                    s.to_string()
                } else if let Some(s) = info.payload().downcast_ref::<String>() {
                    s.clone()
                } else {
                    "unknown panic".to_string()
                };
                let location = info.location().map(|l| format!("{}:{}", l.file(), l.line())).unwrap_or_default();
                log_error!("!!! PANIC at {}: {}", location, msg);
                eprintln!("!!! PANIC at {}: {}", location, msg);
            }));

            view::app_shell::run(tokio_handle);
        }
    }
}
