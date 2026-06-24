//! Chat tab screen — contact list + message panel + text input.

use gpui::*;

use crate::domain::{ChatMessage, Contact, GroupInfo};
use crate::view::theme;
use crate::viewmodel::chat_vm::{self, ChatData};
use crate::viewmodel::root_vm::AppRoot;

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
        if i == 0
            || (msg.timestamp > 0
                && msgs[i - 1].timestamp > 0
                && msg.timestamp.saturating_sub(msgs[i - 1].timestamp) > 300)
        {
            if msg.timestamp > 0 {
                items.push(ListItem::TimeSeparator(msg.created_at.clone()));
            }
        }
        items.push(ListItem::Message(msg.clone()));
    }
    items
}

pub fn render(
    chat: Option<&ChatData>, contact_list_state: &ListState,
    _msg_list_state: &ListState, cx: &mut Context<AppRoot>,
) -> AnyElement {
    let contacts = chat.map(|c| c.contacts.clone()).unwrap_or_default();
    let filtered: Vec<Contact> = chat
        .map(|c| {
            if c.search_text.is_empty() {
                c.contacts.clone()
            } else {
                let st = c.search_text.clone();
                c.contacts
                    .iter()
                    .filter(|ct| ct.screen_name.contains(&st))
                    .cloned()
                    .collect::<Vec<_>>()
            }
        })
        .unwrap_or_default();
    let loading = chat.map(|c| c.loading).unwrap_or(true);
    let count = filtered.len();
    let full_count = contacts.len();
    // When search filtering is active, use a ListState with the correct filtered count
    // to prevent blank space in the list.
    let effective_contact_list = if count != full_count {
        ListState::new(count, ListAlignment::Top, px(60.0))
    } else {
        contact_list_state.clone()
    };
    if loading && contacts.is_empty() {
        return div()
            .flex().flex_col().size_full().items_center().justify_center().gap_3()
            .child(div().text_size(px(32.0)).child("💬"))
            .child(
                div()
                    .text_size(px(14.0))
                    .text_color(rgb(theme::CLR_MUTED))
                    .child("加载会话列表..."),
            )
            .into_any_element();
    }

    let sel_uid = chat.and_then(|c| c.selected_uid.clone());
    let messages = chat.map(|c| c.messages.clone()).unwrap_or_default();
    let sel_name = sel_uid.as_ref().and_then(|u| {
        contacts
            .iter()
            .find(|c| &c.user_id == u)
            .map(|c| c.screen_name.clone())
    });
    let has_more = chat.map(|c| c.has_more).unwrap_or(false);
    let is_group = sel_uid.as_ref().and_then(|u| {
        contacts
            .iter()
            .find(|c| &c.user_id == u)
            .map(|c| c.is_group)
    }).unwrap_or(false);
    let oldest_mid = chat.and_then(|c| c.oldest_mid.clone());
    let my_uid = chat.map(|c| c.my_uid.clone()).unwrap_or_default();
    let draft = chat.map(|c| c.draft_text.clone()).unwrap_or_default();
    let msg_list_state = chat.and_then(|c| c.msg_list_state.clone());
    let show_emoji_panel = chat.map(|c| c.show_emoji_panel).unwrap_or(false);
    let emotions = chat.map(|c| c.emotions.clone()).unwrap_or_default();
    let search_text = chat.map(|c| c.search_text.clone()).unwrap_or_default();
    let group_info = chat.and_then(|c| c.group_info.clone());

    div()
        .flex().flex_row().size_full()
        .child(contact_list(
            &filtered,
            count,
            &sel_uid,
            &effective_contact_list,
            &search_text,
            cx,
        ))
        .child(message_panel(
            &sel_uid,
            &sel_name,
            &messages,
            has_more,
            oldest_mid,
            is_group,
            my_uid,
            &draft,
            msg_list_state,
            show_emoji_panel,
            &emotions,
            group_info.as_ref(),
            cx,
        ))
        .into_any_element()
}

