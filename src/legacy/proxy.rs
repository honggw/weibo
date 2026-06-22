//! MITM 代理服务器模块
//!
//! 实现 HTTP 正向代理 + HTTPS CONNECT MITM，捕获全部请求/响应到 JSON 文件。
//!
//! 使用方式:
//!   1. 启动代理: `cargo run -- proxy`
//!   2. 将浏览器代理设置为 127.0.0.1:8888
//!   3. 安装 CA 证书: ca/ca_cert.pem → 系统受信任根证书
//!   4. 操作浏览器登录，代理自动记录所有报文
//!   5. Ctrl+C 停止代理，报文保存到 captured_network.json

use crate::ca::CaManager;
use anyhow::{Context, Result};
use base64::Engine as _;
use rustls::pki_types::CertificateDer;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio_rustls::TlsAcceptor;

/// 代理服务器配置
const PROXY_PORT: u16 = 8888;

/// 捕获的单条 HTTP 请求/响应记录
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CapturedExchange {
    /// ISO 8601 时间戳
    pub timestamp: String,
    /// 请求方法
    pub method: String,
    /// 完整 URL
    pub url: String,
    /// 请求头
    pub request_headers: HashMap<String, String>,
    /// 请求体 (base64)
    pub request_body_base64: Option<String>,
    /// 请求体 (文本)
    pub request_body_text: Option<String>,
    /// 响应状态码
    pub response_status: u16,
    /// 响应状态文本
    pub response_reason: String,
    /// 响应头
    pub response_headers: HashMap<String, String>,
    /// 响应体 (base64)
    pub response_body_base64: Option<String>,
    /// 响应体 (文本)
    pub response_body_text: Option<String>,
    /// 目标主机
    pub host: String,
}

/// 代理捕获日志
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CapturedLog {
    pub total_exchanges: usize,
    pub exchanges: Vec<CapturedExchange>,
}

/// 代理服务器
pub struct ProxyServer {
    captured: Arc<Mutex<Vec<CapturedExchange>>>,
}

impl ProxyServer {
    pub fn new() -> Self {
        Self {
            captured: Arc::new(Mutex::new(Vec::new())),
        }
    }

    /// 启动代理服务器
    pub async fn run(&self) -> Result<()> {
        // 预初始化 CA (打印一次)
        let _ca = CaManager::init()?;

        let addr = format!("127.0.0.1:{}", PROXY_PORT);
        let listener = TcpListener::bind(&addr).await?;

        println!();
        println!("╔══════════════════════════════════════════════════════╗");
        println!("║      Weibo MITM Proxy - 网络报文捕获工具              ║");
        println!("╠══════════════════════════════════════════════════════╣");
        println!("║  代理地址: {}{}║", pad_right(&addr, 40), "");
        println!("║  CA 证书:  {}{}║", pad_right(&_ca.ca_cert_path().display().to_string(), 38), "");
        println!("╠══════════════════════════════════════════════════════╣");
        println!("║  使用步骤:                                           ║");
        println!("║  1. 安装 CA 证书到「受信任的根证书颁发机构」          ║");
        println!("║     Win+R → certlm.msc → 受信任的根证书颁发机构       ║");
        println!("║     → 右键「所有任务」→「导入」→ 选择 ca_cert.pem     ║");
        println!("║  2. 设置浏览器代理为 127.0.0.1:{}                   ║", PROXY_PORT);
        println!("║     Chrome: 设置 → 系统 → 打开计算机的代理设置        ║");
        println!("║  3. 在浏览器中操作微博短信验证码登录                   ║");
        println!("║  4. 按 Ctrl+C 停止，报文自动保存                      ║");
        println!("╚══════════════════════════════════════════════════════╝");
        println!();
        println!("[proxy] 代理已启动，等待浏览器连接...");
        println!();

        let captured = self.captured.clone();

        loop {
            match listener.accept().await {
                Ok((stream, addr)) => {
                    let cap = captured.clone();
                    tokio::spawn(async move {
                        if let Err(e) = handle_client(stream, cap).await {
                            eprintln!("[proxy] 连接 {} 错误: {}", addr, e);
                        }
                    });
                }
                Err(e) => {
                    eprintln!("[proxy] accept 失败: {}", e);
                }
            }
        }
    }

