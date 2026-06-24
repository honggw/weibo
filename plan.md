# 微博 PC 客户端 - 聊天界面改造计划

> 基于 `weibo_web.png` (微博网页聊天截图) 和 `api.weibo.com_chat_conversation0.har` (抓包数据) 分析得出。

---

## 第一阶段：核心体验 (消息展示 + 实时通信)

### 1.1 扩展 Domain Model — `ChatMessage` 增加消息类型字段

**文件**: `src/domain/mod.rs`

**改动说明**: 当前 `ChatMessage` 只有纯文本字段，无法区分图片、引用、撤回、系统通知等消息类型。需新增 `msg_type`、`media_type`、`fids`、`sender_avatar`、`attitude_info` 等字段。

**当前代码**:
```rust
#[derive(Clone, Debug)]
pub struct ChatMessage {
    pub id: String,
    pub sender_id: String,
    pub sender_name: String,
    pub text: String,
    pub created_at: String,
    pub is_self: bool,
}
```

**改为**:
```rust
/// 消息类型枚举 (来自 HAR 中 type 字段)
#[derive(Clone, Debug, PartialEq)]
pub enum MsgType {
    /// 普通消息 (type=321)
    Normal,
    /// 系统消息: 入群通知等 (type=322)
    System,
    /// 撤回消息 (type=344)
    Recall,
    /// 其他未知类型
    Other(u64),
}

impl MsgType {
    pub fn from_api(type_val: u64) -> Self {
        match type_val {
            321 => MsgType::Normal,
            322 => MsgType::System,
            344 => MsgType::Recall,
            v   => MsgType::Other(v),
        }
    }
}

/// 媒体类型枚举 (来自 HAR 中 media_type 字段)
#[derive(Clone, Debug, PartialEq)]
pub enum MediaType {
    /// 纯文本 (media_type=0)
    Text,
    /// 图片 (media_type=1, 有 fids 字段)
    Image,
    /// 引用/转发 (media_type=14, content 中包含引用块)
    Quote,
    /// 其他
    Other(u64),
}

impl MediaType {
    pub fn from_api(val: u64) -> Self {
        match val {
            0  => MediaType::Text,
            1  => MediaType::Image,
            14 => MediaType::Quote,
            v  => MediaType::Other(v),
        }
    }
}

/// 单条聊天消息
#[derive(Clone, Debug)]
pub struct ChatMessage {
    pub id: String,
    pub sender_id: String,
    pub sender_name: String,
    /// 发送者头像 URL (来自 from_user.profile_image_url)
    pub sender_avatar: String,
    pub text: String,
    pub created_at: String,
    /// Unix 时间戳 (秒), 用于时间分组和格式化
    pub timestamp: u64,
    pub is_self: bool,
    /// 消息类型: Normal / System / Recall
    pub msg_type: MsgType,
    /// 媒体类型: Text / Image / Quote
    pub media_type: MediaType,
    /// 图片消息的文件 ID 列表 (media_type=1 时非空)
    /// 用于拼接缩略图 URL: https://upload.api.weibo.com/2/mss/msget_thumbnail?fid={}&high=240&width=240&source=209678993
    pub fids: Vec<String>,
}
```

---

### 1.2 `chat_service.rs` — 解析新增字段

**文件**: `src/model/chat_service.rs`

#### 1.2.1 修改 `fetch_group_messages` 解析逻辑

**当前代码** (第 173-209 行):
```rust
return arr.iter().map(|m| {
    let sid = m.get("from_uid").and_then(|v| v.as_u64()).unwrap_or(0).to_string();
    let name = m.get("from_user").and_then(|u| u.get("screen_name"))
        .and_then(|v| v.as_str()).unwrap_or("?").to_string();
    let text = m.get("content").and_then(|v| v.as_str()).unwrap_or("").to_string();
    let ts = m.get("time").and_then(|v| v.as_u64()).unwrap_or(0);
    use std::time::{Duration, UNIX_EPOCH};
    let time_str = UNIX_EPOCH.checked_add(Duration::from_secs(ts))
        .map(|t| format!("{:?}", t)).unwrap_or_default();
    crate::domain::ChatMessage {
        id: m.get("id").and_then(|v| v.as_u64()).map(|v| v.to_string()).unwrap_or_default(),
        sender_id: sid.clone(),
        sender_name: name,
        text,
        created_at: time_str,
        is_self: sid == my_uid,
    }
}).collect();
```

**改为**:
```rust
return arr.iter().map(|m| {
    let sid = m.get("from_uid").and_then(|v| v.as_u64()).unwrap_or(0).to_string();
    let name = m.get("from_user")
        .and_then(|u| u.get("screen_name"))
        .and_then(|v| v.as_str()).unwrap_or("?").to_string();
    let avatar = m.get("from_user")
        .and_then(|u| u.get("profile_image_url"))
        .and_then(|v| v.as_str()).unwrap_or("").to_string();
    let text = m.get("content").and_then(|v| v.as_str()).unwrap_or("").to_string();
    let ts = m.get("time").and_then(|v| v.as_u64()).unwrap_or(0);
    let time_str = format_timestamp(ts);
    let type_val = m.get("type").and_then(|v| v.as_u64()).unwrap_or(321);
    let media_val = m.get("media_type").and_then(|v| v.as_u64()).unwrap_or(0);
    // 解析图片 fids: "[5312697042208502]" -> vec!["5312697042208502"]
    let fids = m.get("fids")
        .and_then(|v| v.as_str())
        .map(|s| {
            s.trim_matches(|c| c == '[' || c == ']')
                .split(',')
                .filter(|s| !s.is_empty())
                .map(|s| s.trim().to_string())
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();

    crate::domain::ChatMessage {
        id: m.get("id").and_then(|v| v.as_u64()).map(|v| v.to_string()).unwrap_or_default(),
        sender_id: sid.clone(),
        sender_name: name,
        sender_avatar: avatar,
        text,
        created_at: time_str,
        timestamp: ts,
        is_self: sid == my_uid,
        msg_type: crate::domain::MsgType::from_api(type_val),
        media_type: crate::domain::MediaType::from_api(media_val),
        fids,
    }
}).collect();
```