fn contact_list(
    contacts: &[Contact], count: usize, sel_uid: &Option<String>,
    list_state: &ListState, search_text: &str, cx: &mut Context<AppRoot>,
) -> impl IntoElement {
    let c_owned: Vec<Contact> = contacts.to_vec();
    let sel = sel_uid.clone();
    let entity = cx.entity();
    let search = search_text.to_string();

    div()
        .flex().flex_col().w(px(220.0)).h_full().bg(rgb(0x0d1b36))
        .border_r_1().border_color(rgb(0x1a2a4a))
        // 标题
        .child(
            div().px_3().py_2().border_b_1().border_color(rgb(0x1a2a4a))
                .child(
                    div()
                        .text_size(px(13.0))
                        .text_color(rgb(theme::CLR_MUTED))
                        .child(format!("会话 ({})", count)),
                ),
        )
        // 搜索框
        .child(
            div().px_2().py_1().border_b_1().border_color(rgb(0x1a2a4a))
                .child(
                    div()
                        .id("search-input")
                        .w_full().px_2().py_1()
                        .rounded_md().bg(rgb(0x1a2a4a))
                        .text_size(px(12.0))
                        .text_color(rgb(theme::CLR_TEXT))
                        .focusable()
                        .on_key_down(cx.listener(
                            move |this, ev: &KeyDownEvent, _window, cx| {
                                if let Some(chat) = this.chat_data.as_mut() {
                                    let ch = ev.keystroke.key_char.as_deref().unwrap_or("");
                                    match ev.keystroke.key.as_str() {
                                        "backspace" => {
                                            chat.search_text.pop();
                                        }
                                        _ if !ch.is_empty() => {
                                            chat.search_text.push_str(ch);
                                        }
                                        _ => {}
                                    }
                                }
                                cx.notify();
                            },
                        ))
                        .child(if search.is_empty() {
                            div()
                                .text_color(rgb(theme::CLR_MUTED))
                                .child("🔍 搜索")
                                .into_any_element()
                        } else {
                            div().child(search.clone()).into_any_element()
                        }),
                ),
        )
        // 联系人列表
        .child(
            list(list_state.clone(), move |ix, _window, _cx| {
                if ix >= count {
                    return div().into_any_element();
                }
                let c = &c_owned[ix];
                let is_sel = sel.as_ref() == Some(&c.user_id);
                let uid = c.user_id.clone();
                let entity = entity.clone();
                let u2 = uid.clone();
                let is_group = c.is_group;
                div()
                    .id(("contact", ix))
                    .bg(if is_sel {
                        rgb(0x1a2a4a)
                    } else {
                        rgb(0x00000000)
                    })
                    .cursor_pointer()
                    .child(crate::view::widgets::contact_card::render(c))
                    .on_click(move |_: &ClickEvent, _: &mut Window, cx: &mut App| {
                        cx.update_entity(&entity, |v: &mut AppRoot, cx| {
                            let muid = v
                                .chat_data
                                .as_ref()
                                .map(|c| c.my_uid.clone())
                                .unwrap_or_default();
                            if let Some(chat) = v.chat_data.as_mut() {
                                chat.selected_uid = Some(u2.clone());
                                chat.messages_loading = true;
                            }
                            cx.notify();
                            chat_vm::select_contact(
                                cx,
                                &v.tokio_handle,
                                u2.clone(),
                                muid,
                                is_group,
                            );
                        });
                    })
                    .into_any_element()
            })
            .flex_1(),
        )
}