    /// 保存捕获的报文到 JSON 文件
    pub fn save_log(&self, path: &PathBuf) -> Result<()> {
        let captured = self.captured.lock().unwrap();
        let log = CapturedLog {
            total_exchanges: captured.len(),
            exchanges: captured.clone(),
        };
        let json = serde_json::to_string_pretty(&log)?;
        std::fs::write(path, &json)?;
        println!(
            "[proxy] 已保存 {} 条报文到: {}",
            log.total_exchanges,
            path.display()
        );
        Ok(())
    }

    /// 获取捕获计数
    pub fn capture_count(&self) -> usize {
        self.captured.lock().unwrap().len()
    }
}

// ============================================================================
// 连接处理
// ============================================================================

/// 处理一个客户端连接 (HTTP 或 HTTPS CONNECT)
async fn handle_client(
    mut client: TcpStream,
    captured: Arc<Mutex<Vec<CapturedExchange>>>,
) -> Result<()> {
    let mut buf = vec![0u8; 65536];
    let n = client.read(&mut buf).await?;
    if n == 0 {
        return Ok(());
    }
    let head = String::from_utf8_lossy(&buf[..n]);

    if head.starts_with("CONNECT") {
        let request_line = head.lines().next().unwrap_or("");
        let parts: Vec<&str> = request_line.split_whitespace().collect();
        if parts.len() < 2 {
            anyhow::bail!("invalid CONNECT request: {}", request_line);
        }
        let target = parts[1];
        let host_port: Vec<&str> = target.split(':').collect();
        let host = host_port[0];
        let port: u16 = host_port.get(1).and_then(|p| p.parse().ok()).unwrap_or(443);

        // 回复 200 Connection Established
        client
            .write_all(b"HTTP/1.1 200 Connection Established\r\n\r\n")
            .await?;

        if let Err(e) = handle_https_mitm(client, host, port, captured).await {
            eprintln!("[proxy] MITM {} 失败: {}", host, e);
        }
    } else {
        if let Err(e) = handle_http_proxy(client, &buf[..n], captured).await {
            eprintln!("[proxy] HTTP 代理失败: {}", e);
        }
    }

    Ok(())
}

/// 处理普通 HTTP 代理请求
async fn handle_http_proxy(
    mut client: TcpStream,
    initial_data: &[u8],
    captured: Arc<Mutex<Vec<CapturedExchange>>>,
) -> Result<()> {
    let text = String::from_utf8_lossy(initial_data);
    let first_line = text.lines().next().context("empty request")?;
    let parts: Vec<&str> = first_line.split_whitespace().collect();
    if parts.len() < 2 {
        anyhow::bail!("invalid request line: {}", first_line);
    }
    let method = parts[0].to_string();
    let full_url = parts[1].to_string();

    let parsed = url::Url::parse(&full_url)?;
    let host = parsed.host_str().context("no host in URL")?.to_string();
    let port = parsed.port().unwrap_or(80);
    let path = parsed.path().to_string()
        + &parsed.query().map(|q| format!("?{}", q)).unwrap_or_default();

    let (headers, body_start) = parse_http_headers(initial_data)?;
    let request_body = if body_start < initial_data.len() {
        Some(initial_data[body_start..].to_vec())
    } else {
        None
    };

    let mut server = TcpStream::connect(format!("{}:{}", host, port)).await?;

    let mut forward_req = format!("{} {} HTTP/1.1\r\n", method, path);
    for (k, v) in &headers {
        forward_req.push_str(&format!("{}: {}\r\n", k, v));
    }
    forward_req.push_str("\r\n");
    server.write_all(forward_req.as_bytes()).await?;

    if let Some(ref body) = request_body {
        server.write_all(body).await?;
    }

    let mut resp_buf = vec![0u8; 524288];
    let n = server.read(&mut resp_buf).await?;
    let resp_data = &resp_buf[..n];

    if let Some(exchange) = parse_response(
        &method, &full_url, &host, &headers, &request_body, resp_data,
    ) {
        let count = {
            let mut cap = captured.lock().unwrap();
            cap.push(exchange);
            cap.len()
        };
        println!("[capture #{}] {} {} → {} bytes", count, method, full_url, resp_data.len());
    }

    client.write_all(resp_data).await?;
    Ok(())
}

