# 微博 PC 客户端 — UI 层重构计划: GPUI → Tauri

> 目标: 将 UI 渲染层从 GPUI 迁移到 Tauri (WebView)，保留底层业务逻辑不变。

---

## 一、现状分析

### 1.1 当前模块结构

```
src/
├── domain/mod.rs          ← 纯数据结构 (零依赖)
├── infra/                 ← 基础设施 (HTTP, WS, Cookie, Audio, Config)
├── model/                 ← 业务服务 (auth_service, chat_service, timeline_service)
├── viewmodel/             ← 状态机 (root_vm, login_vm, home_vm, chat_vm)
├── view/                  ← GPUI 渲染 (screens, widgets, theme, app_shell)
├── cli/                   ← CLI 模式 (QR/Cookie 登录)
├── legacy/                ← 遗留代码 (WebView login, proxy)
├── qr_login.rs            ← QR 登录协议
├── logger.rs              ← 日志宏
└── main.rs                ← 入口
```

### 1.2 GPUI 耦合清单

| 模块 | GPUI 依赖项 | 耦合程度 |
|------|------------|---------|
| `view/` (全部 11 个文件) | `div()`, `list()`, `Render`, `IntoElement`, `AnyElement`, `px()`, `rgb()`, 事件系统 | 完全耦合 |
| `viewmodel/root_vm.rs` | `Entity<AppRoot>`, `Context<Self>`, `Render` trait, `ListState`, `cx.spawn`, `cx.notify`, `WeakEntity` | 重度耦合 |
| `viewmodel/login_vm.rs` | `WeakEntity`, `AsyncApp`, `cx.spawn`, `cx.notify`, `Timer::after` | 重度耦合 |
| `viewmodel/home_vm.rs` | `WeakEntity`, `AsyncApp`, `cx.spawn`, `cx.notify`, `ListState`, `Timer::after` | 重度耦合 |
| `viewmodel/chat_vm.rs` | `WeakEntity`, `AsyncApp`, `cx.spawn`, `cx.notify`, `ListState`, `ListAlignment`, `FocusHandle` | 重度耦合 |
| `view/app_shell.rs` | `Application::new()`, `WindowOptions`, `cx.open_window` | 完全耦合 |
| `domain/` | 无 | 无耦合 ✅ |
| `infra/` | 无 | 无耦合 ✅ |
| `model/` | 无 | 无耦合 ✅ |
| `cli/` | 无 | 无耦合 ✅ |

---

## 二、目标架构

```
weibo/
├── src-tauri/                    ← Tauri Rust 后端
│   ├── Cargo.toml
│   ├── tauri.conf.json
│   ├── src/
│   │   ├── main.rs              ← Tauri 入口 (替代 GPUI Application)
│   │   ├── state.rs             ← AppState (替代 AppRoot Entity)
│   │   ├── commands/            ← Tauri Commands (替代 viewmodel/)
│   │   │   ├── mod.rs
│   │   │   ├── auth.rs          ← 登录相关命令
│   │   │   ├── chat.rs          ← 聊天相关命令
│   │   │   └── timeline.rs      ← 时间线相关命令
│   │   ├── events.rs            ← Tauri Events 定义 (替代 cx.notify)
│   │   ├── domain/              ← 原样搬入 + 加 Serialize
│   │   │   └── mod.rs
│   │   ├── infra/               ← 原样搬入
│   │   │   ├── mod.rs
│   │   │   ├── http_client.rs
│   │   │   ├── ws_client.rs
│   │   │   ├── cookie_io.rs
│   │   │   ├── audio.rs
│   │   │   ├── config.rs
│   │   │   └── logger.rs
│   │   ├── model/               ← 原样搬入
│   │   │   ├── mod.rs
│   │   │   ├── auth_service.rs
│   │   │   ├── chat_service.rs
│   │   │   └── timeline_service.rs
│   │   ├── cli/                 ← 原样搬入
│   │   └── qr_login.rs         ← 原样搬入
│   └── icons/
├── src/                          ← 前端 (WebView)
│   ├── index.html
│   ├── main.ts                  ← 前端入口
│   ├── App.vue                  ← 根组件
│   ├── router/                  ← 路由 (login / home / chat)
│   ├── stores/                  ← Pinia 状态管理 (替代 viewmodel 前端部分)
│   │   ├── auth.ts
│   │   ├── chat.ts
│   │   └── timeline.ts
│   ├── views/                   ← 页面 (替代 view/screens/)
│   │   ├── LoginView.vue
│   │   ├── HomeView.vue
│   │   └── ChatView.vue
│   ├── components/              ← 组件 (替代 view/widgets/)
│   │   ├── HeaderBar.vue
│   │   ├── ContactCard.vue
│   │   ├── MessageBubble.vue
│   │   ├── TimelineCard.vue
│   │   ├── QrDisplay.vue
│   │   ├── EmojiPanel.vue
│   │   └── MemberSidebar.vue
│   ├── styles/                  ← 样式 (替代 view/theme.rs)
│   │   └── theme.css
│   └── types/                   ← TypeScript 类型定义 (对应 domain/)
│       └── index.ts
├── package.json
├── vite.config.ts
└── tsconfig.json
```

