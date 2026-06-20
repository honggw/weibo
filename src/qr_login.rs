//! 微博二维码扫码登录模块 — 纯 HTTP 实现
//!
//! 完整流程: warmup → bot_detect(rid) → QR获取 → 轮询 → 票据交换 → SUB cookie

use anyhow::{Context, Result};
use reqwest::header::{HeaderValue, ACCEPT, REFERER, USER_AGENT};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;
use std::time::Duration;

const UA: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 \
                  (KHTML, like Gecko) Chrome/148.0.0.0 Safari/537.36";
const QR_IMAGE_URL: &str = "https://passport.weibo.com/sso/v2/qrcode/image";
const QR_CHECK_URL: &str = "https://passport.weibo.com/sso/v2/qrcode/check";
const LOGIN_REFERER: &str = "https://passport.weibo.com/sso/signin?entry=miniblog";

#[derive(Debug, Clone, PartialEq)]
pub enum QrStatus {
    Waiting,
    Scanned,
    Confirmed { alt: String, redirect_url: String },
    Expired,
    Unknown { code: i64, msg: String, raw: serde_json::Value },
}

#[derive(Debug, Deserialize)]
struct QrImageResponse { retcode: i64, data: Option<QrImageData> }
#[derive(Debug, Deserialize)]
struct QrImageData { qrid: String, image: String }
#[derive(Debug, Deserialize)]
struct QrCheckResponse { retcode: i64, msg: Option<String>, data: Option<serde_json::Value> }

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoredCookie { pub name: String, pub value: String, pub domain: String, pub path: String }
#[derive(Debug, Serialize, Deserialize)]
pub struct CookieStore { pub cookies: Vec<StoredCookie> }

pub struct QrLogin {
    client: reqwest::Client,
    qrid: Option<String>,
    image_url: Option<String>,
    image_bytes: Option<Vec<u8>>,
    rid: Option<String>,
    cookie_jar: HashMap<String, String>,
}

impl QrLogin {
    pub fn new() -> Result<Self> {
        let client = reqwest::Client::builder()
            .cookie_store(false)
            .timeout(Duration::from_secs(30))
            .build()?;
        Ok(Self { client, qrid: None, image_url: None, image_bytes: None, rid: None, cookie_jar: HashMap::new() })
    }

    pub async fn warmup(&mut self) -> Result<()> {
        let c = reqwest::Client::builder().redirect(reqwest::redirect::Policy::none()).cookie_store(false).build()?;
        let resp = c.get("https://passport.weibo.com/sso/signin")
            .query(&[("entry","miniblog"),("r","https://weibo.com/")])
            .header(ACCEPT, "text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8")
            .send().await?;
        for ck in resp.cookies() {
            self.cookie_jar.insert(ck.name().to_string(), ck.value().to_string());
        }
        eprintln!("  [warmup] {} ({} cookies)", resp.status(), self.cookie_jar.len());
        Ok(())
    }

    pub async fn fetch_qr_code(&mut self) -> Result<()> {
        let resp = self.client.get(QR_IMAGE_URL)
            .query(&[("entry","miniblog"),("size","180")])
            .header(REFERER, HeaderValue::from_static(LOGIN_REFERER))
            .send().await.context("获取二维码失败")?;
        let body: QrImageResponse = resp.json().await?;
        if body.retcode != 20000000 { anyhow::bail!("二维码 retcode={}", body.retcode); }
        let data = body.data.context("无 data")?;
        self.qrid = Some(data.qrid.clone());
        self.image_url = Some(data.image.clone());
        self.image_bytes = None;
        Ok(())
    }

    pub async fn download_qr_image(&mut self) -> Result<()> {
        let url = self.image_url.as_ref().context("先调 fetch_qr_code")?.clone();
        let bytes = self.client.get(&url).header(REFERER, HeaderValue::from_static(LOGIN_REFERER))
            .send().await?.bytes().await?;
        self.image_bytes = Some(bytes.to_vec());
        Ok(())
    }

    pub fn save_qr_image(&self, path: &Path) -> Result<()> {
        std::fs::write(path, self.image_bytes.as_ref().context("先调 download_qr_image")?)?;
        let _ = open::that(path);
        Ok(())
    }

    pub fn set_rid(&mut self, rid: String) { self.rid = Some(rid); }
    pub fn client(&self) -> &reqwest::Client { &self.client }

    pub async fn poll_status(&self) -> Result<QrStatus> {
        let qrid = self.qrid.as_ref().context("先调 fetch_qr_code")?;
        let mut params: Vec<(&str, &str)> = vec![("entry","miniblog"), ("qrid",qrid), ("ver","20250520")];
        let rid_str;
        if let Some(ref r) = self.rid { rid_str = r.clone(); params.push(("rid", &rid_str)); }

        let resp = self.client.get(QR_CHECK_URL).query(&params)
            .header(REFERER, HeaderValue::from_static(LOGIN_REFERER))
            .send().await?;
        let body: QrCheckResponse = resp.json().await?;

        Ok(match body.retcode {
            50114001 => QrStatus::Waiting,
            50114002 => QrStatus::Scanned,
            50114004 => QrStatus::Expired,
            20000000 => {
                let d = body.data.as_ref();
                let alt = d.and_then(|v| v.get("alt")).and_then(|v| v.as_str()).unwrap_or("").into();
                let url = d.and_then(|v| v.get("url")).and_then(|v| v.as_str()).unwrap_or("").into();
                if let Some(dd) = d { eprintln!("  [qr check confirmed] data: {}", serde_json::to_string(dd).unwrap_or_default()); }
                QrStatus::Confirmed { alt, redirect_url: url }
            }
            code => QrStatus::Unknown { code, msg: body.msg.unwrap_or_default(), raw: body.data.unwrap_or(serde_json::Value::Null) }
        })
    }