/// 处理 HTTPS CONNECT MITM
async fn handle_https_mitm(
    client: TcpStream,
    host: &str,
    port: u16,
    captured: Arc<Mutex<Vec<CapturedExchange>>>,
) -> Result<()> {
    // 1. 为域名动态生成证书
    let ca = CaManager::init_silent()?;
    let (cert_pem, key_pem) = ca.sign_domain(host)?;

    // 2. 解析证书和私钥
    let certs: Vec<CertificateDer> = rustls_pemfile::certs(&mut cert_pem.as_bytes())
        .collect::<std::result::Result<Vec<_>, _>>()?;
    let private_key = rustls_pemfile::private_key(&mut key_pem.as_bytes())?
        .context("no private key found")?;

    // 3. 构建 TLS server config (用于面向客户端)
    let tls_config = rustls::ServerConfig::builder()
        .with_no_client_auth()
        .with_single_cert(certs, private_key)?;

    let acceptor = TlsAcceptor::from(Arc::new(tls_config));

    // 4. 与客户端建立 TLS
    let tls_client = acceptor.accept(client).await?;

    // 5. 连接到远程服务器
    let server_conn = TcpStream::connect(format!("{}:{}", host, port)).await?;

    // 6. 建立到远程服务器的 TLS (标准客户端 TLS)
    let mut root_store = rustls::RootCertStore::empty();
    for cert in rustls_native_certs::load_native_certs()? {
        root_store.add(cert)?;
    }
    let client_tls_config = rustls::ClientConfig::builder()
        .with_root_certificates(root_store)
        .with_no_client_auth();

    let connector = tokio_rustls::TlsConnector::from(Arc::new(client_tls_config));
    let server_name = rustls::pki_types::ServerName::try_from(host.to_string())
        .map_err(|_| anyhow::anyhow!("invalid server name: {}", host))?;
    let tls_server = connector.connect(server_name, server_conn).await?;

    // 7. 双向转发 + 捕获 HTTP 报文
    relay_and_capture(tls_client, tls_server, host, captured).await?;

    Ok(())
}