---

## 三、技术选型

| 层面 | 选择 | 理由 |
|------|------|------|
| 桌面框架 | **Tauri 2.x** | Rust 后端、WebView 渲染、体积小、跨平台 |
| 前端框架 | **Vue 3 + TypeScript** | 组合式 API 适合状态管理、生态丰富、上手快 |
| 构建工具 | **Vite** | 快速 HMR、Tauri 官方推荐 |
| 状态管理 | **Pinia** | Vue 3 官方推荐、TypeScript 友好 |
| UI 组件库 | **不使用** (手写 CSS) | 保持轻量、设计还原度高 |
| 虚拟滚动 | **vue-virtual-scroller** 或 **@tanstack/vue-virtual** | 替代 GPUI ListState |
| CSS 方案 | **Tailwind CSS** | 原子类方式类似 GPUI 的 `.flex().px_4()` 风格 |

---

## 四、分阶段实施计划

### Phase 0: 项目初始化 (预计 1 天)

#### 0.1 创建 Tauri 项目骨架

```bash
# 在项目根目录
npm create tauri-app@latest . -- --template vue-ts
```

#### 0.2 迁移无耦合模块

将以下模块原样复制到 `src-tauri/src/`:
- `domain/mod.rs`
- `infra/` (全部 6 个文件)
- `model/` (全部 3 个文件)
- `cli/` (全部)
- `qr_login.rs`
- `logger.rs`

#### 0.3 配置 Cargo.toml

```toml
[package]
name = "weibo"
version = "0.2.0"
edition = "2021"

[dependencies]
tauri = { version = "2", features = ["tray-icon"] }
tauri-plugin-shell = "2"
serde = { version = "1", features = ["derive"] }
serde_json = "1"
tokio = { version = "1", features = ["full"] }

# 保留原有业务依赖
reqwest = { version = "0.12", features = ["cookies", "json"] }
anyhow = "1"
tokio-tungstenite = { version = "0.24", features = ["native-tls"] }
futures-util = "0.3"
base64 = "0.22"
rsa = "0.9"
rand = "0.8"
hex = "0.4"
url = "2"
aes = "0.8"
cbc = "0.1"
sha2 = "0.10"
image = "0.25"
rodio = "0.19"

# 移除 gpui, wry, tao
```

#### 0.4 验收标准
- [ ] `cargo build` (src-tauri) 编译通过
- [ ] `npm run tauri dev` 能打开空白窗口
- [ ] domain/infra/model 模块编译无报错

---

### Phase 1: 后端状态层重构 (预计 2 天)

#### 1.1 创建 `state.rs` — 全局应用状态

