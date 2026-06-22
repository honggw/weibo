//! Chat tab screen — contact list + message panel + text input.

use gpui::*;

use crate::domain::{ChatMessage, Contact};
use crate::view::theme;
use crate::viewmodel::chat_vm::{self, ChatData};
use crate::viewmodel::root_vm::AppRoot;

pub fn render(
    chat: Option<&ChatData>, contact_list_state: &ListState,
    _msg_list_state: &ListState, cx: &mut Context<AppRoot>,
) -> AnyElement {
    let contacts = chat.map(|c| c.contacts.clone()).unwrap_or_default();
    let loading = chat.map(|c| c.loading).unwrap_or(true);
    let count = contacts.len();
    if loading && contacts.is_empty() {
        return div().flex().flex_col().size_full().items_center().justify_center().gap_3()
            .child(div().text_size(px(32.0)).child("💬"))
            .child(div().text_size(px(14.0)).text_color(rgb(theme::CLR_MUTED)).child("加载会话列表...")).into_any_element();
    }

    let sel_uid = chat.and_then(|c| c.selected_uid.clone());
    let messages = chat.map(|c| c.messages.clone()).unwrap_or_default();
    let sel_name = sel_uid.as_ref().and_then(|u| contacts.iter().find(|c| &c.user_id == u).map(|c| c.screen_name.clone()));
    let has_more = chat.map(|c| c.has_more).unwrap_or(false);
    let is_group = sel_uid.as_ref().and_then(|u| contacts.iter().find(|c| &c.user_id == u).map(|c| c.is_group)).unwrap_or(false);
    let oldest_mid = chat.and_then(|c| c.oldest_mid.clone());
    let my_uid = chat.map(|c| c.my_uid.clone()).unwrap_or_default();
    let draft = chat.map(|c| c.draft_text.clone()).unwrap_or_default();
    let msg_list_state = chat.and_then(|c| c.msg_list_state.clone());

    div().flex().flex_row().size_full()
        .child(contact_list(&contacts, count, &sel_uid, contact_list_state, cx))
        .child(message_panel(&sel_uid, &sel_name, &messages, has_more, oldest_mid, is_group, my_uid, &draft, msg_list_state.as_ref(), cx))
        .into_any_element()
}

fn contact_list(
    contacts: &[Contact], count: usize, sel_uid: &Option<String>,
    list_state: &ListState, cx: &mut Context<AppRoot>,
) -> impl IntoElement {
    let c_owned: Vec<Contact> = contacts.to_vec();
    let sel = sel_uid.clone();
    let entity = cx.entity();

    div().flex().flex_col().w(px(220.0)).h_full().bg(rgb(0x0d1b36))
        .border_r_1().border_color(rgb(0x1a2a4a))
        .child(div().px_3().py_2().border_b_1().border_color(rgb(0x1a2a4a))
            .child(div().text_size(px(13.0)).text_color(rgb(theme::CLR_MUTED)).child(format!("会话 ({})", count))))
        .child(list(list_state.clone(), move |ix, _window, _cx| {
            if ix >= count { return div().into_any_element(); }
            let c = &c_owned[ix];
            let is_sel = sel.as_ref() == Some(&c.user_id);
            let uid = c.user_id.clone(); let entity = entity.clone();
            let u2 = uid.clone(); let is_group = c.is_group;
            div().id(("contact", ix)).bg(if is_sel { rgb(0x1a2a4a) } else { rgb(0x00000000) }).cursor_pointer()
                .child(crate::view::widgets::contact_card::render(c))
                .on_click(move |_: &ClickEvent, _: &mut Window, cx: &mut App| {
                    cx.update_entity(&entity, |v: &mut AppRoot, cx| {
                        let muid = v.chat_data.as_ref().map(|c| c.my_uid.clone()).unwrap_or_default();
                        if let Some(chat) = v.chat_data.as_mut() { chat.selected_uid = Some(u2.clone()); chat.messages_loading = true; }
                        cx.notify();
                        chat_vm::select_contact(cx, &v.tokio_handle, u2.clone(), muid, is_group);
                    });
                }).into_any_element()
        }).flex_1())
}