/// 在客户端和服务器之间转发数据并捕获 HTTP 报文
///
/// 在 TLS MITM 解密后，这里的 client/server 都是明文流。
/// 循环读取多个 HTTP 请求-响应对直到连接关闭。
async fn relay_and_capture<C, S>(
    mut client: C,
    mut server: S,
    host: &str,
    captured: Arc<Mutex<Vec<CapturedExchange>>>,
) -> Result<()>
where
    C: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin,
    S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin,
{
    // 循环处理多个请求
    let mut req_count = 0u32;
    loop {
        // 读取客户端请求
        let mut buf = vec![0u8; 524288];
        let n = match tokio::time::timeout(
            std::time::Duration::from_secs(30),
            client.read(&mut buf),
        )
        .await
        {
            Ok(Ok(0)) => break,                     // 连接关闭
            Ok(Ok(n)) => n,                          // 成功读取 n > 0 字节
            Ok(Err(e)) => {
                eprintln!("[proxy] 读取客户端请求失败: {}", e);
                break;
            }
            Err(_elapsed) => break,                  // 超时
        };

        let req_data = &buf[..n];

        // 尝试解析 HTTP 请求
        let (method, url, req_headers, req_body) = match parse_http_request(req_data) {
            Ok(r) => r,
            Err(_) => {
                // 无法解析 → 非 HTTP 流量 (可能是 WebSocket 等)，直接转发
                server.write_all(req_data).await?;
                // 读取响应并转发
                let mut resp = vec![0u8; 524288];
                let n = server.read(&mut resp).await?;
                if n > 0 {
                    client.write_all(&resp[..n]).await?;
                }
                break;
            }
        };

        req_count += 1;

        // 构建转发请求 (修改 Host 和 Accept-Encoding)
        let path = extract_path(&url);
        let mut forward_req = format!("{} {} HTTP/1.1\r\n", method, path);
        for (k, v) in &req_headers {
            let low_k = k.to_lowercase();
            if low_k == "host" {
                forward_req.push_str(&format!("Host: {}\r\n", host));
            } else if low_k == "accept-encoding" {
                // 请求原文返回，避免压缩
            } else if low_k == "proxy-connection" {
                // 移除代理头
            } else {
                forward_req.push_str(&format!("{}: {}\r\n", k, v));
            }
        }
        forward_req.push_str("Accept-Encoding: identity\r\n");
        forward_req.push_str("\r\n");

        server.write_all(forward_req.as_bytes()).await?;
        if let Some(ref body) = req_body {
            server.write_all(body).await?;
        }

        // 读取响应
        let mut resp_buf = vec![0u8; 524288];
        let n = server.read(&mut resp_buf).await?;
        if n == 0 {
            break;
        }
        let resp_data = &resp_buf[..n];

        // 记录
        if let Some(exchange) = parse_response(
            &method, &url, host, &req_headers, &req_body, resp_data,
        ) {
            let count = {
                let mut cap = captured.lock().unwrap();
                cap.push(exchange);
                cap.len()
            };
            let url_short = if url.len() > 80 { &url[..80] } else { &url };
            println!("[capture #{}] {} {} → {} bytes", count, method, url_short, resp_data.len());
        }

        // 转发响应
        client.write_all(resp_data).await?;
    }

    println!("[proxy] 与 {} 的连接完成 ({} 个请求)", host, req_count);
    Ok(())
}

// ============================================================================
// HTTP 解析辅助函数
// ============================================================================

/// 解析 HTTP 头部和体的分界
fn parse_http_headers(data: &[u8]) -> Result<(HashMap<String, String>, usize)> {
    let text = String::from_utf8_lossy(data);
    let header_end = text
        .find("\r\n\r\n")
        .or_else(|| text.find("\n\n"))
        .context("no header terminator found")?;

    let header_section = if text.contains("\r\n\r\n") {
        &text[..header_end]
    } else {
        &text[..header_end]
    };

    let mut headers = HashMap::new();
    for line in header_section.lines().skip(1) {
        if let Some((k, v)) = line.split_once(':') {
            headers.insert(k.trim().to_string(), v.trim().to_string());
        }
    }

    let body_start = header_end + if text.contains("\r\n\r\n") { 4 } else { 2 };
    Ok((headers, body_start))
}

/// 解析 HTTP 请求行 + 头 + 体
fn parse_http_request(
    data: &[u8],
) -> Result<(String, String, HashMap<String, String>, Option<Vec<u8>>)> {
    let text = String::from_utf8_lossy(data);
    let first_line = text.lines().next().context("empty request")?;
    let parts: Vec<&str> = first_line.split_whitespace().collect();
    if parts.len() < 2 {
        anyhow::bail!("invalid request: {}", first_line);
    }
    let method = parts[0].to_string();
    let url = parts[1].to_string();

    let (headers, body_start) = parse_http_headers(data)?;
    let body = if body_start < data.len() {
        Some(data[body_start..].to_vec())
    } else {
        None
    };

    Ok((method, url, headers, body))
}

/// 从 URL 提取路径 (移除 scheme 和 host)
fn extract_path(url: &str) -> String {
    if url.starts_with("http") {
        if let Ok(parsed) = url::Url::parse(url) {
            let path = parsed.path().to_string();
            if let Some(q) = parsed.query() {
                format!("{}?{}", path, q)
            } else {
                path
            }
        } else {
            url.to_string()
        }
    } else {
        url.to_string()
    }
}