    pub async fn exchange_ticket_with_url(&mut self, _alt: &str, redirect_url: &str) -> Result<()> {
        let mut url = if !redirect_url.is_empty() { redirect_url.to_string() }
            else { return Err(anyhow::anyhow!("redirect_url 为空")); };

        let no_redir = reqwest::Client::builder().redirect(reqwest::redirect::Policy::none()).cookie_store(false).build()?;
        let mut cookie_header = self.cookie_header();

        for hop in 0..12 {
            let mut req = no_redir.get(&url)
                .header(REFERER, HeaderValue::from_static(LOGIN_REFERER))
                .header(ACCEPT, "text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8")
                .header("Accept-Language", "zh-CN")
                .header("Sec-Fetch-Mode", "navigate")
                .header("Sec-Fetch-Site", "same-origin")
                .header("Sec-Fetch-Dest", "document")
                .header("Sec-Fetch-User", "?1")
                .header("Upgrade-Insecure-Requests", "1")
                .header("Sec-CH-UA", r#""Not/A)Brand";v="99", "Chromium";v="148""#)
                .header("Sec-CH-UA-Mobile", "?0")
                .header("Sec-CH-UA-Platform", "Windows");
            if !cookie_header.is_empty() { req = req.header("Cookie", &cookie_header); }

            let resp = req.send().await?;
            let status = resp.status();

            // Collect Set-Cookie
            let mut new_cookies = Vec::new();
            for c in resp.cookies() {
                if c.value().is_empty() || c.value() == "deleted" { self.cookie_jar.remove(c.name()); continue; }
                self.cookie_jar.entry(c.name().into()).or_insert(c.value().into());
                new_cookies.push(format!("{}={}", c.name(), c.value()));
            }
            cookie_header = self.cookie_header();

            let has_sub = new_cookies.iter().any(|c| c.starts_with("SUB="));
            eprintln!("  [hop {}] {} {} cookies={}{}", hop, status.as_u16(), &url[..url.len().min(80)], new_cookies.len(),
                if has_sub { " [GOT SUB!]" } else { "" });

            if status.is_redirection() {
                if let Some(loc) = resp.headers().get("location") {
                    url = resolve_url(&url, loc.to_str()?)?;
                    continue;
                }
                break;
            }
            break;
        }
        eprintln!("  [cookie jar] total {} cookies", self.cookie_jar.len());
        Ok(())
    }

    fn cookie_header(&self) -> String {
        let mut parts = Vec::new();
        if let Some(v) = self.cookie_jar.get("SUB") { parts.push(format!("SUB={}", v)); }
        if let Some(v) = self.cookie_jar.get("SUBP") { parts.push(format!("SUBP={}", v)); }
        parts.join("; ")
    }

    pub async fn verify_login(&self) -> Result<bool> {
        let ch = self.cookie_header();
        eprintln!("  [verify] cookie: {}", &ch[..ch.len().min(200)]);
        let c = reqwest::Client::builder().cookie_store(false).build()?;
        let resp = c.get("https://weibo.com/ajax/config/get_config")
            .header("Cookie", &ch)
            .header(REFERER, HeaderValue::from_static("https://weibo.com/"))
            .header("X-Requested-With", HeaderValue::from_static("XMLHttpRequest"))
            .header(ACCEPT, HeaderValue::from_static("application/json, text/plain, */*"))
            .header(USER_AGENT, HeaderValue::from_static(UA))
            .header("Accept-Language", "zh-CN")
            .header("Sec-CH-UA", r#""Not/A)Brand";v="99", "Chromium";v="148""#)
            .header("Sec-CH-UA-Mobile", "?0")
            .header("Sec-CH-UA-Platform", "Windows")
            .send().await?;
        let data: serde_json::Value = resp.json().await?;
        let ok = data.get("ok").and_then(|v| v.as_i64()).unwrap_or(0);
        eprintln!("  [verify] ok={}", ok);
        Ok(ok == 1)
    }

    pub fn save_cookies_to_file(&self, path: &Path) -> Result<()> {
        let cookies: Vec<StoredCookie> = self.cookie_jar.iter().map(|(k,v)| StoredCookie {
            name: k.clone(), value: v.clone(), domain: "weibo.com".into(), path: "/".into(),
        }).collect();
        serde_json::to_string_pretty(&CookieStore { cookies }).map(|j| std::fs::write(path, j))??;
        Ok(())
    }
}

fn resolve_url(base: &str, location: &str) -> Result<String> {
    if location.starts_with("http") { Ok(location.into()) }
    else { Ok(url::Url::parse(base)?.join(location)?.to_string()) }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test] async fn test_fetch_qr_code() {
        let mut l = QrLogin::new().unwrap();
        l.fetch_qr_code().await.unwrap();
        assert!(l.qrid.is_some());
    }
    #[tokio::test] async fn test_poll_waiting() {
        let mut l = QrLogin::new().unwrap();
        l.fetch_qr_code().await.unwrap();
        let s = l.poll_status().await.unwrap();
        assert!(matches!(s, QrStatus::Waiting));
    }
}