替代 GPUI 的 `Entity<AppRoot>`，使用 `Arc<RwLock<>>` 管理共享状态:

```rust
// src-tauri/src/state.rs
use tokio::sync::RwLock;
use crate::domain::*;

/// 全局应用状态 (Tauri managed state)
pub struct AppState {
    /// 当前登录阶段
    pub phase: RwLock<LoginPhase>,
    /// 聊天数据
    pub chat_data: RwLock<Option<ChatData>>,
    /// 时间线数据
    pub timeline: RwLock<TimelineState>,
    /// 当前激活的 Tab
    pub active_tab: RwLock<ActiveTab>,
    /// DM 未读数
    pub dm_unread: RwLock<u64>,
}

pub struct TimelineState {
    pub items: Vec<TimelineItem>,
    pub title: String,
    pub since_id: String,
    pub feed_list_id: Option<String>,
    pub loading_more: bool,
}

/// 聊天数据 (从 viewmodel/chat_vm.rs 的 ChatData 迁移, 去掉 UI 字段)
pub struct ChatData {
    pub contacts: Vec<Contact>,
    pub loading: bool,
    pub my_uid: String,
    pub selected_uid: Option<String>,
    pub messages: Vec<ChatMessage>,
    pub messages_loading: bool,
    pub oldest_mid: Option<String>,
    pub has_more: bool,
    pub emotions: Vec<Emotion>,
    pub group_info: Option<GroupInfo>,
    // 去掉: input_focus, msg_list_state, chat_list_state (UI 相关)
    // 去掉: draft_text, show_emoji_panel, search_text (前端管理)
}
```

**关键设计决策**:
- 去掉所有 `ListState`、`FocusHandle`、`px()` 等 GPUI UI 状态
- `draft_text`、`show_emoji_panel`、`search_text` 等纯前端交互状态移至前端 store
- 使用 `RwLock` 而非 `Mutex`，允许多个读取者并行 (事件推送 + command 查询)

#### 1.2 创建 `events.rs` — 事件定义

替代 `cx.notify()` 的推送机制:

```rust
// src-tauri/src/events.rs
use serde::Serialize;
use crate::domain::*;

/// 登录阶段变更事件
#[derive(Clone, Serialize)]
pub struct LoginPhaseChanged {
    pub phase: LoginPhasePayload,
}

#[derive(Clone, Serialize)]
#[serde(tag = "type")]
pub enum LoginPhasePayload {
    CheckingCookie,
    Loading { message: String },
    WaitingScan { status: String, qr_base64: Option<String> },
    Exchanging { message: String },
    FetchingHome,
    HomeLoaded { item_count: usize, title: String },
    Error { message: String },
}

/// 新消息推送
#[derive(Clone, Serialize)]
pub struct NewMessage {
    pub contact_uid: String,
    pub message: ChatMessagePayload,
}

/// 聊天消息 (序列化版)
#[derive(Clone, Serialize)]
pub struct ChatMessagePayload {
    pub id: String,
    pub sender_id: String,
    pub sender_name: String,
    pub sender_avatar: String,
    pub text: String,
    pub timestamp: u64,
    pub is_self: bool,
    pub msg_type: String,
    pub media_type: String,
    pub fids: Vec<String>,
    pub role: u8,
}

/// DM 未读数更新
#[derive(Clone, Serialize)]
pub struct DmUnreadChanged {
    pub count: u64,
}

/// 联系人列表更新
#[derive(Clone, Serialize)]
pub struct ContactsLoaded {
    pub contacts: Vec<ContactPayload>,
}

#[derive(Clone, Serialize)]
pub struct ContactPayload {
    pub user_id: String,
    pub screen_name: String,
    pub avatar: String,
    pub unread_count: u64,
    pub last_message: String,
    pub last_time: String,
    pub is_group: bool,
}
```

#### 1.3 验收标准
- [ ] `state.rs` 编译通过，所有字段类型与 domain 对齐
- [ ] `events.rs` 所有事件类型可序列化 (`serde_json::to_string` 测试通过)
- [ ] 编写单元测试验证 state 的读写锁行为

