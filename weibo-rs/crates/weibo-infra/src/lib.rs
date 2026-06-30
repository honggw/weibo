//! weibo-infra — 基础设施层 (HTTP, WebSocket, Cookie 持久化, Audio, Config, Logger)
//!
//! 依赖: weibo-domain, reqwest, tokio-tungstenite, rodio, serde_json, anyhow

pub mod audio;
pub mod config;
pub mod cookie_io;
pub mod http_client;
pub mod logger;
pub mod ws_client;
