# 微博 PC 客户端 - 聊天界面 Bug 修复计划

> 基于 `failed.png` 截图分析的 4 个 UI 问题，结合 `review.md` 中的代码审查结论。

---

## 问题一览

| # | 问题描述 | 根因 | 涉及文件 |
|---|---------|------|---------|
| 1 | 未读数角标被拉长 | `rounded_full` + 无固定最小宽高，数字较长时胶囊变形 | `contact_card.rs` |
| 2 | 聊天消息互相覆盖 | ListState item_count 与实际项数不一致 + 估算行高过小 | `chat_vm.rs`, `chat_screen.rs`, `root_vm.rs` |
| 3 | 图片渲染失败 | `fids` 解析逻辑错误(JSON array 被 `as_str()`) + 占位图无内容 | `chat_service.rs`, `message_bubble.rs` |
| 4 | 滚轮无法触发加载更多 | 消息列表无 scroll_handler，仅有手动按钮 | `chat_screen.rs`, `chat_vm.rs` |

---

## 修复一: 未读数角标拉长

### 1.1 问题分析

截图中右侧会话列表的未读数角标（如 "22"、"3"）被水平拉长，变成了椭圆/长条形状。

**根因**: `contact_card.rs` 第 57 行：
```rust
div().px_2().py_0p5().rounded_full().bg(rgb(theme::CLR_ACCENT))
    .text_size(px(11.0)).text_color(rgb(0xffffff))
    .child(format!("{}", contact.unread_count))
```

- `rounded_full()` 的圆角是 9999px，在正方形上显示为圆形，但在矩形上显示为胶囊。
- 没有设置 `min_w` / `h` / `items_center` / `justify_center`，内容区域由文本撑开，两位数以上就变形。

### 1.2 修复方案

**文件**: `src/view/widgets/contact_card.rs` 第 56-61 行

**当前代码**:
```rust
.child(if contact.unread_count > 0 {
    div().px_2().py_0p5().rounded_full().bg(rgb(theme::CLR_ACCENT))
        .text_size(px(11.0)).text_color(rgb(0xffffff))
        .child(format!("{}", contact.unread_count))
        .into_any_element()
} else {
    div().into_any_element()
})
```

**改为**:
```rust
.child(if contact.unread_count > 0 {
    let text = if contact.unread_count > 99 {
        "99+".to_string()
    } else {
        format!("{}", contact.unread_count)
    };
    div()
        .min_w(px(18.0))   // 最小宽度，保证单位数也是圆形
        .h(px(18.0))       // 固定高度
        .flex()            // 启用 flex 以便居中
        .items_center()
        .justify_center()
        .rounded_full()
        .bg(rgb(theme::CLR_ACCENT))
        .text_size(px(10.0))
        .text_color(rgb(0xffffff))
        .flex_shrink_0()   // 不被父容器压缩
        .child(text)
        .into_any_element()
} else {
    div().into_any_element()
})
```

**关键改动**:
- `min_w(px(18.0))` + `h(px(18.0))` 保证单位数时是正圆
- `flex()` + `items_center()` + `justify_center()` 让文字居中
- `flex_shrink_0()` 防止被父级 `justify_between` 压缩
- `text_size` 减小到 10px 避免撑破容器
- 超过 99 截断为 "99+"

---

## 修复二: 聊天消息互相覆盖

### 2.1 问题分析

截图中可明显看到多条消息的气泡在垂直方向上重叠，文字互相遮挡。

**根因（3 重叠加）**:

1. **ListState item_count 不匹配**: `ListState::new(count, ...)` 中 `count` 是 `chat.messages.len()` (原始消息数)，但渲染时 `build_list_items()` 会插入 `TimeSeparator` 使实际项数更多。GPUI 只分配 `count` 个 slot 的布局空间，多余的项被挤压/重叠。

2. **overdraw 过小**: `ListState::new(count, ..., px(50.0))` 的 overdraw=50px，意味着 GPUI 只预渲染可视区域上下 50px 的项。消息气泡（尤其带引用的）高度远超 50px，导致测量不足、布局错位。

3. **send_message 后不更新 ListState**: 发消息追加后 ListState 不知道新增了项，下次渲染时内部状态错乱。

### 2.2 修复方案

#### 2.2.1 统一 ListState 创建逻辑 — 新增辅助函数

**文件**: `src/viewmodel/chat_vm.rs`

在文件顶部（`ChatData` 结构体定义之后）新增:

```rust
/// 计算包含时间分割线的消息列表真实项数。
/// 规则: 第一条消息前 + 间隔超过 300 秒的相邻消息之间，各插入一条 TimeSeparator。
pub fn compute_list_item_count(messages: &[ChatMessage]) -> usize {
    let mut count = messages.len();
    for (i, msg) in messages.iter().enumerate() {
        if i == 0 {
            if msg.timestamp > 0 {
                count += 1;
            }
        } else if msg.timestamp > 0 && messages[i - 1].timestamp > 0
            && msg.timestamp.saturating_sub(messages[i - 1].timestamp) > 300
        {
            count += 1;
        }
    }
    count
}

/// 为消息列表创建/重建 ListState，使用正确的 item_count 和 overdraw。
pub fn rebuild_msg_list_state(messages: &[ChatMessage], alignment: ListAlignment) -> ListState {
    let count = compute_list_item_count(messages);
    // overdraw 设为 400px: 消息气泡最高约 200px (引用+长文本), 需要至少预渲染 2 条
    ListState::new(count, alignment, px(400.0))
}
```

#### 2.2.2 修改 `select_contact` — 使用正确的 item_count

**文件**: `src/viewmodel/chat_vm.rs` 第 100 行

**当前**:
```rust
chat.msg_list_state = Some(ListState::new(count, ListAlignment::Bottom, px(50.0)));
```

**改为**:
```rust
chat.msg_list_state = Some(rebuild_msg_list_state(&chat.messages, ListAlignment::Bottom));
```

#### 2.2.3 修改 `load_more_messages` — 使用正确的 item_count

**文件**: `src/viewmodel/chat_vm.rs` 第 141 行

**当前**:
```rust
chat.msg_list_state = Some(ListState::new(chat.messages.len(), ListAlignment::Top, px(50.0)));
```

**改为**:
```rust
chat.msg_list_state = Some(rebuild_msg_list_state(&chat.messages, ListAlignment::Top));
```

#### 2.2.4 修改 `send_message` — 追加消息后更新 ListState

**文件**: `src/viewmodel/chat_vm.rs` 第 162-166 行

**当前**:
```rust
if let Some(msg) = sent {
    chat.messages.push(msg);
}
```

**改为**:
```rust
if let Some(msg) = sent {
    chat.messages.push(msg);
    chat.msg_list_state = Some(rebuild_msg_list_state(&chat.messages, ListAlignment::Bottom));
}
```

#### 2.2.5 修改 `root_vm.rs` `handle_ws_message` — WS 推送也更新 ListState

**文件**: `src/viewmodel/root_vm.rs` 第 127-132 行

**当前**:
```rust
if chat.selected_uid.as_ref() == Some(&contact_uid) {
    chat.messages.push(new_msg);
    if let Some(ref lst) = chat.msg_list_state {
        chat.msg_list_state = Some(ListState::new(chat.messages.len(), ListAlignment::Bottom, px(50.0)));
    }
}
```

**改为**:
```rust
if chat.selected_uid.as_ref() == Some(&contact_uid) {
    chat.messages.push(new_msg);
    // 始终重建 ListState (不论之前是否为 Some)
    chat.msg_list_state = Some(
        crate::viewmodel::chat_vm::rebuild_msg_list_state(&chat.messages, ListAlignment::Bottom)
    );
}
```

#### 2.2.6 `chat_screen.rs` — 渲染时校验 item_count（防御性）

**文件**: `src/view/screens/chat_screen.rs` 第 306-332 行

在使用 `msg_list_state` 前，检测实际 item count 是否匹配:

**当前**:
```rust
.child(if !msgs.is_empty() {
    if let Some(lst) = msg_list_state {
        let items_for_list = list_items.clone();
        list(lst, move |ix, _window, _cx| { ... })
```

**改为**:
```rust
.child(if !msgs.is_empty() {
    if let Some(lst) = msg_list_state {
        let items_for_list = list_items.clone();
        let actual_count = items_for_list.len();
        // 防御性校验: 如果 ListState 的 item_count 与实际不符, splice 修正
        let state_count = lst.item_count();
        if state_count != actual_count {
            if state_count < actual_count {
                lst.splice(state_count..state_count, actual_count - state_count);
            } else {
                lst.splice(actual_count..state_count, 0);
            }
        }
        list(lst, move |ix, _window, _cx| { ... })
```

> 注: `lst.item_count()` 和 `lst.splice()` 是 GPUI `ListState` 的公开 API。

---

## 修复三: 图片渲染失败

### 3.1 问题分析

截图中看到图片消息区域显示为空白/灰色块，而非预期的 "🖼" 占位图标。