---

### Phase 2: Tauri Commands 层 (预计 3 天)

替代 viewmodel 中的异步流程编排，将 `cx.spawn` + `cx.notify` 模式转为 `#[tauri::command]` + `app.emit`。

#### 2.1 `commands/auth.rs` — 登录命令

替代 `login_vm.rs` + `home_vm.rs`:

```rust
// src-tauri/src/commands/auth.rs
use tauri::{AppHandle, State, Emitter};
use crate::state::AppState;
use crate::model::{auth_service, timeline_service};
use crate::events::*;

/// 检查已保存的 Cookie 并自动登录
#[tauri::command]
pub async fn check_saved_cookie(app: AppHandle, state: State<'_, AppState>) -> Result<(), String> {
    // 对应 home_vm::start_cookie_flow
    let cookie = auth_service::load_saved_cookie();
    match cookie {
        Some(cookie) => {
            app.emit("login-phase-changed", LoginPhasePayload::Loading {
                message: "验证 Cookie...".into()
            }).ok();

            let valid = auth_service::verify_cookie(&cookie).await.unwrap_or(false);
            if valid {
                // 加载首页
                load_home_timeline(&app, &state).await;
            } else {
                app.emit("login-phase-changed", LoginPhasePayload::Loading {
                    message: "Cookie 已过期, 请重新登录".into()
                }).ok();
            }
        }
        None => {
            app.emit("login-phase-changed", LoginPhasePayload::Loading {
                message: "请扫码登录".into()
            }).ok();
        }
    }
    Ok(())
}

/// 启动 QR 扫码登录流程
#[tauri::command]
pub async fn start_qr_login(app: AppHandle, state: State<'_, AppState>) -> Result<(), String> {
    // 对应 login_vm::start_login_flow (拆分为多步)
    // 1. prepare_qr
    // 2. emit WaitingScan 事件 (附带 base64 QR 图片)
    // 3. 轮询循环 (spawn 后台任务)
    // 4. 确认后 exchange_ticket → load_home_timeline
    todo!()
}

/// 登出
#[tauri::command]
pub async fn logout(app: AppHandle, state: State<'_, AppState>) -> Result<(), String> {
    crate::infra::cookie_io::delete();
    *state.phase.write().await = LoginPhase::CheckingCookie;
    app.emit("login-phase-changed", LoginPhasePayload::CheckingCookie).ok();
    Ok(())
}

/// 内部辅助: 加载首页时间线
async fn load_home_timeline(app: &AppHandle, state: &State<'_, AppState>) {
    app.emit("login-phase-changed", LoginPhasePayload::FetchingHome).ok();
    let (items, title, feed_list_id, since_id) = timeline_service::fetch_first_page().await;
    // 更新 state
    let mut tl = state.timeline.write().await;
    tl.items = items;
    tl.title = title.clone();
    tl.since_id = since_id;
    tl.feed_list_id = feed_list_id;
    // 通知前端
    app.emit("login-phase-changed", LoginPhasePayload::HomeLoaded {
        item_count: tl.items.len(),
        title,
    }).ok();
}
```

#### 2.2 `commands/chat.rs` — 聊天命令

替代 `chat_vm.rs`:

```rust
// src-tauri/src/commands/chat.rs

/// 加载联系人列表
#[tauri::command]
pub async fn load_contacts(app: AppHandle, state: State<'_, AppState>) -> Result<Vec<ContactPayload>, String> {
    let (contacts, my_info) = tokio::join!(
        chat_service::fetch_contacts(),
        chat_service::fetch_primary_info(),
    );
    // 更新 state 并返回
    todo!()
}

/// 选中一个联系人，加载消息历史
#[tauri::command]
pub async fn select_contact(
    app: AppHandle,
    state: State<'_, AppState>,
    uid: String,
    is_group: bool,
) -> Result<Vec<ChatMessagePayload>, String> {
    // 对应 chat_vm::select_contact
    todo!()
}

/// 加载更早消息 (分页)
#[tauri::command]
pub async fn load_older_messages(
    state: State<'_, AppState>,
    uid: String,
    is_group: bool,
) -> Result<Vec<ChatMessagePayload>, String> {
    // 对应 chat_vm::load_more_messages
    todo!()
}

/// 发送消息
#[tauri::command]
pub async fn send_message(
    app: AppHandle,
    state: State<'_, AppState>,
    uid: String,
    text: String,
    is_group: bool,
) -> Result<Option<ChatMessagePayload>, String> {
    // 对应 chat_vm::send_message
    todo!()
}

/// 获取表情列表
#[tauri::command]
pub async fn fetch_emotions(state: State<'_, AppState>) -> Result<Vec<EmotionPayload>, String> {
    todo!()
}
```

#### 2.3 `commands/timeline.rs` — 时间线命令

替代 `root_vm::try_load_more`:

```rust
// src-tauri/src/commands/timeline.rs

/// 获取当前时间线数据
#[tauri::command]
pub async fn get_timeline(state: State<'_, AppState>) -> Result<TimelinePayload, String> {
    todo!()
}

/// 加载更多时间线 (下拉加载)
#[tauri::command]
pub async fn load_more_timeline(state: State<'_, AppState>) -> Result<TimelinePayload, String> {
    // 对应 root_vm::try_load_more
    todo!()
}
```

#### 2.4 WebSocket 监听 → Tauri Event

替代 `root_vm` 中的 `cx.spawn` + `rx.recv()`:

```rust
// src-tauri/src/commands/chat.rs

/// 启动 WebSocket 长连接 (在登录成功后调用)
#[tauri::command]
pub async fn start_websocket(app: AppHandle, state: State<'_, AppState>) -> Result<(), String> {
    let chat = state.chat_data.read().await;
    let my_uid = chat.as_ref().map(|c| c.my_uid.clone()).unwrap_or_default();
    drop(chat);

    if my_uid.is_empty() { return Err("uid 不可用".into()); }

    let mut rx = chat_service::start_ws(my_uid, /* handle */);

    // 后台任务: 持续接收 WS 消息并 emit 事件
    let app_clone = app.clone();
    let state_inner = state.inner().clone();
    tokio::spawn(async move {
        while let Some(msg) = rx.recv().await {
            // 1. 更新 state.chat_data
            // 2. emit "new-message" 事件给前端
            app_clone.emit("new-message", NewMessage { ... }).ok();
            // 3. 播放提示音
            if !is_self {
                crate::infra::audio::play_notification();
            }
        }
    });

    Ok(())
}
```

#### 2.5 Tauri 入口 `main.rs`

```rust
// src-tauri/src/main.rs
mod commands;
mod domain;
mod events;
mod infra;
mod model;
mod qr_login;
mod state;
#[macro_use]
mod logger;

use state::AppState;

fn main() {
    let _ = rustls::crypto::CryptoProvider::install_default(
        rustls::crypto::aws_lc_rs::default_provider()
    );

    tauri::Builder::default()
        .manage(AppState::new())
        .invoke_handler(tauri::generate_handler![
            commands::auth::check_saved_cookie,
            commands::auth::start_qr_login,
            commands::auth::logout,
            commands::chat::load_contacts,
            commands::chat::select_contact,
            commands::chat::load_older_messages,
            commands::chat::send_message,
            commands::chat::fetch_emotions,
            commands::chat::start_websocket,
            commands::timeline::get_timeline,
            commands::timeline::load_more_timeline,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
```

#### 2.6 验收标准
- [ ] 所有 `#[tauri::command]` 编译通过
- [ ] 为每个 command 编写单元测试 (mock AppHandle)
- [ ] `cargo test` 通过

---

### Phase 3: domain 层适配 (预计 0.5 天)

