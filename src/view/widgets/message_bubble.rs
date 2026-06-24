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

    // 头像: 首字占位圆 (后续可通过 sender_avatar 加载真实头像)
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
        .child(avatar_char)
        .into_any_element();

    // 气泡内容
    let bubble_content = match &msg.media_type {
        MediaType::Image => render_image_bubble(msg, bubble_color, text_color),
        MediaType::Quote => render_quote_bubble(msg, bubble_color, text_color),
        _               => render_text_bubble(msg, bubble_color, text_color),
    };

    // 角色标签
    let role_badge = match msg.role {
        4 => Some(("群主", 0xe8633a)),
        1 => Some(("管理员", 0x4a9eff)),
        _ => None,
    };

    // 发送者名称 + 角色 + 时间
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
fn render_text_bubble(msg: &ChatMessage, bg: Rgba, fg: Rgba) -> AnyElement {
    div()
        .px_3().py_2().rounded_lg()
        .bg(bg).text_size(px(13.0)).text_color(fg)
        .child(msg.text.clone())
        .into_any_element()
}

/// 图片消息气泡 — 显示占位符及 fid 数量信息。
/// 注: GPUI 0.2 不支持 ImageSource::Uri, 缩略图需手动下载后用 ImageSource::Image 渲染。
fn render_image_bubble(msg: &ChatMessage, _bg: Rgba, fg: Rgba) -> AnyElement {
    let fid_count = msg.fids.len();
    let thumb_info = if fid_count > 0 {
        let first_fid = &msg.fids[0];
        format!("{} 张图片\nfid: {}...", fid_count, &first_fid[..first_fid.len().min(12)])
    } else {
        "图片".to_string()
    };

    div()
        .px_3().py_2().rounded_lg().bg(_bg)
        .child(
            div().flex().flex_col().gap_1()
                .child(
                    div()
                        .min_w(px(120.0)).min_h(px(80.0)).rounded_md()
                        .bg(rgb(0x2a3a5a))
                        .flex().flex_col().items_center().justify_center().gap_1()
                        .px_3().py_2()
                        .child(
                            div()
                                .text_size(px(28.0)).text_color(rgb(theme::CLR_MUTED))
                                .child("🖼")
                        )
                        .child(
                            div()
                                .text_size(px(11.0)).text_color(rgb(theme::CLR_MUTED))
                                .child(thumb_info)
                        )
                )
                .child(if !msg.text.is_empty() && msg.text != "分享图片" {
                    div().text_size(px(13.0)).text_color(fg).child(msg.text.clone()).into_any_element()
                } else {
                    div().into_any_element()
                })
        )
        .into_any_element()
}

/// 引用消息气泡 — 解析 content 中「...」引用块
fn render_quote_bubble(msg: &ChatMessage, _bg: Rgba, fg: Rgba) -> AnyElement {
    // 微博引用格式: 「引用文本」\n- - - - -\n回复文本
    let parts: Vec<&str> = msg.text.splitn(2, "\n- - - - - - - - - - - - - - -\n").collect();

    let (quote_text, reply_text) = if parts.len() == 2 {
        (parts[0].trim_matches('「').trim_matches('」').to_string(), parts[1].to_string())
    } else {
        (String::new(), msg.text.clone())
    };

    div()
        .px_3().py_2().rounded_lg().bg(_bg)
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
