# 微博 (weibo.com) SSO 登录认证机制 — 逆向分析文档

## 1. 整体架构

微博使用 **新浪 SSO (Single Sign-On)** 统一认证系统，涉及三个域名:

| 域名 | 作用 |
|------|------|
| `login.sina.com.cn` | SSO 认证服务 — 预登录、密码验证、票据签发 |
| `passport.weibo.com` | 微博护照 — SSO 票据交换、写入最终 Cookie |
| `weibo.com` | 微博主站 — 消费 Cookie 提供业务 API |

## 2. 登录流程 (3 步)

### Step 1 — 预登录 (`prelogin.php`)

```
GET https://login.sina.com.cn/sso/prelogin.php
  ?entry=weibo
  &callback=sinaSSOController.preloginCallBack
  &su={base64(urlencode(username))}
  &rsakt=mod
  &checkpin=1
  &client=ssologin.js(v1.4.19)
  &_={timestamp_ms}
```

**响应示例 (JSONP):**
```json
{
  "retcode": 0,
  "servertime": 1781678451,
  "nonce": "9CT5ZI",
  "pubkey": "EB2A38568661887FA180BDDB5CABD5F21C...",
  "rsakv": "1330428213",
  "pcid": "yf-9ad66e828c12b7416c95d6f02b988a0ae300",
  "showpin": 0,
  "is_openlock": 0,
  "exectime": 60
}
```

关键字段:
- `servertime` — 服务器 Unix 时间戳，参与密码加密
- `nonce` — 6 位随机字符串 [A-Z0-9]，参与密码加密
- `pubkey` — RSA 公钥模数 (256 字节十六进制)
- `rsakv` — 公钥版本标识
- `pcid` — 验证码 ID (需要验证码时用)
- `showpin` — 是否需要验证码 (0/1)

### Step 2 — SSO 登录 (`login.php`)

#### 2a. 密码加密 (`pwencode=rsa2`)

**RSA 明文格式 (与 ssologin.js 完全一致):**

```
{servertime}\t{nonce}\n{password}
```

即: `服务器时间 + TAB + 随机串 + LF + 明文密码`

**RSA 参数:**
- 模数 `n`: 来自 prelogin 返回的 `pubkey` (十六进制→十进制)
- 公钥指数 `e`: **65537** (0x10001)，硬编码在 ssologin.js 中
- 填充: PKCS#1 v1.5
- 输出: 十六进制编码的密文

等价 JS:
```javascript
var f = new sinaSSOEncoder.RSAKey;
f.setPublic(me.rsaPubkey, "10001");
password = f.encrypt([me.servertime, me.nonce].join("\t") + "\n" + password);
```

#### 2b. 登录 POST

```
POST https://login.sina.com.cn/sso/login.php?client=ssologin.js(v1.4.19)
Content-Type: application/x-www-form-urlencoded
Referer: https://weibo.com/

entry=weibo
gateway=1
from=
savestate=7
qrcode_flag=false
useticket=1         ← 必须为 1，请求返回 ticket
vsnf=1
su={base64(urlencode(username))}
service=miniblog
servertime={servertime}
nonce={nonce}
pwencode=rsa2
rsakv={rsakv}
sp={rsa_encrypted_hex}
sr=1920*1080
encoding=UTF-8
prelt=207
url=https://weibo.com/ajaxlogin.php?framelogin=1&callback=parent.sinaSSOController.feedBackUrlCallBack
returntype=META      ← 返回 HTML meta 重定向

# 如果需要验证码:
pcid={pcid}
door={captcha_code}
```

**成功响应 (HTML，GBK 编码):**
```html
<html>
<head>
<title>新浪通行证</title>
<meta http-equiv="refresh" content="0; url='https://weibo.com/ajaxlogin.php?framelogin=1&...&ticket=ST-XXXXX&retcode=0'"/>
</head>
<body>
<script>location.replace("https://weibo.com/ajaxlogin.php?...&ticket=ST-XXXXX&retcode=0");</script>
</body>
</html>
```

**失败响应:**
- `retcode=1117` — 密码错误
- `retcode=2070` — 需要验证码
- `retcode=4049` — 账号异常/冻结

### Step 3 — 票据交换 & Cookie 写入

成功获取 ticket 后，需要跟随重定向链完成跨域 Cookie 设置:

```
① GET https://weibo.com/ajaxlogin.php?ticket=ST-XXXXX&retcode=0&...
   → 返回 JSONP: parent.sinaSSOController.feedBackUrlCallBack({"result":true, "userinfo":{"uniqueid":"..."}})
   → 可能包含 arrURL 数组，列出需要访问的跨域 URL

② GET https://login.sina.com.cn/crossdomain2.php?action=login&...
   → 在 login.sina.com.cn 域设置 SSO Cookie

③ GET https://passport.weibo.com/wbsso/login?ticket=ST-XXXXX&ssosavestate={ts}&callback=sinaSSOController.doCrossDomainCallBack&...
   → 在 passport.weibo.com 域设置关键 Cookie
   → 返回 JSONP: sinaSSOController.doCrossDomainCallBack({"uniqueid":"..."})
```