fn message_panel(
    sel_uid: &Option<String>, sel_name: &Option<String>, msgs: &[ChatMessage],
    has_more: bool, oldest_mid: Option<String>, is_group: bool, my_uid: String,
    draft: &str, msg_list_state: Option<ListState>,
    show_emoji_panel: bool, emotions: &[crate::domain::Emotion],
    group_info: Option<&GroupInfo>,
    cx: &mut Context<AppRoot>,
) -> impl IntoElement {
    if sel_uid.is_none() {
        return div()
            .flex().flex_col().flex_1().h_full().items_center().justify_center().gap_4()
            .child(div().text_size(px(48.0)).child("💬"))
            .child(
                div()
                    .text_size(px(14.0))
                    .text_color(rgb(theme::CLR_MUTED))
                    .child("选择一个会话开始聊天"),
            );
    }
    let name = sel_name.clone().unwrap_or_default();
    let uid = sel_uid.clone().unwrap_or_default();
    let show_more = has_more && oldest_mid.is_some();

    // Build list items with time separators
    let list_items = build_list_items(msgs);

    // Main content area
    let main_content = div()
        .flex().flex_col().flex_1().h_full()
        // Header
        .child(
            div().px_3().py_2().bg(rgb(0x0d1b36)).border_b_1().border_color(rgb(0x1a2a4a))
                .child(
                    div()
                        .text_size(px(14.0))
                        .font_weight(FontWeight::BOLD)
                        .text_color(rgb(theme::CLR_TEXT))
                        .child(format!("💬 {}", name)),
                ),
        )
        // Load-more button
        .child(if show_more {
            let u2 = uid.clone();
            let muid = my_uid.clone();
            let mid = oldest_mid.clone().unwrap_or_default();
            div()
                .flex().flex_row().justify_center().py_1().bg(rgb(0x0a1a30))
                .child(
                    div()
                        .id("load-more-btn")
                        .cursor_pointer()
                        .px_3().py_1().rounded_full()
                        .bg(rgb(0x1a2a4a))
                        .text_size(px(12.0))
                        .text_color(rgb(theme::CLR_MUTED))
                        .child("▲ 加载更早消息")
                        .on_click(cx.listener(
                            move |this, _: &ClickEvent, _window, cx| {
                                chat_vm::load_more_messages(
                                    cx,
                                    &this.tokio_handle,
                                    u2.clone(),
                                    muid.clone(),
                                    is_group,
                                    mid.clone(),
                                );
                            },
                        )),
                )
                .into_any_element()
        } else {
            div().into_any_element()
        })
        // Message list
        .child(if !msgs.is_empty() {
            if let Some(lst) = msg_list_state {
                // Set scroll handler to load more messages when scrolled to top
                if show_more {
                    let scroll_uid = uid.clone();
                    let scroll_muid = my_uid.clone();
                    let scroll_mid = oldest_mid.clone().unwrap_or_default();
                    lst.set_scroll_handler(cx.listener(
                        move |this, event: &ListScrollEvent, _window, cx| {
                            if event.visible_range.start == 0 {
                                // Prevent duplicate loads
                                let already_loading = this.chat_data.as_ref()
                                    .map(|c| c.messages_loading)
                                    .unwrap_or(false);
                                if !already_loading {
                                    if let Some(chat) = this.chat_data.as_mut() {
                                        chat.messages_loading = true;
                                    }
                                    chat_vm::load_more_messages(
                                        cx,
                                        &this.tokio_handle,
                                        scroll_uid.clone(),
                                        scroll_muid.clone(),
                                        is_group,
                                        scroll_mid.clone(),
                                    );
                                }
                            }
                        },
                    ));
                }
                let items_for_list = list_items.clone();
                list(lst, move |ix, _window, _cx| {
                    if ix >= items_for_list.len() {
                        return div().into_any_element();
                    }
                    match &items_for_list[ix] {
                        ListItem::TimeSeparator(time_text) => div()
                            .flex().flex_row().justify_center().py_2()
                            .child(
                                div()
                                    .px_3().py_1().rounded_full()
                                    .bg(rgb(0x1a2a4a))
                                    .text_size(px(11.0))
                                    .text_color(rgb(theme::CLR_MUTED))
                                    .child(time_text.clone()),
                            )
                            .into_any_element(),
                        ListItem::Message(msg) => {
                            crate::view::widgets::message_bubble::render(msg)
                                .into_any_element()
                        }
                    }
                })
                .flex_1()
                .into_any_element()
            } else {
                div().flex_1().into_any_element()
            }
        } else {
            div()
                .flex().flex_col().flex_1().items_center().justify_center().gap_3()
                .child(div().text_size(px(32.0)).child("📭"))
                .child(
                    div()
                        .text_size(px(14.0))
                        .text_color(rgb(theme::CLR_MUTED))
                        .child("暂无消息"),
                )
                .into_any_element()
        })
        // Emoji panel (between message list and input bar)
        .child(if show_emoji_panel {
            crate::view::widgets::emoji_panel::render(emotions, cx).into_any_element()
        } else {
            div().into_any_element()
        })
        // Input bar
        .child(input_bar(&uid, is_group, draft, cx));

    // Wrap main content with member sidebar on the right for group chats
    div()
        .flex().flex_row().flex_1().h_full()
        .child(main_content)
        .child(if is_group {
            if let Some(info) = group_info {
                crate::view::widgets::member_sidebar::render(info).into_any_element()
            } else {
                div().into_any_element()
            }
        } else {
            div().into_any_element()
        })
}