#### 3.1 为所有 domain 类型添加 Serialize/Deserialize

```rust
// domain/mod.rs — 所有结构体添加:
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct Contact { ... }

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub enum LoginPhase { ... }

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct ChatMessage { ... }

// ... 所有 pub 类型
```

#### 3.2 注意事项
- `LoginPhase::WaitingScan.qr_png_bytes` 改为 `qr_base64: Option<String>` (WebView 中用 base64 显示图片)
- 或者保持 `Vec<u8>` 在后端, 在 event payload 中转为 base64

#### 3.3 验收标准
- [ ] 所有 domain 类型可以 `serde_json::to_string` 序列化
- [ ] 编写测试验证序列化/反序列化往返一致

---

### Phase 4: 前端框架搭建 (预计 1 天)

#### 4.1 项目初始化

```bash
# 前端依赖
npm install vue@3 vue-router@4 pinia @tauri-apps/api
npm install -D typescript vite @vitejs/plugin-vue tailwindcss autoprefixer
npm install vue-virtual-scroller  # 虚拟滚动
```

#### 4.2 TypeScript 类型定义

```typescript
// src/types/index.ts — 与 domain/ 对应
export interface Contact {
  user_id: string;
  screen_name: string;
  avatar: string;
  unread_count: number;
  last_message: string;
  last_time: string;
  is_group: boolean;
}

export interface ChatMessage {
  id: string;
  sender_id: string;
  sender_name: string;
  sender_avatar: string;
  text: string;
  timestamp: number;
  is_self: boolean;
  msg_type: 'Normal' | 'System' | 'Recall' | { Other: number };
  media_type: 'Text' | 'Image' | 'Quote' | { Other: number };
  fids: string[];
  role: number;
}

export interface TimelineItem {
  user_name: string;
  text: string;
}

export type LoginPhase =
  | { type: 'CheckingCookie' }
  | { type: 'Loading'; message: string }
  | { type: 'WaitingScan'; status: string; qr_base64: string | null }
  | { type: 'Exchanging'; message: string }
  | { type: 'FetchingHome' }
  | { type: 'HomeLoaded'; item_count: number; title: string }
  | { type: 'Error'; message: string };
```

#### 4.3 Pinia Store 设计

```typescript
// src/stores/auth.ts
export const useAuthStore = defineStore('auth', () => {
  const phase = ref<LoginPhase>({ type: 'CheckingCookie' });
  const isLoggedIn = computed(() => phase.value.type === 'HomeLoaded');

  async function checkCookie() { await invoke('check_saved_cookie'); }
  async function startQrLogin() { await invoke('start_qr_login'); }
  async function logout() { await invoke('logout'); }

  // 监听后端事件
  function setupListeners() {
    listen<LoginPhase>('login-phase-changed', (event) => {
      phase.value = event.payload;
    });
  }

  return { phase, isLoggedIn, checkCookie, startQrLogin, logout, setupListeners };
});

// src/stores/chat.ts
export const useChatStore = defineStore('chat', () => {
  const contacts = ref<Contact[]>([]);
  const selectedUid = ref<string | null>(null);
  const messages = ref<ChatMessage[]>([]);
  const draftText = ref('');           // 纯前端状态
  const showEmojiPanel = ref(false);   // 纯前端状态
  const searchText = ref('');          // 纯前端状态

  async function loadContacts() { ... }
  async function selectContact(uid: string, isGroup: boolean) { ... }
  async function sendMessage() { ... }
  async function loadOlderMessages() { ... }

  function setupListeners() {
    listen<NewMessage>('new-message', (event) => {
      // 追加消息、更新联系人预览
    });
  }

  return { contacts, selectedUid, messages, draftText, ... };
});
```

#### 4.4 路由配置

```typescript
// src/router/index.ts
const routes = [
  { path: '/login', component: LoginView },
  { path: '/', component: () => import('@/views/HomeView.vue') },
  { path: '/chat', component: () => import('@/views/ChatView.vue') },
];
```

