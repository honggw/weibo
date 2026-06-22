//! CA 证书管理模块
//!
//! 负责:
//!   1. 生成自签名 CA 根证书 (首次运行)
//!   2. 持久化 CA 证书和私钥到磁盘
//!   3. 为任意域名动态签发 SSL 证书 (MITM 用)

use anyhow::Result;
use rcgen::{
    BasicConstraints, Certificate, CertificateParams, DnType, IsCa, KeyPair, KeyUsagePurpose,
};
use std::path::PathBuf;
use time::{Duration, OffsetDateTime};

/// CA 文件存放目录
const CA_DIR: &str = "ca";
const CA_CERT_FILE: &str = "ca_cert.pem";
const CA_KEY_FILE: &str = "ca_key.pem";

/// CA 证书管理器
///
/// 管理 CA 根证书的生命周期：生成、持久化、域名证书签发。
/// `Certificate` 字段不可序列化，但在进程生命周期内保持在内存中。
pub struct CaManager {
    /// CA 证书 (仅用于签发)
    ca_cert: Certificate,
    /// CA 私钥
    ca_key: KeyPair,
    /// CA 证书 PEM
    ca_cert_pem: String,
    /// CA 私钥 PEM
    ca_key_pem: String,
    /// 工作目录
    dir: PathBuf,
}

impl CaManager {
    /// 初始化 CA 管理器
    ///
    /// 如果 CA 证书和私钥已存在则加载，否则生成新的 CA。
    /// 同时保持 `Certificate` 和 `KeyPair` 在内存中用于签发域名证书。
    pub fn init() -> Result<Self> {
        let dir = PathBuf::from(CA_DIR);
        std::fs::create_dir_all(&dir)?;

        let cert_path = dir.join(CA_CERT_FILE);
        let key_path = dir.join(CA_KEY_FILE);

        if cert_path.exists() && key_path.exists() {
            // 加载已有 CA
            let ca_cert_pem = std::fs::read_to_string(&cert_path)?;
            let ca_key_pem = std::fs::read_to_string(&key_path)?;

            // 从 PEM 重新构建 Certificate 对象用于签发
            let ca_key = KeyPair::from_pem(&ca_key_pem)?;
            let ca_params = CertificateParams::from_ca_cert_pem(&ca_cert_pem)?;
            let ca_cert = ca_params.self_signed(&ca_key)?;

            println!("[CA] 已加载现有 CA 证书: {}", cert_path.display());
            Ok(Self {
                ca_cert,
                ca_key,
                ca_cert_pem,
                ca_key_pem,
                dir,
            })
        } else {
            // 生成新 CA
            println!("[CA] 生成新的 CA 根证书...");
            let (ca_cert, ca_key, ca_cert_pem, ca_key_pem) = Self::generate_ca()?;
            std::fs::write(&cert_path, &ca_cert_pem)?;
            std::fs::write(&key_path, &ca_key_pem)?;
            println!("[CA] CA 证书已保存到: {}", cert_path.display());
            println!("[CA] CA 私钥已保存到: {}", key_path.display());
            Ok(Self {
                ca_cert,
                ca_key,
                ca_cert_pem,
                ca_key_pem,
                dir,
            })
        }
    }

    /// 生成新的 CA 根证书
    fn generate_ca() -> Result<(Certificate, KeyPair, String, String)> {
        let mut params = CertificateParams::default();
        params.distinguished_name.push(DnType::CommonName, "Weibo Client MITM CA");
        params.distinguished_name.push(DnType::OrganizationName, "Weibo PC Client Dev");
        params.is_ca = IsCa::Ca(BasicConstraints::Unconstrained);
        params.key_usages = vec![
            KeyUsagePurpose::KeyCertSign,
            KeyUsagePurpose::CrlSign,
            KeyUsagePurpose::DigitalSignature,
        ];

        // CA 有效期: 10 年
        let now = OffsetDateTime::now_utc();
        params.not_before = now;
        params.not_after = now + Duration::days(365 * 10);

        let key_pair = KeyPair::generate()?;
        let cert = params.self_signed(&key_pair)?;

        let key_pem = key_pair.serialize_pem();
        let cert_pem = cert.pem();

        // 从 PEM 重新解析 key_pair，避免 clone 问题
        let key_pair = KeyPair::from_pem(&key_pem)?;

        Ok((cert, key_pair, cert_pem, key_pem))
    }