#### 1.2.2 修改 `fetch_messages` (DM) 解析逻辑

**当前代码** (第 253-285 行):
```rust
return arr.iter().rev().map(|m| {
    let sid = m.get("sender_id").and_then(|v| v.as_u64()).unwrap_or(0).to_string();
    crate::domain::ChatMessage {
        id: m.get("idstr").and_then(|v| v.as_str()).unwrap_or("").to_string(),
        sender_id: sid.clone(),
        sender_name: m.get("sender_screen_name").and_then(|v| v.as_str()).unwrap_or("?").to_string(),
        text: m.get("text").and_then(|v| v.as_str()).unwrap_or("").to_string(),
        created_at: m.get("created_at").and_then(|v| v.as_str()).unwrap_or("").to_string(),
        is_self: sid == my_uid,
    }
}).collect();
```

**改为**:
```rust
return arr.iter().rev().map(|m| {
    let sid = m.get("sender_id").and_then(|v| v.as_u64()).unwrap_or(0).to_string();
    let media_val = m.get("media_type").and_then(|v| v.as_u64()).unwrap_or(0);
    // DM 的 type 来自 group_chat_message_type 或 dm_type
    let type_val = m.get("group_chat_message_type")
        .and_then(|v| v.as_u64())
        .unwrap_or(321);
    let fids_str = m.get("fids").and_then(|v| v.as_str()).unwrap_or("");
    let fids = fids_str.trim_matches(|c| c == '[' || c == ']')
        .split(',')
        .filter(|s| !s.is_empty())
        .map(|s| s.trim().to_string())
        .collect::<Vec<_>>();

    crate::domain::ChatMessage {
        id: m.get("idstr").and_then(|v| v.as_str()).unwrap_or("").to_string(),
        sender_id: sid.clone(),
        sender_name: m.get("sender_screen_name")
            .and_then(|v| v.as_str()).unwrap_or("?").to_string(),
        sender_avatar: String::new(), // DM 接口不含头像, 后续可通过 users/show 补全
        text: m.get("text").and_then(|v| v.as_str()).unwrap_or("").to_string(),
        created_at: m.get("created_at").and_then(|v| v.as_str()).unwrap_or("").to_string(),
        timestamp: 0, // DM 接口是字符串时间, 可后续解析
        is_self: sid == my_uid,
        msg_type: crate::domain::MsgType::from_api(type_val),
        media_type: crate::domain::MediaType::from_api(media_val),
        fids,
    }
}).collect();
```

#### 1.2.3 新增 `format_timestamp` 辅助函数

在 `chat_service.rs` 底部添加:

```rust
/// 将 Unix 时间戳格式化为可读时间字符串。
/// 今天的消息只显示 "HH:MM", 其他日期显示 "MM-DD HH:MM"。
fn format_timestamp(ts: u64) -> String {
    if ts == 0 { return String::new(); }
    use std::time::{Duration, UNIX_EPOCH, SystemTime};
    let msg_time = UNIX_EPOCH + Duration::from_secs(ts);
    let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_secs();
    // 计算东八区偏移
    let tz_offset: i64 = 8 * 3600;
    let local_ts = ts as i64 + tz_offset;
    let local_now = now as i64 + tz_offset;
    let secs_in_day: i64 = 86400;
    let msg_day = local_ts / secs_in_day;
    let now_day = local_now / secs_in_day;
    let hour = ((local_ts % secs_in_day) / 3600) as u32;
    let minute = ((local_ts % 3600) / 60) as u32;
    if msg_day == now_day {
        format!("{:02}:{:02}", hour, minute)
    } else if msg_day == now_day - 1 {
        format!("昨天 {:02}:{:02}", hour, minute)
    } else {
        // 简易月/日计算 (近似)
        let month_approx = ((local_ts % (365 * secs_in_day)) / (30 * secs_in_day)) + 1;
        let day_approx = ((local_ts % (30 * secs_in_day)) / secs_in_day) + 1;
        format!("{:02}-{:02} {:02}:{:02}", month_approx, day_approx, hour, minute)
    }
}
```

#### 1.2.4 修改 `send_dm_message` 和 `send_group_message` 返回值

**`send_dm_message`** (第 359-366 行) 补全新字段:
```rust
return Some(crate::domain::ChatMessage {
    id,
    text,
    created_at,
    sender_id: my_uid,
    sender_name: "我".to_string(),
    sender_avatar: String::new(),
    timestamp: std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH).unwrap().as_secs(),
    is_self: true,
    msg_type: crate::domain::MsgType::Normal,
    media_type: crate::domain::MediaType::Text,
    fids: vec![],
});
```

**`send_group_message`** (第 390-393 行) 同理:
```rust
return Some(crate::domain::ChatMessage {
    id, text: text.to_string(), created_at: String::new(),
    sender_id: String::new(), sender_name: "我".to_string(),
    sender_avatar: String::new(),
    timestamp: std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH).unwrap().as_secs(),
    is_self: true,
    msg_type: crate::domain::MsgType::Normal,
    media_type: crate::domain::MediaType::Text,
    fids: vec![],
});
```

---

### 1.3 `message_bubble.rs` — 重构消息气泡渲染

**文件**: `src/view/widgets/message_bubble.rs`