**根因**:
1. `fids` 解析使用 `as_str()`，但 API 返回的是 JSON 数组 `[5312697042208502]`（`Value::Array`），所以 `as_str()` 永远返回 `None`，`fids` 为空。
2. 当 `fids` 为空时，`media_type` 仍然是 `Image`（因为 `media_type=1`），进入 `render_image_bubble`。但该函数目前只渲染一个固定大小的灰色块 + 文字，在某些布局条件下可能被压缩不可见。

### 3.2 修复方案

#### 3.2.1 修复 fids 解析 — 正确处理 JSON array

**文件**: `src/model/chat_service.rs`

**位置 1**: `fetch_group_messages` 第 204-215 行

**当前**:
```rust
let fids = m
    .get("fids")
    .and_then(|v| v.as_str())
    .map(|s| {
        s.trim_matches(|c| c == '[' || c == ']')
            .split(',')
            .filter(|s| !s.is_empty())
            .map(|s| s.trim().to_string())
            .collect::<Vec<_>>()
    })
    .unwrap_or_default();
```

**改为**:
```rust
let fids = m.get("fids")
    .and_then(|v| {
        // API 返回 JSON array: [5312697042208502]
        if let Some(arr) = v.as_array() {
            Some(arr.iter()
                .filter_map(|item| {
                    item.as_u64().map(|n| n.to_string())
                        .or_else(|| item.as_str().map(|s| s.to_string()))
                })
                .collect::<Vec<_>>())
        } else if let Some(s) = v.as_str() {
            // 兼容字符串格式: "[123,456]"
            Some(s.trim_matches(|c| c == '[' || c == ']')
                .split(',')
                .filter(|s| !s.is_empty())
                .map(|s| s.trim().to_string())
                .collect())
        } else {
            None
        }
    })
    .unwrap_or_default();
```

**位置 2**: `fetch_messages` (DM) 第 291-297 行

**当前**:
```rust
let fids_str = m.get("fids").and_then(|v| v.as_str()).unwrap_or("");
let fids = fids_str
    .trim_matches(|c| c == '[' || c == ']')
    .split(',')
    .filter(|s| !s.is_empty())
    .map(|s| s.trim().to_string())
    .collect::<Vec<_>>();
```

**改为** (同样逻辑):
```rust
let fids = m.get("fids")
    .and_then(|v| {
        if let Some(arr) = v.as_array() {
            Some(arr.iter()
                .filter_map(|item| {
                    item.as_u64().map(|n| n.to_string())
                        .or_else(|| item.as_str().map(|s| s.to_string()))
                })
                .collect::<Vec<_>>())
        } else if let Some(s) = v.as_str() {
            Some(s.trim_matches(|c| c == '[' || c == ']')
                .split(',')
                .filter(|s| !s.is_empty())
                .map(|s| s.trim().to_string())
                .collect())
        } else {
            None
        }
    })
    .unwrap_or_default();
```

#### 3.2.2 优化图片占位渲染 — 确保可见性

**文件**: `src/view/widgets/message_bubble.rs` 第 115-135 行

**当前**:
```rust
fn render_image_bubble(msg: &ChatMessage, _bg: Rgba, fg: Rgba) -> AnyElement {
    div()
        .px_3().py_2().rounded_lg().bg(_bg)
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
                ...
```

**改为**:
```rust
fn render_image_bubble(msg: &ChatMessage, _bg: Rgba, fg: Rgba) -> AnyElement {
    // 构建图片描述文本
    let desc = if msg.fids.is_empty() {
        "[图片]".to_string()
    } else {
        format!("[图片 x{}]", msg.fids.len())
    };

    div()
        .px_3().py_2().rounded_lg().bg(_bg)
        .child(
            div().flex().flex_col().gap_1()
                .child(
                    div()
                        .w(px(180.0))        // 稍微缩小避免撑破 max_w
                        .h(px(100.0))
                        .rounded_md()
                        .bg(rgb(0x1e2e4e))   // 更深的背景色以便区分
                        .border_1()
                        .border_color(rgb(0x2a3a5a))
                        .flex()
                        .flex_col()
                        .items_center()
                        .justify_center()
                        .gap_1()
                        .child(
                            div().text_size(px(28.0)).child("🖼")
                        )
                        .child(
                            div().text_size(px(11.0))
                                .text_color(rgb(theme::CLR_MUTED))
                                .child(desc)
                        )
                )
                // 如果有附带文字且不是默认的"分享图片"，则显示
                .when(!msg.text.is_empty() && msg.text != "分享图片", |d| {
                    let t = msg.text.clone();
                    d.child(
                        div().text_size(px(13.0)).text_color(fg).child(t)
                    )
                })
        )
        .into_any_element()
}
```

