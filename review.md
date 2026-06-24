# 代码 Review — 聊天界面改造后崩溃问题分析

> 基于对整个 codebase 的逐文件 review，结合 GPUI 0.2 框架源码分析得出。

---

## 🔴 致命问题 (可能导致崩溃/panic)

### 1. ListState item_count 与实际渲染项数不匹配 — 最大嫌疑崩溃点

**文件**: `src/viewmodel/chat_vm.rs` 第 100 行, `src/view/screens/chat_screen.rs` 第 251 行

**问题**: `ListState::new(count, ...)` 中 `count` 使用的是 `chat.messages.len()`（原始消息条数），但渲染时使用了 `build_list_items(msgs)` 构建的列表，该列表会插入 `TimeSeparator` 项，导致**实际渲染项数 > ListState 声明的 item_count**。

GPUI 的 `List` 元素根据 `item_count` 决定哪些 index 可以调用 `render_item`。当 GPUI 认为只有 N 项但回调中实际有 N+M 项时，**GPUI 不会调用超出 item_count 的索引**，导致部分消息不可见；或者当内部布局计算与实际不符时，产生 panic。

```rust
// chat_vm.rs:100 — 只按消息数创建
chat.msg_list_state = Some(ListState::new(count, ListAlignment::Bottom, px(50.0)));

// chat_screen.rs:251 — 但实际渲染包含了额外的 TimeSeparator 项
let list_items = build_list_items(msgs); // len > msgs.len()
```

**修复方案**: 在创建 `ListState` 时使用包含分割线的真实总数:
```rust
// 方案 A: 在 chat_vm.rs 中预计算 (需要将 build_list_items 逻辑抽到 domain/viewmodel 层)
let item_count = compute_list_item_count(&chat.messages);
chat.msg_list_state = Some(ListState::new(item_count, ListAlignment::Bottom, px(50.0)));

// 方案 B: 在 chat_screen.rs 渲染时检测并 splice
if let Some(ref lst) = msg_list_state {
    let expected = lst.item_count();
    let actual = list_items.len();
    if expected != actual {
        lst.splice(0..expected, actual); // 重置为正确数量
    }
}
```

---

### 2. `send_message` 后不更新 `msg_list_state`

**文件**: `src/viewmodel/chat_vm.rs` 第 162-166 行

**问题**: 发送消息后 `chat.messages.push(msg)` 增加了消息数量，但没有重建/splice `msg_list_state`。ListState 内部记录的 item_count 仍是旧值。下次渲染时:
- GPUI 不知道新增了一项 → 新消息不显示
- 当 `build_list_items` 生成的列表长度超过 ListState 已知数量 → 潜在越界

```rust
// chat_vm.rs:162-166
this.update(&mut cx, |v, cx| {
    if let Some(chat) = v.chat_data.as_mut() {
        if let Some(msg) = sent {
            chat.messages.push(msg);
            // ❌ 缺少: 更新 msg_list_state
        }
    }
    cx.notify();
}).ok();
```

**修复方案**:
```rust
if let Some(msg) = sent {
    chat.messages.push(msg);
    // 重建 ListState (需要用包含 TimeSeparator 的真实数量)
    let item_count = compute_list_item_count(&chat.messages);
    chat.msg_list_state = Some(ListState::new(item_count, ListAlignment::Bottom, px(50.0)));
}
```

---

### 3. `&text[..text.len().min(20)]` 中文字符串切片 panic

**文件**: `src/model/chat_service.rs` 第 375 行和第 438 行

**问题**: 中文/emoji 等多字节 UTF-8 字符，按字节数切片 `text[..20]` 可能切在字符中间，导致:
```
thread 'main' panicked at 'byte index 20 is not a char boundary'
```

**触发条件**: 用户发送任何中文消息（几乎必然触发）。

```rust
// chat_service.rs:375
log_info!("[chat] 发送消息: uid={}, text={}...", uid, &text[..text.len().min(20)]);
// 例: text = "你好世界测试消息" → 每个中文字 3 bytes → 20 bytes 切在第7个字中间 → panic!

// chat_service.rs:438
log_info!("[chat] Group send: gid={}, text={}...", gid, &text[..text.len().min(20)]);
```