**改动说明**: 当前所有消息都渲染为相同的文本气泡。需要根据 `msg_type` 和 `media_type` 分别渲染:
- 普通文本消息: 保持左右对齐气泡, 新增头像 + 时间戳
- 系统消息 (入群/撤回): 居中灰色小字, 不显示气泡
- 图片消息: 气泡内显示 `[图片]` 占位 (后续阶段加载真实缩略图)
- 引用消息: 气泡内嵌灰色引用块

**完整重写**:
```rust
//! Message bubble widget — renders a single chat message, supporting text/image/quote/system types.

use gpui::*;
use gpui::prelude::*;

use crate::domain::{ChatMessage, MediaType, MsgType};
use crate::view::theme;

pub fn render(msg: &ChatMessage) -> impl IntoElement {
    match &msg.msg_type {
        MsgType::System | MsgType::Recall => render_system(msg),
        _ => render_normal(msg),
    }
}

/// 系统消息 / 撤回消息 — 居中灰色小字
fn render_system(msg: &ChatMessage) -> AnyElement {
    div()
        .flex().flex_row().w_full().justify_center().py_1()
        .child(
            div()
                .px_3().py_1().rounded_full()
                .bg(rgb(0x1a2a4a))
                .text_size(px(11.0))
                .text_color(rgb(theme::CLR_MUTED))
                .child(msg.text.clone()),
        )
        .into_any_element()
}

/// 普通消息 (文本/图片/引用) — 气泡 + 头像 + 时间戳
fn render_normal(msg: &ChatMessage) -> AnyElement {
    let is_self = msg.is_self;
    let bubble_color = if is_self { rgb(theme::CLR_ACCENT) } else { rgb(0x2a3a5a) };
    let text_color = if is_self { rgb(0xffffff) } else { rgb(theme::CLR_TEXT) };

    // 头像: 首字占位圆
    let avatar_char = if is_self {
        "我".to_string()
    } else {
        msg.sender_name.chars().next().map(|c| c.to_string()).unwrap_or_default()
    };
    let avatar = div()
        .w(px(36.0)).h(px(36.0)).rounded_full()
        .bg(if is_self { rgb(theme::CLR_ACCENT) } else { rgb(0x3a4a6a) })
        .flex().items_center().justify_center()
        .text_size(px(14.0)).text_color(rgb(0xffffff)).flex_shrink_0()
        .child(avatar_char);

    // 气泡内容
    let bubble_content = match &msg.media_type {
        MediaType::Image => render_image_bubble(msg, bubble_color, text_color),
        MediaType::Quote => render_quote_bubble(msg, bubble_color, text_color),
        _               => render_text_bubble(msg, bubble_color, text_color),
    };

    // 发送者名称 + 时间
    let name_time = div()
        .flex().flex_row().gap_2().px_1()
        .text_size(px(11.0)).text_color(rgb(theme::CLR_MUTED))
        .child(if is_self { "我".to_string() } else { msg.sender_name.clone() })
        .child(msg.created_at.clone());

    // 整行布局: 头像 + (名称 + 气泡), 自己的消息反向排列
    let msg_body = div()
        .flex().flex_col().gap_1()
        .max_w(px(360.0))
        .child(name_time)
        .child(bubble_content);

    div()
        .flex().flex_row().w_full().px_3().py_1().gap_2()
        .when(is_self, |d| d.flex_row_reverse())
        .child(avatar)
        .child(msg_body)
        .into_any_element()
}

/// 纯文本气泡
fn render_text_bubble(msg: &ChatMessage, bg: Hsla, fg: Hsla) -> AnyElement {
    div()
        .px_3().py_2().rounded_lg()
        .bg(bg).text_size(px(13.0)).text_color(fg)
        .child(msg.text.clone())
        .into_any_element()
}

/// 图片消息气泡 — 显示 [图片] 占位 (第二阶段替换为真实缩略图加载)
fn render_image_bubble(msg: &ChatMessage, bg: Hsla, fg: Hsla) -> AnyElement {
    div()
        .px_3().py_2().rounded_lg().bg(bg)
        .child(
            div().flex().flex_col().gap_1()
                .child(
                    div()
                        .w(px(200.0)).h(px(120.0)).rounded_md()
                        .bg(rgb(0x2a3a5a))
                        .flex().items_center().justify_center()
                        .text_size(px(24.0)).text_color(rgb(theme::CLR_MUTED))
                        .child("🖼")
                )
                .child(if !msg.text.is_empty() && msg.text != "分享图片" {
                    div().text_size(px(13.0)).text_color(fg).child(msg.text.clone()).into_any_element()
                } else {
                    div().text_size(px(12.0)).text_color(rgb(theme::CLR_MUTED)).child("[图片]").into_any_element()
                })
        )
        .into_any_element()
}

/// 引用消息气泡 — 解析 content 中「...」引用块
fn render_quote_bubble(msg: &ChatMessage, bg: Hsla, fg: Hsla) -> AnyElement {
    // 微博引用格式: 「引用文本」\n- - - - -\n回复文本
    // 可能多层嵌套
    let parts: Vec<&str> = msg.text.splitn(2, "\n- - - - - - - - - - - - - - -\n").collect();

    let (quote_text, reply_text) = if parts.len() == 2 {
        (parts[0].trim_matches('「').trim_matches('」').to_string(), parts[1].to_string())
    } else {
        (String::new(), msg.text.clone())
    };

    div()
        .px_3().py_2().rounded_lg().bg(bg)
        .child(
            div().flex().flex_col().gap_2()
                .when(!quote_text.is_empty(), |d| {
                    let qt = quote_text.clone();
                    d.child(
                        div()
                            .px_2().py_1().rounded_md()
                            .bg(rgb(0x1a2a4a))
                            .border_l_2().border_color(rgb(theme::CLR_MUTED))
                            .text_size(px(12.0)).text_color(rgb(theme::CLR_MUTED))
                            .child(qt)
                    )
                })
                .child(div().text_size(px(13.0)).text_color(fg).child(reply_text))
        )
        .into_any_element()
}
```

