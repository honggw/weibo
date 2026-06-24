# 消息覆盖问题根因分析

> 通过阅读 GPUI `list.rs` 1180 行源码得出（gpui-0.2.2/src/elements/list.rs）

---

## GPUI List 组件内部机制

### 数据结构

GPUI `List` 使用 `SumTree<ListItem>` 记录每个 item 的状态：

```rust
enum ListItem {
    Unmeasured { focus_handle: Option<FocusHandle> },  // 高度 = px(0.)
    Measured { size: Size<Pixels>, focus_handle: ... }, // 高度 = 实际测量值
}
```

关键：**`Unmeasured` 的 item 高度为 0**（list.rs 第 1117-1122 行）。

### ListState::new() 行为

调用 `ListState::new(item_count, alignment, overdraw)` 时：
1. 创建空的 `SumTree`
2. 调用 `splice(0..0, item_count)` 插入 `item_count` 个 `Unmeasured` item
3. 所有 item 高度均为 0

### 绘制流程 (prepaint)

每次 prepaint 时（list.rs 第 1008-1057 行）：

1. **宽度变化检查**（第 1026-1038 行）：如果 `last_layout_bounds` 为 `None`（新 ListState 必然如此）或宽度变化，**强制所有 item 回到 Unmeasured**：
   ```rust
   if state.last_layout_bounds.is_none_or(|last_bounds| last_bounds.size.width != bounds.size.width) {
       state.items = SumTree::from_iter(
           state.items.iter().map(|item| ListItem::Unmeasured { ... }),
           (),
       );
   }
   ```

2. **layout_items**（第 604-779 行）：从 `scroll_top` 开始，只测量 **可视区域 + overdraw** 范围内的 item。其余 item 保持 `Unmeasured`（高度=0）。

3. **paint**（第 1059-1095 行）：
   ```rust
   let mut item_origin = bounds.origin + Point::new(px(0.), padding.top);
   item_origin.y -= layout_response.scroll_top.offset_in_item;
   for item in &mut layout_response.item_layouts {
       window.with_content_mask(Some(ContentMask { bounds }), |window| {
           item.element.prepaint_at(item_origin, window, cx);
       });
       item_origin.y += item.size.height;  // 下一个 item 起始位置
   }
   ```
   注意：`content_mask` 作用于**整个列表的 bounds**，而非单个 item。

---

## 问题 1：频繁重建 ListState 导致高度缓存丢失

### 原始代码

```rust
pub fn rebuild_msg_list_state(chat: &mut ChatData, alignment: ListAlignment) {
    let item_count = count_list_items(&chat.messages);
    chat.msg_list_state = Some(ListState::new(item_count, alignment, px(400.0)));
}
```

每次发送消息、收到 WS 推送、加载历史消息时都调用此函数。

### 问题链

1. `ListState::new()` → 所有 item 变为 `Unmeasured`（高度=0）
2. 第一帧 prepaint：`last_layout_bounds` 为 `None` → 再次强制所有 item 为 `Unmeasured`
3. `layout_items` 从 `scroll_top` 开始测量可见区域的 item
4. 由于 `ListAlignment::Bottom`，scroll_top 初始值为 `item_ix = item_count`（最底部）
5. GPUI 向上遍历 item 来填充可见区域时，未测量的 item 高度为 0
6. 导致 `scroll_top` 计算偏差 → 部分 item 的 y 坐标基于错误的高度累积
7. 结果：item 之间出现视觉重叠

### 修复

区分"全量替换"和"增量更新"两种场景：
- **切换会话**：消息列表完全替换，旧缓存无意义 → 使用 `rebuild_msg_list_state()` 创建新 ListState
- **追加消息**（发送/WS）：使用 `splice(old_count..old_count, delta)` 在尾部插入新 item，保留已测量 item 的高度
- **加载历史**（prepend）：使用 `splice(0..0, prepended_count)` 在头部插入新 item，保留尾部已测量 item 的高度

---

## 问题 2：单个 item 无裁剪边界

### GPUI List 的 content_mask

List 的 paint 方法设置 `content_mask` 的范围是**整个 List 组件的 bounds**：
```rust
window.with_content_mask(Some(ContentMask { bounds }), |window| {
    for item in &mut prepaint.layout.item_layouts {
        item.element.paint(window, cx);
    }
});
```

这意味着：只要 item 的渲染内容在 List 总 bounds 内，就不会被裁剪。如果一个 item 的视觉元素（如 `rounded_lg` 圆角、文字行高、阴影等）超出其自身的 `size.height`，这些超出部分会覆盖到相邻 item 的区域。

### message_bubble 的布局

```rust
// render_normal 最外层
div()
    .flex().flex_row().w_full().px_3().py_1().gap_2()  // py_1 = 仅 4px padding
    .child(avatar)  // 36x36
    .child(msg_body) // flex_col, 无 overflow 限制
```

- 消息气泡有 `rounded_lg`（8px 圆角）
- 气泡有 `px_3().py_2()` 内边距
- 但外层只有 `py_1`（4px）上下间距
- 两条相邻消息之间有效间距仅 8px

当文本渲染或圆角绘制略微超出测量边界时，内容就会溢出到下一条消息的"发送人名称"区域。

### 修复

在 list render callback 中为每个 item 包一层 `div().overflow_hidden()`：
```rust
ListItem::Message(msg) => {
    div()
        .overflow_hidden()
        .child(crate::view::widgets::message_bubble::render(msg))
        .into_any_element()
}
```

这确保每个 item 的渲染严格限制在其测量高度内，即使内部元素有溢出也会被裁剪。

---

## 修复文件清单

| 文件 | 改动 | 解决的问题 |
|------|------|-----------|
| `src/view/screens/chat_screen.rs` | 每个 list item 包 `div().overflow_hidden()` | 防止单个 item 溢出覆盖相邻 item |
| `src/viewmodel/chat_vm.rs` | 新增 `update_msg_list_state_append()` | 追加消息时保留高度缓存 |
| `src/viewmodel/chat_vm.rs` | 新增 `update_msg_list_state_prepend()` | 加载历史时保留高度缓存 |
| `src/viewmodel/chat_vm.rs` send_message | 改用 `update_msg_list_state_append()` | 避免全量重建 |
| `src/viewmodel/chat_vm.rs` load_more | 改用 `update_msg_list_state_prepend()` | 避免全量重建 |
| `src/viewmodel/root_vm.rs` WS handler | 改用 `update_msg_list_state_append()` | 避免全量重建 |

---

## splice() API 行为说明

```rust
/// 通知 ListState：old_range 范围内的 item 已被 count 个新 item 替换。
/// 新 item 初始化为 Unmeasured，但 old_range 之外的 item 保留 Measured 状态。
pub fn splice(&self, old_range: Range<usize>, count: usize)
```

- `splice(old_count..old_count, delta)` — 在尾部追加 delta 个新 item
- `splice(0..0, count)` — 在头部插入 count 个新 item（已有 item 高度保留）
- `splice(0..old_count, new_count)` — 全量替换（等价于 reset，但不设 reset flag）