**修复方案**:
```rust
// 安全的字符截取
let preview: String = text.chars().take(20).collect();
log_info!("[chat] 发送消息: uid={}, text={}...", uid, preview);
```

---

### 4. `fids` 解析逻辑错误 — JSON array 被当作 string 解析

**文件**: `src/model/chat_service.rs` 第 205-215 行, 第 291-297 行

**问题**: 通过 HAR 抓包验证，API 返回的 `fids` 字段是 **JSON 数组** `[5312697042208502]`（`serde_json::Value::Array`），而非字符串。代码使用 `.as_str()` 对数组调用永远返回 `None`，导致图片消息的 `fids` 永远为空 vec。

```rust
// 当前代码（错误）:
let fids = m.get("fids")
    .and_then(|v| v.as_str())  // ← Array Value 上调用 as_str() 永远返回 None!
    .map(|s| s.trim_matches(|c| c == '[' || c == ']')
        .split(',')
        .filter(|s| !s.is_empty())
        .map(|s| s.trim().to_string())
        .collect::<Vec<_>>())
    .unwrap_or_default();
```

**修复方案**:
```rust
let fids = m.get("fids")
    .and_then(|v| v.as_array())
    .map(|arr| arr.iter()
        .filter_map(|v| v.as_u64().map(|n| n.to_string()))
        .collect::<Vec<_>>())
    .unwrap_or_default();
```

---

## 🟠 严重问题 (功能缺陷/潜在 panic)

### 5. `tokio::spawn` 在非 Tokio runtime 上下文中调用

**文件**: `src/view/screens/chat_screen.rs` 第 381 行, `src/viewmodel/chat_vm.rs` 第 108, 115 行

**问题**: GPUI 的 UI 线程不在 tokio runtime 上下文中。`cx.spawn` 产生的是 GPUI 管理的 async task，其内部也不保证有 tokio runtime。直接使用 `tokio::spawn()` 可能导致:
- panic: `"there is no reactor running, must be called from the context of a Tokio runtime"`
- 或者（如果恰好在 `handle.block_on` 内部调用）tokio 会自动检测到嵌套 runtime 而 panic

```rust
// chat_screen.rs:381 — 在 GPUI on_click listener 中 (UI 线程!)
tokio::spawn(async move {
    let emotions = crate::model::chat_service::fetch_emotions().await;
    let _ = emotions;  // 结果直接丢弃!
    let _ = handle;
});

// chat_vm.rs:108 — 在 cx.spawn 的 async block 中
tokio::spawn(async move {
    if let Some(info) = chat_service::fetch_group_info(&gid).await {
        log_info!("[chat_vm] 群信息已获取: {:?}", info.name);
        // 结果无法回传到 AppRoot!
    }
});

// chat_vm.rs:115
tokio::spawn(async move {
    chat_service::report_read(&uid_for_report).await;
});
```

**修复方案**: 使用保存的 `tokio_handle.spawn(...)` 替代裸 `tokio::spawn(...)`:
```rust
// 正确做法:
let handle = this.tokio_handle.clone();
handle.spawn(async move {
    chat_service::report_read(&uid_for_report).await;
});
```
对于需要回传结果的情况，应使用 `cx.spawn` + `handle.block_on` 模式。

---

### 6. 表情面板加载是"火后不管" — emotions 永远为空

**文件**: `src/view/screens/chat_screen.rs` 第 378-389 行

**问题**: 表情按钮点击后尝试加载表情列表，但:
1. 使用了裸 `tokio::spawn`（见 #5，可能 panic）
2. 即使不 panic，加载结果也直接丢弃 `let _ = emotions;`
3. `chat.emotions` 永远不会被填充 → 面板永远空白

```rust
if chat.show_emoji_panel && chat.emotions.is_empty() {
    let handle = this.tokio_handle.clone();
    tokio::spawn(async move {
        let emotions = crate::model::chat_service::fetch_emotions().await;
        let _ = emotions;  // ← 加载了但直接丢弃!
        let _ = handle;
    });
}
```