#### 4.5 验收标准
- [ ] `npm run dev` 前端开发服务器启动
- [ ] 基础路由跳转工作正常
- [ ] Pinia store 可以调用 Tauri invoke (即使后端还是 todo)

---

### Phase 5: 前端视图实现 (预计 4-5 天)

#### 5.1 视图对应关系

| GPUI (旧) | Vue (新) | 功能 |
|-----------|---------|------|
| `view/app_shell.rs` | `App.vue` + `router` | 窗口骨架 + 路由 |
| `view/screens/root_screen.rs` | `App.vue` (tabs + body) | Tab 栏 + 内容路由 |
| `view/screens/login_screen.rs` | `views/LoginView.vue` | 登录界面 |
| `view/screens/home_screen.rs` | `views/HomeView.vue` | 时间线首页 |
| `view/screens/chat_screen.rs` | `views/ChatView.vue` | 聊天界面 |
| `view/widgets/header_bar.rs` | `components/HeaderBar.vue` | 顶部状态栏 |
| `view/widgets/contact_card.rs` | `components/ContactCard.vue` | 联系人卡片 |
| `view/widgets/message_bubble.rs` | `components/MessageBubble.vue` | 消息气泡 |
| `view/widgets/timeline_card.rs` | `components/TimelineCard.vue` | 时间线卡片 |
| `view/widgets/qr_display.rs` | `components/QrDisplay.vue` | QR 码显示 |
| `view/widgets/emoji_panel.rs` | `components/EmojiPanel.vue` | 表情面板 |
| `view/widgets/member_sidebar.rs` | `components/MemberSidebar.vue` | 群成员侧栏 |
| `view/widgets/centered_msg.rs` | `components/CenteredMsg.vue` | 居中提示文字 |
| `view/theme.rs` | `styles/theme.css` | 主题色值 |

#### 5.2 关键组件实现要点

**LoginView.vue:**
- 监听 `login-phase-changed` 事件切换 UI 状态
- QR 图片用 `<img :src="'data:image/png;base64,' + qrBase64" />`
- 状态提示文字响应式更新

**ChatView.vue (最复杂):**
- 联系人列表: 虚拟滚动 (`vue-virtual-scroller`)
- 消息列表: 虚拟滚动 + 滚动到底部 + 向上滚动加载历史
- 输入框: `v-model` + Ctrl+Enter 发送
- WebSocket 消息实时追加

**HomeView.vue:**
- 时间线卡片列表: 虚拟滚动
- 无限滚动加载更多

#### 5.3 主题迁移

```css
/* src/styles/theme.css — 对应 view/theme.rs */
:root {
  --clr-bg: #0d1b2a;
  --clr-text: #e0e6ed;
  --clr-accent: #4fc3f7;
  --clr-muted: #5a7a9a;
  --clr-card: #1b2838;
  --clr-surface: #152238;
  /* ... */
}
```

#### 5.4 验收标准
- [ ] 登录页: QR 码显示、状态切换、自动跳转
- [ ] 首页: 时间线列表渲染、无限滚动
- [ ] 聊天: 联系人列表、消息列表、发送消息、表情面板
- [ ] WebSocket: 实时收到新消息

---

### Phase 6: 集成测试 & 清理 (预计 1 天)

#### 6.1 端到端验证

- [ ] 冷启动: 无 Cookie → QR 扫码 → 首页加载
- [ ] 热启动: 有 Cookie → 自动登录 → 首页加载
- [ ] 聊天: 选择联系人 → 查看历史 → 发送消息 → 收到推送
- [ ] 登出: 点击登出 → 回到扫码界面
- [ ] 时间线: 滚动加载更多

#### 6.2 清理旧代码