## 3. 登录后 Cookie 结构

| Cookie | 域名 | 说明 |
|--------|------|------|
| **SUB** | `.weibo.com` | **核心登录态令牌**，所有鉴权的依据 |
| **SUBP** | `.weibo.com` | SUB 配套参数 |
| **_s_tentry** | `.weibo.com` | 登录来源入口标记 |
| **XSRF-TOKEN** | `.weibo.com` | CSRF 防护 token |
| **WBPSESS** | `.weibo.com` | 微博会话 ID |
| **SCF** | `.weibo.com` | 安全防护 Cookie |
| **ALF** | `.weibo.com` | 登录过期时间戳 |
| **SSOLoginState** | `.weibo.com` | SSO 登录状态时间戳 |
| **Apache** | `.weibo.com` | 时间戳追踪 |

## 4. API 鉴权机制

### 4.1 GET 请求

只需携带 Cookie，关键头:

```
Cookie: SUB=_2A25P...; SUBP=0033Wr...; _s_tentry=weibo.com; ...
Referer: https://weibo.com/
```

### 4.2 POST/PUT/DELETE 请求

需要额外的 XSRF 防护:

```
X-XSRF-TOKEN: {XSRF-TOKEN cookie 的值}
X-Requested-With: XMLHttpRequest
Referer: https://weibo.com/
```

### 4.3 Python 请求示例

```python
# 构造鉴权头
headers = {
    "Accept": "application/json, text/plain, */*",
    "X-XSRF-TOKEN": xsrf_token,       # 来自 cookie
    "X-Requested-With": "XMLHttpRequest",
    "Referer": "https://weibo.com/",
}

# GET 请求
resp = session.get(
    "https://weibo.com/ajax/statuses/home_timeline",
    params={"page": 1},
    headers=headers,
)

# POST 请求
resp = session.post(
    "https://weibo.com/ajax/statuses/update",
    data={"content": "Hello"},
    headers=headers,
)
```

## 5. 常用 API 端点

| 端点 | 方法 | 说明 | 需登录 |
|------|------|------|--------|
| `/ajax/profile/info?uid={uid}` | GET | 用户信息 | 否 |
| `/ajax/statuses/home_timeline` | GET | 首页时间线 | 是 |
| `/ajax/statuses/mymblog?uid={uid}` | GET | 用户微博列表 | 否 |
| `/ajax/statuses/show?id={mid}` | GET | 微博详情+评论 | 否 |
| `/ajax/statuses/update` | POST | 发微博 | 是 |
| `/ajax/statuses/like` | POST | 点赞 | 是 |
| `/ajax/search/weibo?q={kw}` | GET | 搜索 | 否 |
| `/ajax/side/hotSearch` | GET | 热搜榜 | 否 |
| `/ajax/statuses/repost` | POST | 转发 | 是 |
| `/ajax/comments/create` | POST | 评论 | 是 |

## 6. 安全机制总结

1. **密码保护**: RSA-2048 公钥加密，服务器持有私钥解密
2. **重放保护**: nonce + servertime 组合，每次登录不同
3. **验证码**: 基于 IP 信誉和账号风险评分触发
4. **Ticket 机制**: 一次性 SSO ticket，用后即失效
5. **CSRF 防护**: XSRF-TOKEN Cookie + Header 双重验证
6. **跨域 SSO**: 通过重定向链在不同域之间传递票据
7. **反爬虫**: User-Agent + Referer + Cookie 一致性检查

## 7. 二维码扫码登录 (QR Code Login) — 2025 新版

### 7.1 整体流程 (4 步)

```
PC 端                             微博服务端                        手机端
  │                                  │                              │
  ├─① GET /sso/v2/qrcode/image ────→│                              │
  │←─ {qrid, image_url} ───────────┤                              │
  │                                  │                              │
  ├─② POST /sso/bd (bot检测) ──────→│                              │
  │←─ {rid} ────────────────────────┤                              │
  │                                  │                              │
  ├─③ 循环轮询 /sso/v2/qrcode/check─→│                              │
  │   ?qrid=...&rid=...&ver=20250520 │←── 用户扫码 ────────────────┤
  │←─ {retcode:50114001} "未使用"    │                              │
  │←─ {retcode:50114002} "已扫描"    │←── 用户确认 ────────────────┤
  │←─ {retcode:20000000, alt:...}   │                              │
  │                                  │                              │
  ├─④ 票据交换 (重定向链) ──────────→│                              │
  │   /sso/v2/login?alt=ALT-...      │                              │
  │   → crossdomain?ticket=ST-...    │                              │
  │   → crossdomain?ticket=ST-...    │                              │
  │←─ Set-Cookie: SUB=... ──────────┤                              │
```