fn input_bar(
    uid: &str, is_group: bool, draft: &str, cx: &mut Context<AppRoot>,
) -> impl IntoElement {
    let u1 = uid.to_string();
    let u2 = uid.to_string();
    let d = draft.to_string();

    div()
        .flex().flex_row().items_center().gap_2()
        .px_3().py_2()
        .bg(rgb(0x0d1b36))
        .border_t_1()
        .border_color(rgb(0x1a2a4a))
        // Emoji toggle button
        .child(
            div()
                .id("emoji-btn")
                .cursor_pointer()
                .px_2().py_2().rounded_lg()
                .text_size(px(18.0))
                .hover(|s| s.bg(rgb(0x1a2a4a)))
                .child("😊")
                .on_click(cx.listener(
                    |this, _: &ClickEvent, _window, cx| {
                        if let Some(chat) = this.chat_data.as_mut() {
                            chat.show_emoji_panel = !chat.show_emoji_panel;
                            // 首次打开时加载表情列表
                            if chat.show_emoji_panel && chat.emotions.is_empty() {
                                let handle = this.tokio_handle.clone();
                                cx.spawn(|this: WeakEntity<AppRoot>, cx: &mut AsyncApp| {
                                    let mut cx = cx.clone();
                                    async move {
                                        let emotions = handle.block_on(
                                            crate::model::chat_service::fetch_emotions()
                                        );
                                        this.update(&mut cx, |v, cx| {
                                            if let Some(chat) = v.chat_data.as_mut() {
                                                chat.emotions = emotions;
                                            }
                                            cx.notify();
                                        }).ok();
                                    }
                                }).detach();
                            }
                        }
                        cx.notify();
                    },
                )),
        )
        // Text input area
        .child(
            div()
                .id("msg-input")
                .flex_1()
                .px_3().py_2().rounded_lg()
                .bg(rgb(0x1a2a4a))
                .text_size(px(13.0))
                .text_color(rgb(theme::CLR_TEXT))
                .focusable()
                .on_key_down(cx.listener(
                    move |this, ev: &KeyDownEvent, _window, cx| {
                        let Some(chat) = this.chat_data.as_mut() else {
                            return;
                        };
                        let k = ev.keystroke.key.as_str();
                        let ch = ev.keystroke.key_char.as_deref().unwrap_or("");
                        match k {
                            "enter" | "return" if ev.keystroke.modifiers.shift => {
                                // Shift+Enter 换行
                                chat.draft_text.push('\n');
                            }
                            "enter" | "return" => {
                                let text = chat.draft_text.trim().to_string();
                                if !text.is_empty() {
                                    chat.draft_text.clear();
                                    chat_vm::send_message(
                                        cx,
                                        &this.tokio_handle,
                                        u1.clone(),
                                        text,
                                        is_group,
                                    );
                                }
                            }
                            "backspace" => {
                                chat.draft_text.pop();
                            }
                            "space" => {
                                chat.draft_text.push(' ');
                            }
                            _ => {
                                if !ch.is_empty() {
                                    chat.draft_text.push_str(ch);
                                }
                            }
                        }
                        cx.notify();
                    },
                ))
                .child(if d.is_empty() {
                    div()
                        .text_color(rgb(theme::CLR_MUTED))
                        .child("| 输入消息, Enter发送, Shift+Enter换行")
                        .into_any_element()
                } else {
                    div()
                        .text_color(rgb(theme::CLR_TEXT))
                        .child(format!("{}|", d))
                        .into_any_element()
                }),
        )
        // Send button
        .child(
            div()
                .id("send-msg-btn")
                .px_4().py_2().rounded_lg()
                .bg(rgb(theme::CLR_ACCENT))
                .cursor_pointer()
                .text_size(px(13.0))
                .text_color(rgb(0xffffff))
                .child("发送")
                .on_click(cx.listener(
                    move |this, _: &ClickEvent, _window, cx| {
                        let Some(chat) = this.chat_data.as_mut() else {
                            return;
                        };
                        let text = chat.draft_text.trim().to_string();
                        if !text.is_empty() {
                            chat.draft_text.clear();
                            chat_vm::send_message(
                                cx,
                                &this.tokio_handle,
                                u2.clone(),
                                text,
                                is_group,
                            );
                            cx.notify();
                        }
                    },
                )),
        )
}