- [ ] 删除 `src/view/` 目录 (全部 GPUI 渲染代码)
- [ ] 删除 `src/viewmodel/` 目录 (已被 commands/ 替代)
- [ ] 从 Cargo.toml 移除 `gpui`、`wry`、`tao` 依赖
- [ ] 删除 `build.rs` (如果只是为 gpui 服务的)
- [ ] 更新 `main.rs` 入口

#### 6.3 验收标准
- [ ] 无残留 GPUI 代码
- [ ] `cargo clippy` 无 warning
- [ ] `npm run build` 前端构建通过
- [ ] `cargo tauri build` 打包成功

---

## 五、风险与注意事项

### 5.1 model 层的 `block_on` 调用

当前 model 层的函数是 `async fn`，但在 viewmodel 中通过 `handle.block_on()` 调用。
迁移到 Tauri 后，`#[tauri::command]` 本身支持 `async`，可以直接 `.await`，不再需要 `block_on`。

**变化**:
```rust
// 旧: handle.block_on(auth_service::verify_cookie(&cookie))
// 新: auth_service::verify_cookie(&cookie).await
```

### 5.2 QR 轮询机制

GPUI 中用 `Timer::after(Duration::from_secs(1)).await` + 循环实现。
Tauri 中改为 `tokio::time::sleep` + `app.emit` 通知前端进度:

```rust
loop {
    let status = auth_service::poll_qr(&login).await;
    match status { ... }
    tokio::time::sleep(Duration::from_secs(1)).await;
}
```

### 5.3 image crate 用途

当前 `image = "0.25"` 仅用于 QR 码 PNG 解码 (生成 GPUI 可用的 RGBA 数据)。
迁移后，QR 码直接以 base64 PNG 传给前端 `<img>` 标签，**可能不再需要** image crate。

### 5.4 CLI 模式保留

`cli/` 模块 (终端 QR 登录、Cookie 登录) 与 UI 无关，保持不变。
可以在 `main.rs` 中用命令行参数判断是否走 CLI 模式:

```rust
fn main() {
    let args: Vec<String> = std::env::args().collect();
    match args.get(1).map(|s| s.as_str()) {
        Some("cookie") => { /* CLI cookie login */ }
        Some("http") => { /* CLI QR login */ }
        _ => { /* Tauri GUI mode */ }
    }
}
```

### 5.5 音频播放

`infra/audio.rs` 使用 `rodio`，与 UI 框架无关，可以直接在 Tauri 后端的 WS 消息处理中调用。

### 5.6 跨平台注意

- Tauri 2.x 在 Linux 上使用 WebKitGTK，需确保开发环境安装了相关依赖
- 字体渲染: 原 GPUI 指定 "Microsoft YaHei"，Web 中可用 `font-family` CSS 自适应

---

## 六、工作量总结

| Phase | 内容 | 预估工时 | 依赖 |
|-------|------|---------|------|
| 0 | 项目初始化 + 模块搬迁 | 1 天 | 无 |
| 1 | AppState + Events 定义 | 2 天 | Phase 0 |
| 2 | Tauri Commands 实现 | 3 天 | Phase 1 |
| 3 | domain 序列化适配 | 0.5 天 | Phase 0 |
| 4 | 前端框架搭建 | 1 天 | Phase 0 |
| 5 | 前端视图实现 | 4-5 天 | Phase 2, 3, 4 |
| 6 | 集成测试 + 清理 | 1 天 | Phase 5 |
| **总计** | | **12-13 天** | |

**可并行**: Phase 3 + Phase 4 可与 Phase 1-2 并行开发。

---

## 七、迁移策略

采用 **平行开发** 而非逐步替换:

1. 在项目根目录新建 `src-tauri/` 和前端 `src/` (Tauri 标准结构)
2. 旧的 `src/` 重命名为 `src-gpui/` 保留作为参考
3. 两套代码共存期间，用 git branch 隔离
4. 新版本功能对齐后，删除旧代码，合入主分支

这样做的好处:
- 旧版本随时可用 (切回 branch)
- 不会因为半成品导致项目不可用
- 可以逐个功能对比验证
