//! wbBotDetector 的 Rust 重实现
//!
//! 完整逆向自 fp/1.2.1.umd.js + fp/1.3.2.umd.js。
//! 指纹格式经验证可与微博服务端正常通信。

use anyhow::{Context, Result};
use aes::cipher::{block_padding::Pkcs7, BlockEncryptMut, KeyIvInit};
use base64::Engine as _;
use rand::Rng;
use rsa::{
    traits::PublicKeyParts,
    BigUint, Oaep, RsaPublicKey,
};
use sha2::Sha256;
use std::collections::HashMap;

type Aes128CbcEnc = cbc::Encryptor<aes::Aes128>;

// RSA 公钥 (从 fp/1.3.2.umd.js 的 Qe 函数提取)
const RSA_MODULUS_HEX: &str =
    "b4f9654ae3f7dee618dc0a95b783a4b914a6a4729e472e974d47e2174e43b1f6c5f9d527f33726701140879b6d32b93d15696af594d47f0712e3ff28c7f141d3a7b9e805babdf53ba1d630a0fb155cbbac53980b5548258957683f275606965406b2e5dc908583d42f8be8b9c0615989aa8d2713550499ee4b5df360ce48875b";
const RSA_EXPONENT: u32 = 65537;

// ============================================================================
// 数据结构 (严格匹配真实浏览器指纹)
// ============================================================================

/// 单个指纹字段: {s: status, v: value} 或 {s: -1, e: ""}
#[derive(serde::Serialize, Clone)]
struct FpField<T: serde::Serialize> {
    s: i32,
    #[serde(skip_serializing_if = "Option::is_none")]
    v: Option<T>,
    #[serde(skip_serializing_if = "Option::is_none")]
    e: Option<String>,
}

impl<T: serde::Serialize> FpField<T> {
    fn success(v: T) -> Self {
        Self { s: 1, v: Some(v), e: None }
    }
}

/// 错误字段
fn fp_error() -> serde_json::Value {
    serde_json::json!({"s": -1, "e": ""})
}

/// 构建浏览器指纹 (24 个字段，索引 0-23)
fn build_fingerprint_map() -> HashMap<String, serde_json::Value> {
    let mut fp: HashMap<String, serde_json::Value> = HashMap::new();

    // 0: 版本号
    fp.insert("0".into(), serde_json::json!("1.2.1"));

    // 1: 某种检测 (boolean)
    fp.insert("1".into(), serde_json::json!({"s": 1, "v": true}));

    // 2: 语言检测
    fp.insert("2".into(), serde_json::json!({"s": 1, "v": ["lang"]}));

    // 3: navigator.userAgent
    fp.insert("3".into(), serde_json::json!({"s": 1, "v":
        "5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/148.0.0.0 Safari/537.36"
    }));

    // 4: Function.prototype.bind 检测 (某些环境会失败)
    // 真实浏览器中这是一个 stack trace，我们填一个简单的
    fp.insert("4".into(), serde_json::json!({"s": 1, "v":
        "Error\n    at Object.<anonymous>"
    }));

    // 5: 颜色深度相关
    fp.insert("5".into(), serde_json::json!({"s": 1, "v": 33}));

    // 6: bind 函数检测
    fp.insert("6".into(), serde_json::json!({"s": 1, "v":
        "function bind() { [native code] }"
    }));

    // 7: navigator.languages
    fp.insert("7".into(), serde_json::json!({"s": 1, "v": [["zh-CN"]]}));

    // 8-10: booleans
    fp.insert("8".into(), serde_json::json!({"s": 1, "v": true}));
    fp.insert("9".into(), serde_json::json!({"s": 1, "v": false}));
    fp.insert("10".into(), serde_json::json!({"s": 1, "v": true}));

    // 11: navigator.hardwareConcurrency
    fp.insert("11".into(), serde_json::json!({"s": 1, "v": 5}));

    // 12: 某些环境检测失败 (s=-1)
    fp.insert("12".into(), serde_json::json!({"s": -1, "e": ""}));

    // 13: Chrome 版本号 / 构建号
    fp.insert("13".into(), serde_json::json!({"s": 1, "v": "20030107"}));

    // 14: 数值
    fp.insert("14".into(), serde_json::json!({"s": 1, "v": 100}));

    // 15: 与 3 相同 (navigator.userAgent)
    fp.insert("15".into(), serde_json::json!({"s": 1, "v":
        "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/148.0.0.0 Safari/537.36"
    }));

    // 16: WebGL vendor/renderer
    fp.insert("16".into(), serde_json::json!({"s": 1, "v": {
        "vendor": "WebKit",
        "renderer": "WebKit WebGL"
    }}));

    // 17: window.external
    fp.insert("17".into(), serde_json::json!({"s": 1, "v": "[object External]"}));

    // 18: 屏幕尺寸
    fp.insert("18".into(), serde_json::json!({"s": 1, "v": {
        "ow": 1296,   // outerWidth
        "oh": 808,    // outerHeight
        "iw": 1280,   // innerWidth
        "ih": 720     // innerHeight
    }}));

    // 19: 浏览器类型
    fp.insert("19".into(), serde_json::json!({"s": 1, "v": "chrome"}));

    // 20: 浏览器引擎
    fp.insert("20".into(), serde_json::json!({"s": 1, "v": "chromium"}));

    // 21-22: booleans
    fp.insert("21".into(), serde_json::json!({"s": 1, "v": true}));
    fp.insert("22".into(), serde_json::json!({"s": 1, "v": true}));

    // 23: 时间/Timing 信息
    fp.insert("23".into(), serde_json::json!({"s": 1, "v": {
        "ots": false,
        "mtp": 20,
        "mmtp": -1
    }}));

    fp
}