**关键改动**:
- 缩小图片占位尺寸 (180x100)，避免超出 `max_w(360px)` 气泡宽度
- 添加 `border_1()` 边框使占位区域清晰可见
- 显示图片数量信息 `[图片 x1]`
- 使用 `.when()` 条件渲染文字，避免不必要的空 div

---

## 修复四: 滚轮无法触发加载更多消息

### 4.1 问题分析

当前"加载更早消息"功能仅有一个手动点击按钮 (`load-more-btn`)，没有滚轮滚动到顶部自动加载的逻辑。用户需要不断点击按钮，体验差。

**根因**: `msg_list_state` 没有设置 `set_scroll_handler`，无法检测滚动到顶部的事件。

### 4.2 修复方案

#### 4.2.1 在 `chat_screen.rs` 的消息列表上设置 scroll handler

**文件**: `src/view/screens/chat_screen.rs`

在 `message_panel` 函数中，创建 list 之前，给 `msg_list_state` 设置 scroll handler:

**在第 306 行 `.child(if !msgs.is_empty() {` 之前插入**:

```rust
// 设置滚动监听: 滚到顶部时自动加载更多
if has_more {
    if let Some(ref lst) = msg_list_state {
        let uid_scroll = uid.clone();
        let muid_scroll = my_uid.clone();
        let mid_scroll = oldest_mid.clone().unwrap_or_default();
        lst.set_scroll_handler(cx.listener(
            move |this, event: &ListScrollEvent, _window, cx| {
                // 当可见区域的起始索引 <= 2 时 (接近顶部), 自动加载更早消息
                if event.visible_range.start <= 2 {
                    if let Some(chat) = this.chat_data.as_ref() {
                        if chat.has_more && !chat.messages_loading {
                            if let Some(chat_mut) = this.chat_data.as_mut() {
                                chat_mut.messages_loading = true;
                            }
                            chat_vm::load_more_messages(
                                cx,
                                &this.tokio_handle,
                                uid_scroll.clone(),
                                muid_scroll.clone(),
                                is_group,
                                mid_scroll.clone(),
                            );
                        }
                    }
                }
            }
        ));
    }
}
```

#### 4.2.2 使用 `messages_loading` 防止重复触发

`ChatData` 中已有 `messages_loading: bool` 字段。需确保在加载完成后重置:

**文件**: `src/viewmodel/chat_vm.rs` `load_more_messages` 函数 (第 130-149 行)

**在 `this.update(...)` 闭包内，添加重置**:

**当前** (约第 131 行):
```rust
this.update(&mut cx, |v, cx| {
    if let Some(chat) = v.chat_data.as_mut() {
        let count = older.len();
        if count > 0 {
            ...
        } else {
            chat.has_more = false;
        }
    }
    cx.notify();
}).ok();
```

**改为**:
```rust
this.update(&mut cx, |v, cx| {
    if let Some(chat) = v.chat_data.as_mut() {
        chat.messages_loading = false;  // ← 新增: 重置加载状态
        let count = older.len();
        if count > 0 {
            chat.oldest_mid = older.first().map(|m| m.id.clone());
            chat.has_more = count >= 30;
            let mut all = older;
            all.append(&mut chat.messages);
            chat.messages = all;
            chat.msg_list_state = Some(rebuild_msg_list_state(&chat.messages, ListAlignment::Top));
            log_info!("[chat_vm] 加载更早 {} 条消息, 总计 {} 条, has_more={}", count, chat.messages.len(), chat.has_more);
        } else {
            chat.has_more = false;
            log_info!("[chat_vm] 没有更早的消息了");
        }
    }
    cx.notify();
}).ok();
```

#### 4.2.3 scroll handler 中的 `oldest_mid` 需要动态获取

**问题**: 上面 4.2.1 中 `mid_scroll` 在闭包创建时就被捕获了，但 `oldest_mid` 在加载更多消息后会更新。闭包中使用的是旧值。

**修复**: 在 scroll handler 内部从 `this.chat_data` 实时读取 `oldest_mid`:

```rust
lst.set_scroll_handler(cx.listener(
    move |this, event: &ListScrollEvent, _window, cx| {
        if event.visible_range.start <= 2 {
            if let Some(chat) = this.chat_data.as_ref() {
                if chat.has_more && !chat.messages_loading {
                    if let Some(mid) = chat.oldest_mid.clone() {
                        let uid_s = chat.selected_uid.clone().unwrap_or_default();
                        let muid_s = chat.my_uid.clone();
                        let is_group_s = is_group;
                        // 标记加载中
                        if let Some(chat_mut) = this.chat_data.as_mut() {
                            chat_mut.messages_loading = true;
                        }
                        chat_vm::load_more_messages(
                            cx, &this.tokio_handle,
                            uid_s, muid_s, is_group_s, mid,
                        );
                    }
                }
            }
        }
    }
));
```

> **注意**: `cx.listener` 闭包的第一个参数 `this` 是 `&mut AppRoot`，可以直接访问最新的 `chat_data`。不需要提前捕获 `uid`/`oldest_mid`。

#### 4.2.4 保留手动按钮作为备选

保留现有的 "▲ 加载更早消息" 按钮不变，作为滚轮触发失败时的手动备选。但可以将文案改为更友好的提示:

```rust
.child("▲ 滚动到顶部自动加载 / 点击加载更早消息")
```

---

## 附加修复: 防止 review.md 中的 P0 panic

以下是 `review.md` 中标记为 P0 的 panic 问题，应与上述 4 个修复一并处理:

### A. 中文字符串切片 panic

**文件**: `src/model/chat_service.rs` 第 375 行和第 438 行

**当前**:
```rust
&text[..text.len().min(20)]
```

**改为**:
```rust
&text.chars().take(20).collect::<String>()
```

**或者使用辅助函数** (在 `chat_service.rs` 底部添加):
```rust
/// 安全截取字符串前 N 个字符 (UTF-8 安全)
fn truncate_chars(s: &str, max_chars: usize) -> String {
    s.chars().take(max_chars).collect()
}
```

然后替换:
```rust
log_info!("[chat] 发送消息: uid={}, text={}...", uid, truncate_chars(text, 20));
log_info!("[chat] Group send: gid={}, text={}...", gid, truncate_chars(text, 20));
```

### B. tokio::spawn 在非 runtime 上下文中的 panic

**文件**: `src/view/screens/chat_screen.rs` 第 381 行

**当前**:
```rust
tokio::spawn(async move { ... });
```

**改为** (使用 `cx.spawn` + `handle.block_on` 正确模式):
```rust
// 在 on_click listener 中, 将加载表情改为 cx.spawn 模式
let handle = this.tokio_handle.clone();
cx.spawn(|this_weak: WeakEntity<AppRoot>, cx: &mut AsyncApp| {
    let mut cx = cx.clone();
    async move {
        let emotions = handle.block_on(
            crate::model::chat_service::fetch_emotions()
        );
        this_weak.update(&mut cx, |v, cx| {
            if let Some(chat) = v.chat_data.as_mut() {
                chat.emotions = emotions;
            }
            cx.notify();
        }).ok();
    }
}).detach();
```

**文件**: `src/viewmodel/chat_vm.rs` 第 108 行和第 115 行

**当前**:
```rust
tokio::spawn(async move { ... });
```

**改为**:
```rust
handle.spawn(async move { ... });
```

---

## 修改文件汇总

| 文件 | 修复内容 |
|------|---------|
| `src/view/widgets/contact_card.rs` | 修复一: 未读角标固定最小尺寸 + 居中 |
| `src/viewmodel/chat_vm.rs` | 修复二: 新增 `compute_list_item_count` + `rebuild_msg_list_state`，修改 `select_contact`/`load_more_messages`/`send_message` |
| `src/viewmodel/root_vm.rs` | 修复二: `handle_ws_message` 使用 `rebuild_msg_list_state` |
| `src/view/screens/chat_screen.rs` | 修复二(防御校验) + 修复四(scroll handler) + 附加B(tokio::spawn) |
| `src/model/chat_service.rs` | 修复三(fids 解析) + 附加A(中文切片) |
| `src/view/widgets/message_bubble.rs` | 修复三: 优化图片占位渲染 |

---

## 验证方法

1. **角标修复**: 打开聊天界面，确认未读数为 1-3 位数时角标保持圆形/短胶囊，不再被拉长
2. **消息覆盖**: 进入有大量消息的群聊，确认消息之间不再重叠，滚动流畅
3. **图片渲染**: 进入有图片消息的聊天，确认显示 "🖼 [图片 x1]" 占位，不再是空白块
4. **滚轮加载**: 在消息列表中向上滚动到顶部，确认自动触发加载更早消息（观察日志 `[chat_vm] 加载更早`）