fn message_panel(
    sel_uid: &Option<String>, sel_name: &Option<String>, msgs: &[ChatMessage],
    has_more: bool, oldest_mid: Option<String>, is_group: bool, my_uid: String,
    draft: &str, msg_list_state: Option<&ListState>, cx: &mut Context<AppRoot>,
) -> impl IntoElement {
    if sel_uid.is_none() {
        return div().flex().flex_col().flex_1().h_full().items_center().justify_center().gap_4()
            .child(div().text_size(px(48.0)).child("💬"))
            .child(div().text_size(px(14.0)).text_color(rgb(theme::CLR_MUTED)).child("选择一个会话开始聊天"));
    }
    let name = sel_name.clone().unwrap_or_default();
    let msgs_v: Vec<ChatMessage> = msgs.to_vec();
    let n = msgs_v.len();
    let uid = sel_uid.clone().unwrap_or_default();
    let show_more = has_more && oldest_mid.is_some();

    div().flex().flex_col().flex_1().h_full()
        // Header with contact name
        .child(div().px_3().py_2().bg(rgb(0x0d1b36)).border_b_1().border_color(rgb(0x1a2a4a))
            .child(div().text_size(px(14.0)).font_weight(FontWeight::BOLD).text_color(rgb(theme::CLR_TEXT)).child(format!("💬 {}", name))))
        // Load-more button (fixed above scroll area)
        .child(if show_more {
            let u2 = uid.clone(); let muid = my_uid.clone(); let mid = oldest_mid.clone().unwrap_or_default();
            div().flex().flex_row().justify_center().py_1().bg(rgb(0x0a1a30))
                .child(div().id("load-more-btn").cursor_pointer().px_3().py_1().rounded_full()
                    .bg(rgb(0x1a2a4a)).text_size(px(12.0)).text_color(rgb(theme::CLR_MUTED))
                    .child("▲ 加载更早消息")
                    .on_click(cx.listener(move |this, _: &ClickEvent, _window, cx| {
                        chat_vm::load_more_messages(cx, &this.tokio_handle, u2.clone(), muid.clone(), is_group, mid.clone());
                    }))).into_any_element()
        } else { div().into_any_element() })
        // Scrollable message area (virtual list, fills remaining space)
        .child(if n > 0 {
            if let Some(ref lst) = msg_list_state {
                list((*lst).clone(), move |ix, _window, _cx| {
                    if ix < msgs_v.len() {
                        crate::view::widgets::message_bubble::render(&msgs_v[ix]).into_any_element()
                    } else {
                        div().into_any_element()
                    }
                }).flex_1().into_any_element()
            } else {
                div().flex_1().into_any_element()
            }
        } else {
            div().flex().flex_col().flex_1().items_center().justify_center().gap_3()
                .child(div().text_size(px(32.0)).child("📭"))
                .child(div().text_size(px(14.0)).text_color(rgb(theme::CLR_MUTED)).child("暂无消息")).into_any_element()
        })
        // Fixed input bar at bottom (not in scroll area)
        .child(input_bar(&uid, is_group, draft, cx))
}

fn input_bar(uid: &str, is_group: bool, draft: &str, cx: &mut Context<AppRoot>) -> impl IntoElement {
    let u1 = uid.to_string();
    let u2 = uid.to_string();
    let d = draft.to_string();

    div().flex().flex_row().items_center().gap_2()
        .px_3().py_2().bg(rgb(0x0d1b36)).border_t_1().border_color(rgb(0x1a2a4a))
        .child(
            div().id("msg-input").flex_1()
                .px_3().py_2().rounded_lg().bg(rgb(0x1a2a4a))
                .text_size(px(13.0)).text_color(rgb(theme::CLR_TEXT))
                .focusable()
                .on_key_down(cx.listener(move |this, ev: &KeyDownEvent, _window, cx| {
                    let Some(chat) = this.chat_data.as_mut() else { return };
                    let k = ev.keystroke.key.as_str();
                    let ch = ev.keystroke.key_char.as_deref().unwrap_or("");
                    crate::log_info!("[input] key={}, key_char={}", k, ch);
                    match k {
                        "enter" | "return" => {
                            let text = chat.draft_text.trim().to_string();
                            if !text.is_empty() {
                                chat.draft_text.clear();
                                chat_vm::send_message(cx, &this.tokio_handle, u1.clone(), text, is_group);
                            }
                        }
                        "backspace" => { chat.draft_text.pop(); }
                        "space" => { chat.draft_text.push(' '); }
                        _ => {
                            if !ch.is_empty() {
                                chat.draft_text.push_str(ch);
                            }
                        }
                    }
                    cx.notify();
                }))
                .child(if d.is_empty() {
                    div().text_color(rgb(theme::CLR_MUTED)).child("| 输入消息...").into_any_element()
                } else {
                    div().text_color(rgb(theme::CLR_TEXT)).child(format!("{}|", d)).into_any_element()
                }),
        )
        .child(
            div().id("send-msg-btn").px_4().py_2().rounded_lg()
                .bg(rgb(theme::CLR_ACCENT)).cursor_pointer()
                .text_size(px(13.0)).text_color(rgb(0xffffff))
                .child("发送")
                .on_click(cx.listener(move |this, _: &ClickEvent, _window, cx| {
                    let Some(chat) = this.chat_data.as_mut() else { return };
                    let text = chat.draft_text.trim().to_string();
                    if !text.is_empty() {
                        chat.draft_text.clear();
                        chat_vm::send_message(cx, &this.tokio_handle, u2.clone(), text, is_group);
                        cx.notify();
                    }
                })),
        )
}