    /// 为指定域名签发 SSL 证书
    ///
    /// 返回 (cert_pem, key_pem) 用于 TLS 握手。
    pub fn sign_domain(&self, domain: &str) -> Result<(String, String)> {
        let mut params = CertificateParams::new(vec![domain.to_string()])?;
        params.distinguished_name.push(DnType::CommonName, domain);
        params.key_usages = vec![
            KeyUsagePurpose::DigitalSignature,
            KeyUsagePurpose::KeyEncipherment,
        ];

        // 域名证书有效期: 30 天
        let now = OffsetDateTime::now_utc();
        params.not_before = now;
        params.not_after = now + Duration::days(30);

        let leaf_key = KeyPair::generate()?;
        let cert = params.signed_by(&leaf_key, &self.ca_cert, &self.ca_key)?;

        Ok((cert.pem(), leaf_key.serialize_pem()))
    }

    /// 返回 CA 证书 PEM 内容 (供客户端安装)
    pub fn ca_cert_pem(&self) -> &str {
        &self.ca_cert_pem
    }

    /// 返回 CA 证书文件路径 (供用户安装)
    pub fn ca_cert_path(&self) -> PathBuf {
        self.dir.join(CA_CERT_FILE)
    }

    /// 静默初始化 (不打印日志，供 proxy 内部使用)
    pub fn init_silent() -> Result<Self> {
        let dir = PathBuf::from(CA_DIR);
        std::fs::create_dir_all(&dir)?;

        let cert_path = dir.join(CA_CERT_FILE);
        let key_path = dir.join(CA_KEY_FILE);

        if cert_path.exists() && key_path.exists() {
            let ca_cert_pem = std::fs::read_to_string(&cert_path)?;
            let ca_key_pem = std::fs::read_to_string(&key_path)?;
            let ca_key = KeyPair::from_pem(&ca_key_pem)?;
            let ca_params = CertificateParams::from_ca_cert_pem(&ca_cert_pem)?;
            let ca_cert = ca_params.self_signed(&ca_key)?;
            Ok(Self {
                ca_cert,
                ca_key,
                ca_cert_pem,
                ca_key_pem,
                dir,
            })
        } else {
            let (ca_cert, ca_key, ca_cert_pem, ca_key_pem) = Self::generate_ca()?;
            std::fs::write(&cert_path, &ca_cert_pem)?;
            std::fs::write(&key_path, &ca_key_pem)?;
            Ok(Self {
                ca_cert,
                ca_key,
                ca_cert_pem,
                ca_key_pem,
                dir,
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_ca() {
        let manager = CaManager::init().expect("Failed to init CA manager");
        assert!(!manager.ca_cert_pem.is_empty());
        assert!(!manager.ca_key_pem.is_empty());
        assert!(manager.ca_cert_pem.contains("BEGIN CERTIFICATE"));
        assert!(manager.ca_key_pem.contains("BEGIN PRIVATE KEY"));
    }

    #[test]
    fn test_sign_domain() {
        let manager = CaManager::init().expect("Failed to init CA manager");
        let (cert_pem, key_pem) = manager.sign_domain("login.sina.com.cn").expect("Failed to sign domain");
        assert!(!cert_pem.is_empty());
        assert!(!key_pem.is_empty());
        assert!(cert_pem.contains("BEGIN CERTIFICATE"));
        assert!(key_pem.contains("BEGIN PRIVATE KEY"));
    }

    #[test]
    fn test_reload_ca() {
        // 第一次初始化
        let m1 = CaManager::init().expect("Failed to init CA manager");
        let cert1 = m1.ca_cert_pem.clone();
        drop(m1);

        // 第二次初始化应该加载同一个证书
        let m2 = CaManager::init().expect("Failed to reload CA manager");
        assert_eq!(cert1, m2.ca_cert_pem);
    }
}