**修复方案**: 使用 `cx.spawn` + `WeakEntity` 回写:
```rust
if chat.show_emoji_panel && chat.emotions.is_empty() {
    let handle = this.tokio_handle.clone();
    cx.spawn(|this: WeakEntity<AppRoot>, cx: &mut AsyncApp| {
        let mut cx = cx.clone();
        async move {
            let emotions = handle.block_on(chat_service::fetch_emotions());
            this.update(&mut cx, |v, cx| {
                if let Some(chat) = v.chat_data.as_mut() {
                    chat.emotions = emotions;
                }
                cx.notify();
            }).ok();
        }
    }).detach();
}
```

---

### 7. `contact_card.rs` 两个分支代码完全相同 — 头像未真正加载

**文件**: `src/view/widgets/contact_card.rs` 第 10-34 行

**问题**: `if !contact.avatar.is_empty()` 和 `else` 分支渲染的内容完全一样，都是首字母占位圆。头像加载功能实际未实现，但代码结构暗示已实现。不会导致崩溃，但功能完全失效。

---

### 8. `message_bubble.rs` 两个分支代码完全相同 — 头像未真正加载

**文件**: `src/view/widgets/message_bubble.rs` 第 43-59 行

**问题**: 同上，`if !msg.sender_avatar.is_empty()` 和 `else` 分支完全相同，`sender_avatar` URL 未被实际使用。

---

### 9. `handle_ws_message` 中 `msg_list_state` 更新条件有 bug

**文件**: `src/viewmodel/root_vm.rs` 第 129-132 行

**问题**: 仅在 `msg_list_state` 已经为 `Some` 时才更新。如果 WS 推送消息到来时 `msg_list_state` 为 `None`（例如用户还没点击过任何联系人），则 `messages.push(new_msg)` 了但列表不会渲染。

更严重的是：更新时使用 `chat.messages.len()` 而非包含 TimeSeparator 的真实数量（同问题 #1）。

```rust
if let Some(ref lst) = chat.msg_list_state {
    // lst 被绑定但未使用
    chat.msg_list_state = Some(ListState::new(chat.messages.len(), ListAlignment::Bottom, px(50.0)));
}
```

---

## 🟡 中等问题 (逻辑缺陷/可靠性)

### 10. `load_more_messages` 中重建 ListState 导致滚动位置丢失

**文件**: `src/viewmodel/chat_vm.rs` 第 141 行

**问题**: 加载更早消息后 `ListState::new(..., ListAlignment::Top, ...)` 创建全新的 ListState。由于是全新实例，滚动位置重置到顶部。用户点击"加载更早消息"后，视图会突然跳到最顶部，而非停留在之前阅读的位置。

**修复方案**: 使用 `lst.splice(0..0, new_count)` 在现有 ListState 头部插入新项，保持滚动位置。

---

### 11. 搜索过滤后 `contact_list_state` 的 item_count 未更新

**文件**: `src/view/screens/chat_screen.rs` 第 182 行

**问题**: 搜索过滤后 `filtered` 的长度可能小于原始联系人数，但传入 `list()` 的 `list_state`（即 `chat_list_state`）的 item_count 仍是全部联系人的数量。

虽然渲染回调中有 `if ix >= count { return div() }` 保护，但 GPUI 仍会为多余的项分配布局空间，导致列表底部出现大量空白区域，用户体验差。

---

### 12. DM 消息的 `timestamp` 始终为 0 — 时间分割线失效

**文件**: `src/model/chat_service.rs` 第 325 行

```rust
timestamp: 0, // DM 接口是字符串时间, 可后续解析
```

**影响**: `build_list_items` 中的时间分割线逻辑判断 `msg.timestamp > 0`，DM 消息全部跳过 → DM 会话完全没有时间分隔。

**修复方案**: 解析 DM 的 `created_at` 字符串（格式如 `"Wed Jun 24 12:07:17 +0800 2026"`）为 Unix 时间戳。