---

### 1.4 `chat_screen.rs` — 时间分组 + 自动滚到底部

**文件**: `src/view/screens/chat_screen.rs`

#### 1.4.1 消息列表使用 `ListAlignment::Bottom` 自动滚到底部

**当前代码** (`chat_vm.rs` 第 85 行 和第 112 行):
```rust
chat.msg_list_state = Some(ListState::new(count, ListAlignment::Top, px(50.0)));
```

**改为**:
```rust
chat.msg_list_state = Some(ListState::new(count, ListAlignment::Bottom, px(50.0)));
```

涉及位置:
- `select_contact()` 第 85 行
- `load_more_messages()` 第 112 行 (此处保持 `Top`, 因为加载更早的消息应保持滚动位置)
- `handle_ws_message()` (`root_vm.rs` 第 111 行)

#### 1.4.2 消息列表中插入时间分割线

**改动说明**: 在 `message_panel` 的 list 渲染回调中, 比较相邻消息的 `timestamp`, 如果间隔超过 5 分钟则在上方插入时间分割线。

**方案**: 预处理消息列表, 构建一个 `Vec<ListItem>` 枚举:

在 `chat_screen.rs` 顶部新增:
```rust
/// 消息列表中的项目类型: 真实消息 或 时间分割线
#[derive(Clone)]
enum ListItem {
    Message(ChatMessage),
    TimeSeparator(String), // 格式化的时间文本
}

/// 将消息列表转化为包含时间分割线的列表。
/// 规则: 如果两条相邻消息的 timestamp 间隔 > 300 秒 (5分钟), 插入分割线。
fn build_list_items(msgs: &[ChatMessage]) -> Vec<ListItem> {
    let mut items = Vec::new();
    for (i, msg) in msgs.iter().enumerate() {
        if i == 0 || (msg.timestamp > 0 && msgs[i-1].timestamp > 0
            && msg.timestamp.saturating_sub(msgs[i-1].timestamp) > 300)
        {
            if msg.timestamp > 0 {
                items.push(ListItem::TimeSeparator(msg.created_at.clone()));
            }
        }
        items.push(ListItem::Message(msg.clone()));
    }
    items
}
```

在 `message_panel` 函数中, 将 `msgs_v` 替换为 `list_items`:
```rust
let list_items = build_list_items(&msgs_v);
let item_count = list_items.len();
```

List 渲染回调中:
```rust
list((*lst).clone(), move |ix, _window, _cx| {
    if ix >= list_items.len() { return div().into_any_element(); }
    match &list_items[ix] {
        ListItem::TimeSeparator(time_text) => {
            div().flex().flex_row().justify_center().py_2()
                .child(
                    div().px_3().py_1().rounded_full()
                        .bg(rgb(0x1a2a4a))
                        .text_size(px(11.0)).text_color(rgb(theme::CLR_MUTED))
                        .child(time_text.clone())
                )
                .into_any_element()
        }
        ListItem::Message(msg) => {
            crate::view::widgets::message_bubble::render(msg).into_any_element()
        }
    }
}).flex_1().into_any_element()
```

**注意**: `msg_list_state` 的 item count 需要改为 `item_count` (包含分割线), 需在 `chat_vm.rs` 中相应调整, 或者在渲染时动态计算。
最简实现: 在 `chat_screen.rs` 的 `message_panel` 中, 每次渲染时用 `build_list_items` 重新计算 count, 如果与当前 `msg_list_state` 的 count 不一致, 则重建 `ListState`。

---

### 1.5 `root_vm.rs` — WebSocket 推送消息补全新字段

**文件**: `src/viewmodel/root_vm.rs`

**当前代码** (`handle_ws_message`, 第 96-104 行):
```rust
let new_msg = ChatMessage {
    id: String::new(),
    sender_id,
    sender_name,
    text: text.clone(),
    created_at: String::new(),
    is_self,
};
```

**改为**:
```rust
let new_msg = ChatMessage {
    id: String::new(),
    sender_id,
    sender_name,
    sender_avatar: msg.data.get("from_user")
        .and_then(|u| u.get("profile_image_url"))
        .and_then(|v| v.as_str()).unwrap_or("").to_string(),
    text: text.clone(),
    created_at: String::new(),
    timestamp: std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_secs(),
    is_self,
    msg_type: crate::domain::MsgType::from_api(
        msg.data.get("type").and_then(|v| v.as_u64()).unwrap_or(321)
    ),
    media_type: crate::domain::MediaType::from_api(
        msg.data.get("media_type").and_then(|v| v.as_u64()).unwrap_or(0)
    ),
    fids: msg.data.get("fids")
        .and_then(|v| v.as_str())
        .map(|s| s.trim_matches(|c| c == '[' || c == ']')
            .split(',').filter(|s| !s.is_empty())
            .map(|s| s.trim().to_string()).collect())
        .unwrap_or_default(),
};
```

#### 1.5.1 新消息自动滚到底部

在 `handle_ws_message` 中追加消息后重建 `msg_list_state` 时使用 `ListAlignment::Bottom`:

**当前** (第 111 行):
```rust
chat.msg_list_state = Some(ListState::new(chat.messages.len(), ListAlignment::Top, px(50.0)));
```

**改为**:
```rust
chat.msg_list_state = Some(ListState::new(chat.messages.len(), ListAlignment::Bottom, px(50.0)));
```

---

### 1.6 编译适配 — 所有构造 `ChatMessage` 的位置

以下位置直接构造 `ChatMessage` 结构体, 需要补全新增字段:

| 文件 | 函数 | 行号 | 说明 |
|------|------|------|------|
| `src/model/chat_service.rs` | `fetch_group_messages` | 198-209 | 1.2.1 已覆盖 |
| `src/model/chat_service.rs` | `fetch_messages` | 260-282 | 1.2.2 已覆盖 |
| `src/model/chat_service.rs` | `send_dm_message` | 359-366 | 1.2.4 已覆盖 |
| `src/model/chat_service.rs` | `send_group_message` | 390-393 | 1.2.4 已覆盖 |
| `src/viewmodel/root_vm.rs` | `handle_ws_message` | 96-104 | 1.5 已覆盖 |

---

## 第二阶段：交互完善 (头像/表情/输入/已读)

### 2.1 `contact_card.rs` — 加载真实头像

**文件**: `src/view/widgets/contact_card.rs`

**改动说明**: 当前头像是首字母占位圆, 需要加载 `Contact.avatar` 的真实图片。GPUI 支持通过 `img()` 元素 + `ImageSource::Uri` 加载网络图片。

**当前代码** (第 16-21 行):
```rust
div()
    .w(px(40.0)).h(px(40.0)).rounded_full()
    .bg(rgb(theme::CLR_ACCENT))
    .flex().items_center().justify_center()
    .text_size(px(16.0)).text_color(rgb(0xffffff))
    .child(if contact.is_group { "群".to_string() } else { contact.screen_name.chars().next()... }),
```

**改为**:
```rust
// 检查是否有可用头像 URL
if !contact.avatar.is_empty() {
    img(ImageSource::Uri(contact.avatar.clone().into()))
        .w(px(40.0)).h(px(40.0)).rounded_full()
        .bg(rgb(theme::CLR_ACCENT)) // fallback背景色
        .into_any_element()
} else {
    // 无头像时使用首字占位
    div()
        .w(px(40.0)).h(px(40.0)).rounded_full()
        .bg(rgb(theme::CLR_ACCENT))
        .flex().items_center().justify_center()
        .text_size(px(16.0)).text_color(rgb(0xffffff))
        .child(if contact.is_group { "群".to_string() } else {
            contact.screen_name.chars().next().map(|c| c.to_string()).unwrap_or_default()
        })
        .into_any_element()
}
```

> **注意**: 需要确认 GPUI 0.2 版本是否支持 `ImageSource::Uri` 网络图片加载。如果不支持, 需要改为:
> 1. 在 `chat_service.rs` 中异步下载头像图片为 bytes
> 2. 将 bytes 存入 `Contact` 结构体
> 3. 使用 `img(ImageSource::Render(...))` 或 `SharedUri` 渲染

### 2.2 `message_bubble.rs` — 气泡中的头像也用同样方式

在 `render_normal` 中将 avatar 占位替换为条件加载:
```rust
let avatar = if !msg.sender_avatar.is_empty() {
    img(ImageSource::Uri(msg.sender_avatar.clone().into()))
        .w(px(36.0)).h(px(36.0)).rounded_full()
        .bg(rgb(0x3a4a6a))
        .flex_shrink_0()
        .into_any_element()
} else {
    div()
        .w(px(36.0)).h(px(36.0)).rounded_full()
        .bg(if is_self { rgb(theme::CLR_ACCENT) } else { rgb(0x3a4a6a) })
        .flex().items_center().justify_center()
        .text_size(px(14.0)).text_color(rgb(0xffffff)).flex_shrink_0()
        .child(avatar_char)
        .into_any_element()
};
```

### 2.3 表情面板 — 新增 `emoji_panel` widget

**新文件**: `src/view/widgets/emoji_panel.rs`

**改动说明**: 
- 在 `chat_service.rs` 新增 `fetch_emotions()` 函数调用 `GET /webim/emotions.json?source=209678993`
- 解析返回的表情列表: `[{phrase: "[不愧是你]", url: "https://...png", ...}]`
- 在 `ChatData` 中缓存表情列表
- 新增 widget 渲染表情选择面板 (Grid 布局, 点击插入 `[表情名]` 到 draft_text)

#### 2.3.1 Domain model 新增 `Emotion`

在 `src/domain/mod.rs` 添加:
```rust
/// 微博表情
#[derive(Clone, Debug)]
pub struct Emotion {
    /// 表情文本标记, 如 "[不愧是你]"
    pub phrase: String,
    /// 表情图片 URL
    pub url: String,
}
```

#### 2.3.2 `chat_service.rs` 新增 `fetch_emotions()`

```rust
/// 获取微博表情列表
pub async fn fetch_emotions() -> Vec<crate::domain::Emotion> {
    let (cookie, _xsrf) = chat_headers();
    let url = format!("{}/webim/emotions.json?source={}", CHAT_BASE, SOURCE);

    let client = http_client::build_no_store();
    match client.get(&url)
        .header("Cookie", &cookie)
        .header("Referer", format!("{}/chat", CHAT_BASE))
        .header("User-Agent", config::DEFAULT_UA)
        .timeout(config::REQUEST_TIMEOUT)
        .send().await
    {
        Ok(resp) => {
            if let Ok(arr) = resp.json::<Vec<serde_json::Value>>().await {
                return arr.iter().filter_map(|e| {
                    let phrase = e.get("phrase")?.as_str()?.to_string();
                    let url = e.get("url")?.as_str()?.to_string();
                    Some(crate::domain::Emotion { phrase, url })
                }).collect();
            }
        }
        Err(e) => log_info!("[chat] 获取表情失败: {}", e),
    }
    Vec::new()
}
```

#### 2.3.3 `ChatData` 新增字段

在 `src/viewmodel/chat_vm.rs` 的 `ChatData` 中:
```rust
pub struct ChatData {
    // ... 已有字段 ...
    /// 表情列表 (懒加载缓存)
    pub emotions: Vec<crate::domain::Emotion>,
    /// 是否显示表情面板
    pub show_emoji_panel: bool,
}
```

#### 2.3.4 `emoji_panel.rs` widget