/// 解析 HTTP 响应并构建 CapturedExchange
fn parse_response(
    method: &str,
    url: &str,
    host: &str,
    req_headers: &HashMap<String, String>,
    req_body: &Option<Vec<u8>>,
    resp_data: &[u8],
) -> Option<CapturedExchange> {
    let text = String::from_utf8_lossy(resp_data);
    let first_line = text.lines().next()?;

    let status_parts: Vec<&str> = first_line.split_whitespace().collect();
    let status = status_parts.get(1)?.parse::<u16>().ok()?;
    let reason = status_parts.get(2).map(|s| s.to_string()).unwrap_or_default();

    let header_end = text.find("\r\n\r\n").or_else(|| text.find("\n\n"))?;
    let header_section = if text.contains("\r\n\r\n") {
        &text[..header_end]
    } else {
        &text[..header_end]
    };

    let mut resp_headers = HashMap::new();
    for line in header_section.lines().skip(1) {
        if let Some((k, v)) = line.split_once(':') {
            resp_headers.insert(k.trim().to_string(), v.trim().to_string());
        }
    }

    let body_start = header_end + if text.contains("\r\n\r\n") { 4 } else { 2 };
    let resp_body = if body_start < resp_data.len() {
        Some(resp_data[body_start..].to_vec())
    } else {
        None
    };

    let (req_body_b64, req_body_text) = encode_body(req_body);
    let (resp_body_b64, resp_body_text) = encode_body(&resp_body);

    Some(CapturedExchange {
        timestamp: chrono_now(),
        method: method.to_string(),
        url: url.to_string(),
        request_headers: req_headers.clone(),
        request_body_base64: req_body_b64,
        request_body_text: req_body_text,
        response_status: status,
        response_reason: reason,
        response_headers: resp_headers,
        response_body_base64: resp_body_b64,
        response_body_text: resp_body_text,
        host: host.to_string(),
    })
}

/// 将可选字节体编码为 base64 和 UTF-8 文本
fn encode_body(body: &Option<Vec<u8>>) -> (Option<String>, Option<String>) {
    match body {
        Some(data) if !data.is_empty() => {
            let b64 = base64::engine::general_purpose::STANDARD.encode(data);
            let text = String::from_utf8(data.clone()).ok();
            (Some(b64), text)
        }
        _ => (None, None),
    }
}

/// 获取当前 UTC 时间 (RFC 3339)
fn chrono_now() -> String {
    use time::OffsetDateTime;
    let now = OffsetDateTime::now_utc();
    now.format(&time::format_description::well_known::Rfc3339)
        .unwrap_or_else(|_| "unknown".to_string())
}

/// 右侧填充
fn pad_right(s: &str, width: usize) -> String {
    if s.len() >= width {
        s.to_string()
    } else {
        format!("{}{}", s, " ".repeat(width - s.len()))
    }
}

// ============================================================================
// 单元测试
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_http_headers() {
        let raw = b"GET / HTTP/1.1\r\nHost: example.com\r\nContent-Type: text/html\r\n\r\nbody";
        let (headers, body_start) = parse_http_headers(raw).unwrap();
        assert_eq!(headers.get("Host").unwrap(), "example.com");
        assert_eq!(headers.get("Content-Type").unwrap(), "text/html");
        assert!(body_start > 0);
    }

    #[test]
    fn test_extract_path() {
        assert_eq!(extract_path("/api/test"), "/api/test");
        assert_eq!(
            extract_path("https://example.com/path?q=1"),
            "/path?q=1"
        );
    }

    #[test]
    fn test_encode_body() {
        let (b64, text) = encode_body(&Some(b"hello".to_vec()));
        assert_eq!(text.unwrap(), "hello");
        assert!(b64.is_some());
    }

    #[test]
    fn test_encode_body_empty() {
        let (b64, text) = encode_body(&None);
        assert!(b64.is_none());
        assert!(text.is_none());
    }

    #[test]
    fn test_parse_http_request() {
        let raw = b"POST /api/login HTTP/1.1\r\nHost: example.com\r\nContent-Length: 9\r\n\r\nuser=test";
        let (method, url, headers, body) = parse_http_request(raw).unwrap();
        assert_eq!(method, "POST");
        assert_eq!(url, "/api/login");
        assert_eq!(headers.get("Host").unwrap(), "example.com");
        assert_eq!(body.unwrap(), b"user=test");
    }
}