---

### 13. `format_timestamp` 月/日计算不准确

**文件**: `src/model/chat_service.rs` 第 619-630 行

**问题**: 使用 `days_since_year_start / (30 * secs_in_day)` 计算月份是非常粗略的近似。1月=31天，2月=28天，各月天数不同。可能导致显示的日期与实际日期偏差数天。

```rust
let days_since_year_start = local_ts % (365 * secs_in_day);
let month_approx = (days_since_year_start / (30 * secs_in_day)) + 1;
let day_approx = ((days_since_year_start % (30 * secs_in_day)) / secs_in_day) + 1;
```

**修复方案**: 引入 `chrono` crate 或使用更精确的日历算法。

---

### 14. emoji_panel 布局位置不合理 — 在消息列表上方

**文件**: `src/view/screens/chat_screen.rs` 第 300 行 vs 第 306 行

消息面板的 `.child` 顺序是:
1. Header
2. Load-more button
3. **Emoji Panel** ← 位置错误
4. Message List
5. Input Bar

表情面板应该在 Input Bar 上方（Message List 和 Input Bar 之间），而非在消息列表上方。当前布局会导致:
- 打开表情面板时，消息列表被向下压缩
- 表情面板离输入框很远，用户需要先看到面板、选完后再看下方输入框

---

## 🔵 低优先级问题

### 15. `member_sidebar.rs` 已创建但未被集成

widget 在 `mod.rs` 中注册，但 `chat_screen.rs` 中没有调用 `member_sidebar::render()`。群聊时不会显示成员列表侧栏。

### 16. `ChatData.filtered_contacts` 字段冗余

`ChatData` 中有 `filtered_contacts` 字段，但搜索过滤逻辑在 `chat_screen::render()` 中每次渲染时重新计算，未使用此缓存字段。属于死代码。

### 17. `fetch_group_info` 解析 members 字段可能不正确

**文件**: `src/model/chat_service.rs` 第 739-773 行

HAR 数据显示 `members` 字段可能是 UID 数组 `[1744323680]` 而非对象数组。代码尝试从每个元素中提取 `uid`/`screen_name` 等字段，对纯数字 UID 数组会全部返回空。真正的成员信息在 `member_infos` 字段中。

### 18. `root_vm.rs` 硬编码 UID fallback

**文件**: `src/viewmodel/root_vm.rs` 第 239 行

```rust
.unwrap_or_else(|| "1744323680".to_string());
```

当 `chat_data` 未加载时，WebSocket 使用硬编码的 UID。这在其他用户的设备上会连接到错误的频道。

---

## 崩溃根因优先级排序

| 优先级 | 问题编号 | 描述 | 崩溃可能性 | 触发条件 |
|--------|---------|------|-----------|---------|
| **P0** | #1 | ListState count 与 TimeSeparator 实际项数不匹配 | **极高** | 进入任何有多条消息的聊天 |
| **P0** | #2 | send_message 后不更新 msg_list_state | **极高** | 发送任何一条消息 |
| **P0** | #3 | 中文字符串 byte 切片 | **极高** | 发送任何中文消息 |
| **P1** | #5 | tokio::spawn 在非 runtime 上下文 | **高** | 点击表情按钮、切换群聊、发送消息 |
| **P2** | #11 | 搜索后 list_state count 不匹配 | **中** | 使用搜索功能 |
| **P2** | #9 | WS 新消息时 list count 不匹配 | **中** | 收到 WebSocket 推送消息 |

---

## 建议修复顺序

1. **立即修复 #3** — 最简单，一行代码改两处
2. **修复 #1 + #2 + #9** — 统一解决 ListState count 同步问题，建议抽取公共函数 `rebuild_msg_list_state(chat: &mut ChatData)`
3. **修复 #5** — 将所有 `tokio::spawn` 替换为 `handle.spawn` 或 `cx.spawn`
4. **修复 #6** — 表情加载逻辑重写
5. **修复 #4** — fids 解析修正
6. 其余问题按需修复