```rust
//! Emoji panel widget — grid of clickable emoji items.

use gpui::*;
use crate::domain::Emotion;
use crate::view::theme;
use crate::viewmodel::root_vm::AppRoot;

pub fn render(emotions: &[Emotion], cx: &mut Context<AppRoot>) -> impl IntoElement {
    let emotions_owned: Vec<Emotion> = emotions.to_vec();
    let cols = 8; // 每行 8 个表情

    div()
        .flex().flex_col().w_full().max_h(px(200.0)).overflow_y_scroll()
        .bg(rgb(0x0d1b36)).border_t_1().border_color(rgb(0x1a2a4a))
        .px_2().py_2()
        .children(
            emotions_owned.chunks(cols).enumerate().map(|(row_idx, row)| {
                let row_items: Vec<Emotion> = row.to_vec();
                div().flex().flex_row().gap_1()
                    .children(row_items.into_iter().enumerate().map(|(col_idx, em)| {
                        let phrase = em.phrase.clone();
                        let display = phrase.trim_matches(|c| c == '[' || c == ']').to_string();
                        div()
                            .id(("emoji", row_idx * cols + col_idx))
                            .cursor_pointer()
                            .px_1().py_1().rounded_md()
                            .hover(|s| s.bg(rgb(0x1a2a4a)))
                            .text_size(px(12.0)).text_color(rgb(theme::CLR_TEXT))
                            .child(display)
                            .on_click(cx.listener(move |this, _: &ClickEvent, _window, cx| {
                                if let Some(chat) = this.chat_data.as_mut() {
                                    chat.draft_text.push_str(&phrase);
                                    chat.show_emoji_panel = false;
                                }
                                cx.notify();
                            }))
                    }))
            })
        )
}
```

#### 2.3.5 注册 widget module

`src/view/widgets/mod.rs` 添加:
```rust
pub mod emoji_panel;
```

#### 2.3.6 `input_bar` 添加表情按钮

在 `chat_screen.rs` 的 `input_bar` 函数中, 在发送按钮前添加表情按钮:
```rust
// 表情按钮
.child(
    div().id("emoji-btn").cursor_pointer()
        .px_2().py_2().rounded_lg()
        .text_size(px(18.0))
        .hover(|s| s.bg(rgb(0x1a2a4a)))
        .child("😊")
        .on_click(cx.listener(|this, _: &ClickEvent, _window, cx| {
            if let Some(chat) = this.chat_data.as_mut() {
                chat.show_emoji_panel = !chat.show_emoji_panel;
                // 首次打开时加载表情列表
                if chat.show_emoji_panel && chat.emotions.is_empty() {
                    // 异步加载 emotions
                    crate::viewmodel::chat_vm::load_emotions(cx, &this.tokio_handle);
                }
            }
            cx.notify();
        }))
)
```

在 `message_panel` 中, input_bar 上方条件渲染表情面板:
```rust
// 表情面板 (输入栏上方, 消息区域下方)
.child(if show_emoji_panel {
    let emotions = chat_emotions.clone();
    crate::view::widgets::emoji_panel::render(&emotions, cx).into_any_element()
} else {
    div().into_any_element()
})
.child(input_bar(&uid, is_group, draft, cx))
```

### 2.4 `chat_service.rs` — 已读上报

**新增函数**: `report_read()`

根据 HAR 中 `POST /webim/report.json` 的结构:
```rust
/// 上报已读状态 (进入/切换会话时调用)
pub async fn report_read(uid: &str) {
    let (cookie, xsrf) = chat_headers();
    let url = format!("{}/webim/report.json", CHAT_BASE);
    let client = http_client::build_no_store();

    let data_json = serde_json::json!({
        "type": 2,
        "uid": uid,
    });
    let params = format!("data={}&source={}", 
        url::form_urlencoded::byte_serialize(data_json.to_string().as_bytes()).collect::<String>(),
        SOURCE
    );

    match client.post(&url)
        .header("Cookie", &cookie)
        .header("Referer", format!("{}/chat", CHAT_BASE))
        .header("User-Agent", config::DEFAULT_UA)
        .header("Content-Type", "application/x-www-form-urlencoded")
        .header("X-XSRF-TOKEN", &xsrf)
        .body(params)
        .timeout(config::REQUEST_TIMEOUT)
        .send().await
    {
        Ok(_) => log_info!("[chat] 已读上报: uid={}", uid),
        Err(e) => log_info!("[chat] 已读上报失败: {}", e),
    }
}
```

在 `chat_vm::select_contact()` 中, 加载消息成功后调用:
```rust
// 在 this.update 闭包内, messages 赋值后:
let uid_report = uid.clone();
let handle_report = handle.clone();
tokio::spawn(async move {
    chat_service::report_read(&uid_report).await;
});
```

### 2.5 `input_bar` — 改进输入体验

**文件**: `src/view/screens/chat_screen.rs`

**改动说明**: 当前 `input_bar` 使用 `on_key_down` 逐字拦截, 不支持中文输入法(IME)。需要改为使用 GPUI 的 `InputEditor` 或更精细的 `on_input` 事件处理。

**当前 input_bar 问题**:
1. `on_key_down` 无法正确处理 IME 组合输入
2. 不支持 Shift+Enter 换行
3. 不支持 Ctrl+A/C/V 等快捷键