### 7.2 Step 1 — 获取二维码

```
GET https://passport.weibo.com/sso/v2/qrcode/image?entry=miniblog&size=180
Referer: https://passport.weibo.com/sso/signin?...
```

**响应:**
```json
{
  "retcode": 20000000,
  "msg": "succ",
  "data": {
    "qrid": "3NDZqM6l5AAO5SwK2zF08bxP8iANdl_aDBnFyY29kZQ..",
    "image": "https://v2.qr.weibo.cn/inf/gen?api_key=...&data=https%3A%2F%2Fpassport.weibo.cn%2Fsignin%2Fqrcode%2Fscan%3Fqr%3D3NDZqM..."
  }
}
```

- `qrid`: QR 码唯一 ID，用于后续轮询
- `image`: 二维码图片 URL (PNG)，可直接下载展示

### 7.3 Step 2 — Bot 检测 (Behavior Detection)

```
POST https://passport.weibo.com/sso/bd
Content-Type: application/x-www-form-urlencoded

data=<加密的浏览器指纹数据>
```

**响应:**
```json
{"retcode": 20000000, "msg": "succ", "data": {"rid": "02Aat0ILktOeIslg019ddef_MZBvll"}}
```

- `data` 参数由 `wbBotDetector` JS 库生成（加密的浏览器指纹 + 行为数据）
- `rid`: 风控 token，用于轮询时验证
- **注意**: `rid` 可能是可选的，如果没有可以尝试不带 `rid` 参数轮询

### 7.4 Step 3 — 轮询扫码状态

```
GET https://passport.weibo.com/sso/v2/qrcode/check
  ?entry=miniblog
  &qrid={qrid}
  &rid={rid}
  &ver=20250520
```

**状态码及含义:**

| retcode | 含义 | 下一步 |
|---------|------|--------|
| `50114001` | 未使用 (等待扫码) | 继续轮询 |
| `50114002` | 成功扫描，请在手机点击确认 | 继续轮询 |
| `50114004` | 二维码已过期 | 重新获取 |
| `20000000` | 确认成功 | 进入 Step 4 |

**确认成功响应 (20000000):**
```json
{
  "retcode": 20000000,
  "msg": "succ",
  "data": {
    "alt": "ALT-1U30gU-1781770629-1-1974943084-54b75fbec01b87de80c5303c890f-2",
    "url": "https://passport.weibo.com/sso/v2/login?..."
  }
}
```

- `alt`: ALT 票据，用于换取 SSO ticket
- 轮询间隔建议: 1-2 秒

### 7.5 Step 4 — 票据交换 (Redirect Chain)

拿到 `alt` 后，跟随重定向链（每一步都是 302）：

```
① GET https://passport.weibo.com/sso/v2/login
    ?entry=miniblog&source=miniblog&type=3
    &alt=ALT-1U30gU-...
    &url=https://weibo.com/
  → 302 → Set-Cookie (passport.weibo.com 域)

② GET https://login.sina.com.cn/sso/v2/crossdomain
    ?entry=miniblog&action=login
    &ticket=ST-1U30gU-...
    &cdurl=https://passport.weibo.cn/sso/crossdomain?...
  → 302 → Set-Cookie (login.sina.com.cn 域)

③ GET https://passport.weibo.cn/sso/crossdomain
    ?entry=miniblog&service=miniblog&action=login
    &ticket=ST-1U30gU-...
  → 302 → Set-Cookie (passport.weibo.cn / weibo.com 域)
```

**注意:** 
- 需要使用 HTTP 客户端开启 cookie 追踪（自动跟随 redirect）
- 最终会设置 SUB 等核心鉴权 Cookie
- 重定向链过程与密码登录的 Step 3 相同

### 7.6 Bot Detection 规避方案

`POST /sso/bd` 的 `data` 参数由 `wbBotDetector` JS 库生成，包含：
- 浏览器指纹 (Canvas, WebGL, 字体等)
- 鼠标/触摸行为数据
- 时间戳和随机数
- 加密后的二进制数据 (URL-safe Base64)

**可选规避方案:**

| 方案 | 复杂度 | 可靠性 |
|------|--------|--------|
| A. WebView 嵌入登录页面 | 低 | 高 — 直接用真实浏览器完成登录 |
| B. 跳过 `rid` 参数直接轮询 | 极低 | 待测试 |
| C. 逆向 `wbBotDetector` JS 并重写 | 高 | 中 |
| D. 固定 `rid` 值复用 | 低 | 低 (可能过期) |

**推荐方案:** 对于 PC 客户端，使用 **WebView 嵌入** 登录页面。
在 Rust 中可用 `wry` 或 `tao` crate 嵌入系统 WebView，
登录完成后从 WebView 提取 Cookie 用于 API 调用。