// ============================================================================
// 加密函数
// ============================================================================

fn build_rsa_pubkey() -> Result<RsaPublicKey> {
    let modulus = BigUint::parse_bytes(RSA_MODULUS_HEX.as_bytes(), 16)
        .context("解析 RSA modulus 失败")?;
    RsaPublicKey::new(modulus, BigUint::from(RSA_EXPONENT as u64))
        .context("构造 RSA 公钥失败")
}

fn aes_cbc_encrypt(key: &[u8], iv: &[u8], plaintext: &[u8]) -> Vec<u8> {
    let block_size = 16usize;
    let pad_len = block_size - (plaintext.len() % block_size);
    let total = plaintext.len() + pad_len;
    let mut buf = vec![0u8; total];
    buf[..plaintext.len()].copy_from_slice(plaintext);
    Aes128CbcEnc::new(key.into(), iv.into())
        .encrypt_padded_mut::<Pkcs7>(&mut buf, plaintext.len())
        .expect("AES encrypt");
    buf
}

fn rsa_oaep_encrypt(pubkey: &RsaPublicKey, data: &[u8]) -> Result<Vec<u8>> {
    let padding = Oaep::new::<Sha256>();
    pubkey
        .encrypt(&mut rand::thread_rng(), padding, data)
        .context("RSA OAEP 加密失败")
}

// ============================================================================
// 主入口
// ============================================================================

/// 生成加密的 bot detection 数据
pub fn generate_bd_data() -> Result<String> {
    let mut rng = rand::thread_rng();

    // 1. 构建指纹 JSON
    let finger = serde_json::json!({
        "fp": build_fingerprint_map(),
        "bh": {
            "mt": [],
            "kt": { "down": 0, "up": 0 }
        },
        "meta": {
            "isTraceKeyboard": true,
            "isTraceMouse": true
        }
    });
    let json_str = serde_json::to_string(&finger)?;

    // 2. 生成随机 AES-128 key + IV
    let aes_key: [u8; 16] = rng.gen();
    let aes_iv: [u8; 16] = rng.gen();

    // 3. AES-CBC 加密指纹
    let aes_ciphertext = aes_cbc_encrypt(&aes_key, &aes_iv, json_str.as_bytes());

    // 4. RSA-OAEP 加密 (key || iv)
    let rsa_pubkey = build_rsa_pubkey()?;
    let mut key_iv = Vec::with_capacity(32);
    key_iv.extend_from_slice(&aes_key);
    key_iv.extend_from_slice(&aes_iv);
    let rsa_blob = rsa_oaep_encrypt(&rsa_pubkey, &key_iv)?;

    // 5. 打包: "01" + rsa_blob + "02" + aes_ciphertext
    let mut inner = Vec::new();
    inner.extend_from_slice(b"01");
    inner.extend_from_slice(&rsa_blob);
    inner.extend_from_slice(b"02");
    inner.extend_from_slice(&aes_ciphertext);

    // 6. Base64 + 外层前缀
    let b64 = base64::engine::general_purpose::STANDARD.encode(&inner);
    Ok(format!("01{}", b64))
}

/// 向 /sso/bd 发送数据并获取 rid
pub async fn get_rid(client: &reqwest::Client) -> Result<String> {
    let data = generate_bd_data()?;

    let resp = client
        .post("https://passport.weibo.com/sso/bd")
        .header("Content-Type", "application/x-www-form-urlencoded")
        .header("Referer", "https://passport.weibo.com/sso/signin?entry=miniblog")
        .form(&[("data", data.as_str()), ("from", "weibo")])
        .send()
        .await
        .context("POST /sso/bd 失败")?;

    let body: serde_json::Value = resp.json().await.context("解析 /sso/bd 响应失败")?;
    let retcode = body.get("retcode").and_then(|v| v.as_i64()).unwrap_or(-1);
    if retcode != 20000000 {
        anyhow::bail!(
            "/sso/bd 错误: retcode={} msg={:?}",
            retcode,
            body.get("msg")
        );
    }

    body.get("data")
        .and_then(|d| d.get("rid"))
        .and_then(|v| v.as_str())
        .map(String::from)
        .context("响应中无 rid 字段")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_bd_data_format() {
        let data = generate_bd_data().expect("generate bd data");
        assert!(data.starts_with("01"), "should start with 01");
        // 解码验证内部结构
        let b64 = &data[2..];
        let inner = base64::engine::general_purpose::STANDARD
            .decode(b64)
            .expect("base64 decode");
        assert_eq!(&inner[..2], b"01", "inner marker 01");
        assert!(inner.len() > 50);
    }

    #[tokio::test]
    async fn test_get_rid_real() {
        let client = reqwest::Client::builder()
            .cookie_store(true)
            .build()
            .unwrap();
        match get_rid(&client).await {
            Ok(rid) => {
                println!("rid={}", rid);
                assert!(!rid.is_empty(), "rid should not be empty");
            }
            Err(e) => {
                panic!("get_rid failed: {}", e);
            }
        }
    }
}