**改造方案** — 使用 `on_input` + `on_key_down` 配合:
```rust
fn input_bar(uid: &str, is_group: bool, draft: &str, cx: &mut Context<AppRoot>) -> impl IntoElement {
    let u1 = uid.to_string();
    let u2 = uid.to_string();
    let d = draft.to_string();

    div().flex().flex_row().items_center().gap_2()
        .px_3().py_2().bg(rgb(0x0d1b36)).border_t_1().border_color(rgb(0x1a2a4a))
        // 表情按钮 (2.3.6)
        .child(/* emoji button */)
        .child(
            div().id("msg-input").flex_1()
                .px_3().py_2().rounded_lg().bg(rgb(0x1a2a4a))
                .text_size(px(13.0)).text_color(rgb(theme::CLR_TEXT))
                .focusable()
                .on_input(cx.listener(move |this, ev: &InputEvent, _window, cx| {
                    // IME 友好: on_input 在 IME 确认后触发
                    if let Some(chat) = this.chat_data.as_mut() {
                        chat.draft_text.push_str(&ev.text);
                    }
                    cx.notify();
                }))
                .on_key_down(cx.listener(move |this, ev: &KeyDownEvent, _window, cx| {
                    let Some(chat) = this.chat_data.as_mut() else { return };
                    match ev.keystroke.key.as_str() {
                        "enter" | "return" if !ev.keystroke.modifiers.shift => {
                            // 普通 Enter 发送
                            let text = chat.draft_text.trim().to_string();
                            if !text.is_empty() {
                                chat.draft_text.clear();
                                chat_vm::send_message(cx, &this.tokio_handle, u1.clone(), text, is_group);
                            }
                        }
                        "enter" | "return" if ev.keystroke.modifiers.shift => {
                            // Shift+Enter 换行
                            chat.draft_text.push('\n');
                        }
                        "backspace" => { chat.draft_text.pop(); }
                        _ => {} // 其他字符由 on_input 处理
                    }
                    cx.notify();
                }))
                .child(if d.is_empty() {
                    div().text_color(rgb(theme::CLR_MUTED)).child("输入消息, Enter发送, Shift+Enter换行").into_any_element()
                } else {
                    div().text_color(rgb(theme::CLR_TEXT)).child(format!("{}", d)).into_any_element()
                }),
        )
        .child(/* send button */)
}
```

> **注意**: 需要确认 GPUI 0.2 是否有 `on_input` / `InputEvent`。如果不支持, 备选方案是使用 GPUI 的 `TextInput` 组件或保持当前方式, 在 `on_key_down` 中过滤掉 IME 前置键。

---

## 第三阶段：功能增强 (群聊/搜索/图片加载)

### 3.1 群成员侧栏

**新增 API**: `chat_service::fetch_group_info()`
```rust
/// 获取群详情 (成员列表/群名/管理员等)
/// API: GET /webim/query_group.json?is_pc=1&query_member=1&sort_by_jp=1&query_member_count=5000&id={gid}&source=209678993
pub async fn fetch_group_info(gid: &str) -> Option<GroupInfo> { ... }
```

**新增 Domain model**:
```rust
#[derive(Clone, Debug)]
pub struct GroupInfo {
    pub id: String,
    pub name: String,
    pub owner_uid: String,
    pub member_count: u64,
    pub members: Vec<GroupMember>,
}

#[derive(Clone, Debug)]
pub struct GroupMember {
    pub uid: String,
    pub screen_name: String,
    pub avatar: String,
    pub is_admin: bool,
}
```

**新增 widget**: `src/view/widgets/member_sidebar.rs`
- 右侧面板, 宽 180px
- 成员列表: 头像 + 昵称, 管理员标识
- 顶部显示群名和成员数

**`chat_screen.rs` 布局调整**:
```rust
// message_panel 最外层
div().flex().flex_row().flex_1().h_full()
    .child(/* 消息区域 flex_1 */)
    .child(if is_group { member_sidebar(...).into_any_element() } else { div().into_any_element() })
```

### 3.2 会话搜索

**`ChatData` 新增**:
```rust
pub search_text: String,
pub filtered_contacts: Vec<Contact>, // 搜索过滤后的联系人
```

**`chat_screen.rs` 联系人列表顶部新增搜索框**:
```rust
// contact_list 函数中, "会话 (N)" 标题下方
.child(
    div().px_2().py_1()
        .child(
            div().id("search-input").w_full().px_2().py_1()
                .rounded_md().bg(rgb(0x1a2a4a))
                .text_size(px(12.0)).text_color(rgb(theme::CLR_TEXT))
                .focusable()
                .on_key_down(cx.listener(|this, ev: &KeyDownEvent, _window, cx| {
                    if let Some(chat) = this.chat_data.as_mut() {
                        let ch = ev.keystroke.key_char.as_deref().unwrap_or("");
                        match ev.keystroke.key.as_str() {
                            "backspace" => { chat.search_text.pop(); }
                            _ if !ch.is_empty() => { chat.search_text.push_str(ch); }
                            _ => {}
                        }
                        // 过滤联系人
                        chat.filtered_contacts = chat.contacts.iter()
                            .filter(|c| c.screen_name.contains(&chat.search_text) || chat.search_text.is_empty())
                            .cloned().collect();
                    }
                    cx.notify();
                }))
                .child(if search_text.is_empty() {
                    div().text_color(rgb(theme::CLR_MUTED)).child("🔍 搜索")
                } else {
                    div().child(search_text.clone())
                })
        )
)
```

### 3.3 图片消息真实渲染

**改动说明**: 第一阶段的图片消息只显示占位符。本阶段加载真实图片缩略图。

**缩略图 URL 拼接规则** (来自 HAR):
```
https://upload.api.weibo.com/2/mss/msget_thumbnail?fid={fid}&high=240&width=240&size=240,240&source=209678993
```
群聊图片额外需要 `gid` 参数:
```
https://upload.api.weibo.com/2/mss/msget_thumbnail?fid={fid}&high=240&width=240&gid={gid}&size=240,240&source=209678993
```

**`message_bubble.rs` 中 `render_image_bubble` 改为**:
```rust
fn render_image_bubble(msg: &ChatMessage, bg: Hsla, fg: Hsla) -> AnyElement {
    let image_elements: Vec<AnyElement> = msg.fids.iter().map(|fid| {
        let thumb_url = format!(
            "https://upload.api.weibo.com/2/mss/msget_thumbnail?fid={}&high=240&width=240&size=240,240&source=209678993",
            fid
        );
        img(ImageSource::Uri(thumb_url.into()))
            .w(px(200.0)).h(px(150.0))
            .rounded_md()
            .bg(rgb(0x2a3a5a)) // 加载中的背景色
            .into_any_element()
    }).collect();

    div()
        .px_3().py_2().rounded_lg().bg(bg)
        .child(
            div().flex().flex_col().gap_1()
                .children(image_elements)
                .child(if !msg.text.is_empty() && msg.text != "分享图片" {
                    div().text_size(px(13.0)).text_color(fg).child(msg.text.clone()).into_any_element()
                } else {
                    div().into_any_element()
                })
        )
        .into_any_element()
}
```

> **Cookie 问题**: 图片 URL 需要 Cookie 鉴权。GPUI 的 `img()` 可能不支持自定义 Header。
> 备选方案: 在 `chat_service.rs` 中预下载图片 bytes, 将 `Vec<u8>` 传给渲染层。

### 3.4 新消息提示音

**新文件**: `src/infra/audio.rs`

```rust
//! Simple audio playback for new message notification.

use std::io::Cursor;

/// 播放新消息提示音 (chat.mp3 内嵌资源)
pub fn play_notification() {
    // 使用 rodio crate 播放音频
    std::thread::spawn(|| {
        // 内嵌的提示音 bytes (编译时 include_bytes! 或运行时下载)
        let audio_data = include_bytes!("../../assets/chat.mp3");
        // 需要 rodio 依赖
        if let Ok((_stream, stream_handle)) = rodio::OutputStream::try_default() {
            if let Ok(source) = rodio::Decoder::new(Cursor::new(audio_data)) {
                let _ = stream_handle.play_raw(source.convert_samples());
                std::thread::sleep(std::time::Duration::from_secs(2));
            }
        }
    });
}
```

**Cargo.toml 新增**:
```toml
rodio = "0.19"
```

**调用位置**: `root_vm.rs` 的 `handle_ws_message`, 收到非自己的消息时:
```rust
if !is_self {
    crate::infra::audio::play_notification();
}
```

### 3.5 消息角色标识 (群主/管理员)

**Domain model** `ChatMessage` 新增:
```rust
/// 消息发送者在群中的角色 (0=普通, 1=管理员, 4=群主)
pub role: u8,
```

**`message_bubble.rs` 渲染** — `name_time` 区域追加角色标签:
```rust
let role_badge = match msg.role {
    4 => Some(("群主", 0xe8633a)),  // 橙色
    1 => Some(("管理员", 0x4a9eff)), // 蓝色
    _ => None,
};

let name_time = div()
    .flex().flex_row().gap_2().px_1()
    .text_size(px(11.0)).text_color(rgb(theme::CLR_MUTED))
    .child(if is_self { "我".to_string() } else { msg.sender_name.clone() })
    .when_some(role_badge, |d, (label, color)| {
        d.child(
            div().px_1().rounded_sm()
                .bg(rgb(color))
                .text_size(px(10.0)).text_color(rgb(0xffffff))
                .child(label)
        )
    })
    .child(msg.created_at.clone());
```

---

## 改造文件清单

| 文件 | 阶段 | 改动类型 |
|------|------|---------|
| `src/domain/mod.rs` | 1 | 修改: 新增 `MsgType`/`MediaType` 枚举, 扩展 `ChatMessage` |
| `src/model/chat_service.rs` | 1+2 | 修改: 解析新字段, 新增 `format_timestamp`/`fetch_emotions`/`report_read` |
| `src/view/widgets/message_bubble.rs` | 1 | 重写: 支持文本/图片/引用/系统消息渲染 |
| `src/view/screens/chat_screen.rs` | 1+2 | 修改: 时间分割线, 表情面板, 输入改进 |
| `src/viewmodel/chat_vm.rs` | 1+2 | 修改: `ListAlignment::Bottom`, 新增 `emotions`/`show_emoji_panel` 字段 |
| `src/viewmodel/root_vm.rs` | 1 | 修改: WS 消息补全字段 |
| `src/view/widgets/contact_card.rs` | 2 | 修改: 真实头像加载 |
| `src/view/widgets/emoji_panel.rs` | 2 | **新增**: 表情选择面板 |
| `src/view/widgets/member_sidebar.rs` | 3 | **新增**: 群成员侧栏 |
| `src/view/widgets/mod.rs` | 2+3 | 修改: 注册新 widget |
| `src/infra/audio.rs` | 3 | **新增**: 提示音播放 |
| `src/infra/mod.rs` | 3 | 修改: 注册 audio 模块 |
| `Cargo.toml` | 3 | 修改: 新增 `rodio` 依赖 |

---

## 风险 & 待确认项

1. **GPUI `img()` 网络图片**: 需确认 `gpui 0.2` 是否支持 `ImageSource::Uri` 加载远程图片, 以及是否能自定义 HTTP Header (Cookie 鉴权)。如不支持, 需改为手动下载 + `ImageSource::from_data(bytes)` 方案。
2. **GPUI `on_input` / `InputEvent`**: 需确认是否有 IME 友好的输入事件。如不存在, 保持当前 `on_key_down` 方案, 暂不支持中文 IME。
3. **时间格式化精度**: `format_timestamp` 使用简化月/日计算, 非闰年安全。可考虑引入 `chrono` crate 做精确格式化。
4. **图片加载性能**: 群聊可能有大量图片消息, 需要考虑虚拟列表中的懒加载策略, 避免同时发起过多 HTTP 请求。
5. **`rodio` 跨平台**: 音频库在 Linux/macOS/Windows 上的可用性需验证, 可能需要 ALSA/PulseAudio 等系统依赖。
