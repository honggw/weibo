# 微博 PC 客户端 — UI 层重构计划: VMContext Trait + Workspace 多模块架构

> 目标: 通过 VMContext trait 抽象将 ViewModel 与 UI 框架完全解耦，采用 Rust workspace 多 crate 架构，
> 使得 UI 层可以自由切换 (当前从 GPUI → Tauri)，而 ViewModel 及以下各层零改动。

---

## 一、架构设计总览

### 1.1 目标依赖关系

```
weibo-tauri (bin)                weibo-cli (bin, 可选)
    │                                │
    │ impl VMContext for Tauri       │ 不走 UI
    ▼                                ▼
weibo-viewmodel (lib)           weibo-model (lib)
    │                                │
    │ 依赖 trait, 不依赖具体框架      │
    ▼                                ▼
weibo-model (lib)               weibo-infra (lib)
    │                                │
    ▼                                ▼
weibo-infra (lib)               weibo-domain (lib)
    │
    ▼
weibo-domain (lib)
```

**核心原则**: `weibo-viewmodel` 的 `Cargo.toml` 中 **不出现** `gpui`、`tauri` 或任何 UI 框架依赖。

### 1.2 Workspace 目录结构

```
weibo-rs/                              ← workspace 根目录
├── Cargo.toml                         ← workspace 定义
├── crates/
│   ├── weibo-domain/                  ← 纯数据模型 (零外部依赖)
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── chat.rs               ← Contact, ChatMessage, Emotion, GroupInfo
│   │       ├── timeline.rs           ← TimelineItem
│   │       ├── auth.rs               ← LoginPhase, CookieData, QrStatus
│   │       ├── tabs.rs               ← ActiveTab
│   │       └── error.rs              ← AppError
│   │
│   ├── weibo-infra/                   ← 基础设施 (HTTP, WS, Cookie, Audio)
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── http_client.rs
│   │       ├── ws_client.rs
│   │       ├── cookie_io.rs
│   │       ├── audio.rs
│   │       ├── config.rs
│   │       └── logger.rs
│   │
│   ├── weibo-model/                   ← 业务服务层
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── auth_service.rs
│   │       ├── chat_service.rs
│   │       ├── timeline_service.rs
│   │       └── qr_login.rs
│   │
│   ├── weibo-viewmodel/               ← ViewModel (纯逻辑 + VMContext trait)
│   │   ├── Cargo.toml                 ← 仅依赖 weibo-domain, weibo-model, tokio
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── context.rs             ← VMContext trait 定义
│   │       ├── app_state.rs           ← AppState (纯数据状态)
│   │       ├── login_vm.rs            ← 登录流程逻辑
│   │       ├── home_vm.rs             ← 首页逻辑
│   │       └── chat_vm.rs             ← 聊天逻辑
│   │
│   └── weibo-tauri/                   ← Tauri 应用 (binary + VMContext 实现)
│       ├── Cargo.toml                 ← 依赖 tauri, weibo-viewmodel, weibo-model 等
│       ├── tauri.conf.json
│       ├── build.rs
│       ├── icons/
│       └── src/
│           ├── main.rs                ← Tauri 入口
│           ├── tauri_context.rs       ← impl VMContext for Tauri
│           ├── commands/              ← Tauri IPC commands (薄包装)
│           │   ├── mod.rs
│           │   ├── auth.rs
│           │   ├── chat.rs
│           │   └── timeline.rs
│           └── events.rs              ← 事件序列化定义
│
├── frontend/                          ← 前端 (Vue 3 + TypeScript)
│   ├── package.json
│   ├── vite.config.ts
│   ├── tsconfig.json
│   ├── index.html
│   └── src/
│       ├── main.ts
│       ├── App.vue
│       ├── router/
│       ├── stores/
│       ├── views/
│       ├── components/
│       ├── styles/
│       └── types/
│
├── CLAUDE.md
└── README.md
```

---

## 二、VMContext Trait 详细设计

### 2.1 Trait 定义

```rust
// crates/weibo-viewmodel/src/context.rs

use std::future::Future;
use std::pin::Pin;

/// ViewModel 对外界执行环境的唯一抽象。
///
/// 职责:
///   1. 通知 UI 层状态变更 (替代 gpui::cx.notify())
///   2. 调度异步任务并在完成时回写状态 (替代 gpui::cx.spawn + WeakEntity)
///   3. 延时等待 (替代 gpui::Timer::after)
///
/// 不同 UI 框架提供各自的实现:
///   - GPUI: WeakEntity + AsyncApp
///   - Tauri: AppHandle + emit
///   - 测试: MockContext (同步执行，方便断言)
pub trait VMContext: Send + Sync + 'static {
    /// 通知 UI 层: 状态已变更，需要刷新渲染。
    ///
    /// GPUI 实现: cx.notify()
    /// Tauri 实现: app.emit("state-changed", payload)
    fn notify(&self);

    /// 调度一个异步任务。
    ///
    /// 任务在后台执行，完成后通过 `on_done` 回调更新 ViewModel 状态。
    /// 实现者负责:
    ///   1. spawn 异步任务
    ///   2. 任务完成时获取 ViewModel 的可变引用
    ///   3. 调用 on_done 更新状态
    ///   4. 自动调用 notify()
    ///
    /// 类型约束:
    ///   - F: 异步操作本身 (如网络请求)
    ///   - T: 异步操作的返回值
    ///   - C: 状态更新回调
    fn spawn_task<F, T, C>(&self, task: F, on_done: C)
    where
        F: Future<Output = T> + Send + 'static,
        T: Send + 'static,
        C: FnOnce(&mut dyn VMState, T) + Send + 'static;

    /// 调度一个延时后执行的回调。
    ///
    /// 用于: QR 轮询间隔、加载动画延迟等。
    fn schedule_after<C>(&self, millis: u64, callback: C)
    where
        C: FnOnce(&mut dyn VMState) + Send + 'static;
}

/// ViewModel 状态的抽象引用 (用于 on_done 回调中的类型擦除)。
///
/// 让 trait 方法不需要在签名中携带具体的 AppState 泛型。
/// 实现者将 &mut AppState downcast 为 &mut dyn VMState。
pub trait VMState: Send {
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any;
}
```

### 2.2 为什么引入 `VMState` trait

问题: 如果 `spawn_task` 的回调签名是 `FnOnce(&mut AppState, T)`，那 `VMContext` trait 就必须携带泛型 `<S>` 或关联类型，导致无法做 trait object。

解决: 引入 `VMState` trait 做类型擦除，回调中用 `downcast_mut` 取回具体类型:

```rust
impl VMState for AppState {
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any { self }
}

// ViewModel 中使用:
ctx.spawn_task(
    async { auth_service::prepare_qr().await },
    |state, result| {
        let app = state.as_any_mut().downcast_mut::<AppState>().unwrap();
        app.phase = LoginPhase::WaitingScan { ... };
    },
);
```

### 2.3 替代方案: 泛型 VMContext (无类型擦除)

如果不想用 downcast，可以让 trait 带关联类型:

```rust
pub trait VMContext {
    type State;

    fn notify(&self);
    fn spawn_task<F, T, C>(&self, task: F, on_done: C)
    where
        F: Future<Output = T> + Send + 'static,
        T: Send + 'static,
        C: FnOnce(&mut Self::State, T) + Send + 'static;
}
```

**权衡**:
| 方案 | 优点 | 缺点 |
|------|------|------|
| `VMState` 类型擦除 | 可做 `dyn VMContext`, 灵活 | 需要 downcast, 运行时有微小开销 |
| 关联类型 `type State` | 编译期安全, 无 downcast | 不能做 trait object, 泛型传染 |

**推荐**: 对于本项目，只有一个 `AppState`，用 **关联类型** 更简洁安全。

### 2.4 最终推荐签名 (关联类型版)

```rust
// crates/weibo-viewmodel/src/context.rs

pub trait VMContext: Send + Sync + 'static {
    type State: Send + 'static;

    /// 通知 UI 状态变更
    fn notify(&self);

    /// 调度异步任务, 完成后回调更新状态
    fn spawn_task<F, T, C>(&self, task: F, on_done: C)
    where
        F: Future<Output = T> + Send + 'static,
        T: Send + 'static,
        C: FnOnce(&mut Self::State, T) + Send + 'static;

    /// 延时后回调
    fn schedule_after<C>(&self, millis: u64, callback: C)
    where
        C: FnOnce(&mut Self::State) + Send + 'static;
}
```

---

## 三、各 Crate 详细设计

### 3.1 `weibo-domain` — 纯数据模型

**Cargo.toml 依赖**: 仅 `serde` (序列化)

```toml
[package]
name = "weibo-domain"
version = "0.1.0"
edition = "2021"

[dependencies]
serde = { version = "1", features = ["derive"] }
```

**改动要点**:
- 从当前 `src/domain/mod.rs` 直接搬入
- 所有类型添加 `#[derive(Serialize, Deserialize)]`
- `LoginPhase` 中的 `qr_png_bytes: Option<Vec<u8>>` 保留 (序列化时前端用 base64)
- 将 `QrStatus` 从 `qr_login.rs` 搬入 domain (它是纯数据枚举)

**文件拆分**:

| 当前位置 | 新位置 | 内容 |
|---|---|---|
| `domain/mod.rs` 第 10-14 行 | `domain/tabs.rs` | `ActiveTab` |
| `domain/mod.rs` 第 20-172 行 | `domain/chat.rs` | `Contact`, `MsgType`, `MediaType`, `ChatMessage`, `Emotion`, `GroupInfo`, `GroupMember` |
| `domain/mod.rs` 第 136-142 行 | `domain/timeline.rs` | `TimelineItem` |
| `domain/mod.rs` 第 148-186 行 | `domain/auth.rs` | `LoginPhase`, `CookieData` |
| `domain/mod.rs` 第 193-228 行 | `domain/error.rs` | `AppError` |
| `qr_login.rs` 中的 `QrStatus` | `domain/auth.rs` | `QrStatus` |

---

### 3.2 `weibo-infra` — 基础设施

**Cargo.toml 依赖**: `reqwest`, `tokio-tungstenite`, `rodio`, `serde_json`, `weibo-domain`

```toml
[package]
name = "weibo-infra"
version = "0.1.0"
edition = "2021"

[dependencies]
weibo-domain = { path = "../weibo-domain" }
reqwest = { version = "0.12", features = ["cookies", "json"] }
tokio = { version = "1", features = ["full"] }
tokio-tungstenite = { version = "0.24", features = ["native-tls"] }
futures-util = "0.3"
serde_json = "1"
rodio = "0.19"
anyhow = "1"
```

**改动要点**:
- 从当前 `src/infra/` 原样搬入，无逻辑修改
- `ws_client.rs` 中的 `WsMessage` 保持在 infra 层 (它是传输层协议结构)
- 若 `logger.rs` 使用了宏导出 (`#[macro_export]`)，需调整跨 crate 宏引用方式

---

### 3.3 `weibo-model` — 业务服务

**Cargo.toml 依赖**: `weibo-domain`, `weibo-infra`, `anyhow`, `serde_json`

```toml
[package]
name = "weibo-model"
version = "0.1.0"
edition = "2021"

[dependencies]
weibo-domain = { path = "../weibo-domain" }
weibo-infra = { path = "../weibo-infra" }
anyhow = "1"
serde_json = "1"
tokio = { version = "1", features = ["full"] }
```

**改动要点**:
- 从当前 `src/model/` 原样搬入
- `qr_login.rs` 中的协议逻辑搬入此 crate (它是业务逻辑，不是数据模型)
- 只需将 `use crate::infra::` 改为 `use weibo_infra::`
- 将 `use crate::domain::` 改为 `use weibo_domain::`
- 所有函数保持 `pub async fn` 签名不变

---

### 3.4 `weibo-viewmodel` — 核心: ViewModel + Trait

**Cargo.toml 依赖**: `weibo-domain`, `weibo-model`, `tokio` (仅 sync feature)

```toml
[package]
name = "weibo-viewmodel"
version = "0.1.0"
edition = "2021"

[dependencies]
weibo-domain = { path = "../weibo-domain" }
weibo-model = { path = "../weibo-model" }

# 注意: 不依赖任何 UI 框架!
# 仅需 tokio 的 mpsc channel (可选, 用于 WS 消息转发)
tokio = { version = "1", features = ["sync"] }
```

**注意**: **没有** `gpui`、**没有** `tauri`。

#### 3.4.1 `app_state.rs` — 纯状态容器

从 `root_vm.rs` 的 `AppRoot` 和 `chat_vm.rs` 的 `ChatData` 中提取纯数据:

```rust
// crates/weibo-viewmodel/src/app_state.rs

use weibo_domain::*;

/// 应用全局状态 (纯数据，不含任何 UI 框架类型)
pub struct AppState {
    /// 当前登录/加载阶段
    pub phase: LoginPhase,
    /// 当前激活 Tab
    pub active_tab: ActiveTab,
    /// 时间线状态
    pub timeline: TimelineState,
    /// 聊天状态
    pub chat: ChatState,
    /// DM 未读数
    pub dm_unread: u64,
}

pub struct TimelineState {
    pub items: Vec<TimelineItem>,
    pub title: String,
    pub since_id: String,
    pub feed_list_id: Option<String>,
    pub loading_more: bool,
}

/// 聊天状态 (纯业务数据)
pub struct ChatState {
    pub contacts: Vec<Contact>,
    pub contacts_loading: bool,
    pub my_uid: String,
    pub selected_uid: Option<String>,
    pub messages: Vec<ChatMessage>,
    pub messages_loading: bool,
    pub oldest_mid: Option<String>,
    pub has_more: bool,
    pub emotions: Vec<Emotion>,
    pub group_info: Option<GroupInfo>,
    // ❌ 不包含:
    //   - ListState (GPUI 专属)
    //   - FocusHandle (GPUI 专属)
    //   - draft_text (前端 UI 局部状态)
    //   - show_emoji_panel (前端 UI 局部状态)
    //   - search_text (前端 UI 局部状态)
}
```

#### 3.4.2 `login_vm.rs` — 登录逻辑 (解耦后)

```rust
// crates/weibo-viewmodel/src/login_vm.rs

use weibo_domain::*;
use weibo_model::auth_service;
use crate::context::VMContext;
use crate::app_state::AppState;

/// 启动 QR 扫码登录全流程。
pub fn start_login_flow<C: VMContext<State = AppState>>(ctx: &C) {
    // Step 1: 获取 QR 码
    ctx.spawn_task(
        async { auth_service::prepare_qr().await },
        |state, result| {
            match result {
                Ok((_login, png_bytes)) => {
                    state.phase = LoginPhase::WaitingScan {
                        status: "请用微博手机客户端扫描二维码".into(),
                        qr_png_bytes: Some(png_bytes),
                    };
                    // TODO: 保存 login 实例用于后续 poll
                    // 启动轮询 (见下方)
                }
                Err(e) => {
                    state.phase = LoginPhase::Error(format!("连接失败: {}", e));
                }
            }
        },
    );
}

/// QR 轮询单次检查
pub fn poll_qr_once<C: VMContext<State = AppState>>(ctx: &C, /* login state */) {
    ctx.schedule_after(1000, |state| {
        // 内部再 spawn 一次 poll
        // ...
    });
}
```

#### 3.4.3 `chat_vm.rs` — 聊天逻辑 (解耦后)

```rust
// crates/weibo-viewmodel/src/chat_vm.rs

use weibo_domain::*;
use weibo_model::chat_service;
use crate::context::VMContext;
use crate::app_state::AppState;

/// 加载联系人列表
pub fn load_contacts<C: VMContext<State = AppState>>(ctx: &C) {
    ctx.spawn_task(
        async {
            let contacts = chat_service::fetch_contacts().await.unwrap_or_default();
            let my_info = chat_service::fetch_primary_info().await;
            (contacts, my_info)
        },
        |state, (contacts, my_info)| {
            state.chat.contacts = contacts;
            state.chat.contacts_loading = false;
            if let Some((uid, _)) = my_info {
                state.chat.my_uid = uid;
            }
        },
    );
}

/// 选中联系人, 加载消息历史
pub fn select_contact<C: VMContext<State = AppState>>(ctx: &C, uid: String, is_group: bool) {
    let my_uid = /* from state */;
    ctx.spawn_task(
        async move {
            chat_service::fetch_messages(&uid, &my_uid, is_group, None).await
        },
        |state, messages| {
            state.chat.selected_uid = Some(uid);
            state.chat.oldest_mid = messages.first().map(|m| m.id.clone());
            state.chat.has_more = messages.len() >= 30;
            state.chat.messages = messages;
            state.chat.messages_loading = false;
        },
    );
}

/// 发送消息
pub fn send_message<C: VMContext<State = AppState>>(ctx: &C, uid: String, text: String, is_group: bool) {
    ctx.spawn_task(
        async move {
            chat_service::send_message(&uid, &text, is_group).await
        },
        |state, sent| {
            if let Some(msg) = sent {
                state.chat.messages.push(msg);
            }
        },
    );
}

/// 加载更早消息
pub fn load_older_messages<C: VMContext<State = AppState>>(ctx: &C) {
    // ...
}
```

#### 3.4.4 状态读取: ViewModel 如何在回调中读状态

问题: `spawn_task` 的异步闭包需要从 state 中读取数据 (如 `my_uid`) 来构造请求参数。

方案: 在调用 `spawn_task` 前从状态中提取所需参数:

```rust
pub fn select_contact<C: VMContext<State = AppState>>(
    ctx: &C,
    state: &AppState,      // ← 传入当前状态的不可变引用
    uid: String,
    is_group: bool,
) {
    let my_uid = state.chat.my_uid.clone(); // 提取后 move 进 async
    ctx.spawn_task(
        async move {
            chat_service::fetch_messages(&uid, &my_uid, is_group, None).await
        },
        |state, messages| { /* ... */ },
    );
}
```

---

### 3.5 `weibo-tauri` — Tauri 应用

**Cargo.toml 依赖**:

```toml
[package]
name = "weibo-tauri"
version = "0.1.0"
edition = "2021"

[dependencies]
weibo-domain = { path = "../weibo-domain" }
weibo-infra = { path = "../weibo-infra" }
weibo-model = { path = "../weibo-model" }
weibo-viewmodel = { path = "../weibo-viewmodel" }

tauri = { version = "2", features = [] }
tauri-plugin-shell = "2"
serde = { version = "1", features = ["derive"] }
serde_json = "1"
tokio = { version = "1", features = ["full"] }
```

#### 3.5.1 `tauri_context.rs` — impl VMContext for Tauri

```rust
// crates/weibo-tauri/src/tauri_context.rs

use std::sync::Arc;
use tokio::sync::RwLock;
use tauri::{AppHandle, Emitter};

use weibo_viewmodel::context::VMContext;
use weibo_viewmodel::app_state::AppState;

/// Tauri 对 VMContext 的实现
pub struct TauriContext {
    app: AppHandle,
    state: Arc<RwLock<AppState>>,
}

impl TauriContext {
    pub fn new(app: AppHandle, state: Arc<RwLock<AppState>>) -> Self {
        Self { app, state }
    }
}

impl VMContext for TauriContext {
    type State = AppState;

    fn notify(&self) {
        // 把当前状态的摘要 emit 给前端
        // (不发送全量状态, 只发变更事件, 前端按需 invoke 获取详情)
        self.app.emit("state-changed", ()).ok();
    }

    fn spawn_task<F, T, C>(&self, task: F, on_done: C)
    where
        F: std::future::Future<Output = T> + Send + 'static,
        T: Send + 'static,
        C: FnOnce(&mut AppState, T) + Send + 'static,
    {
        let state = self.state.clone();
        let app = self.app.clone();
        tokio::spawn(async move {
            let result = task.await;
            let mut s = state.write().await;
            on_done(&mut s, result);
            app.emit("state-changed", ()).ok();
        });
    }

    fn schedule_after<C>(&self, millis: u64, callback: C)
    where
        C: FnOnce(&mut AppState) + Send + 'static,
    {
        let state = self.state.clone();
        let app = self.app.clone();
        tokio::spawn(async move {
            tokio::time::sleep(std::time::Duration::from_millis(millis)).await;
            let mut s = state.write().await;
            callback(&mut s);
            app.emit("state-changed", ()).ok();
        });
    }
}
```

#### 3.5.2 `commands/` — Tauri IPC 命令 (薄封装)

```rust
// crates/weibo-tauri/src/commands/auth.rs

use tauri::State;
use std::sync::Arc;
use tokio::sync::RwLock;
use weibo_viewmodel::{app_state::AppState, login_vm, home_vm};
use crate::tauri_context::TauriContext;

pub struct ManagedState {
    pub state: Arc<RwLock<AppState>>,
    pub ctx: Arc<TauriContext>,
}

/// 前端调用: 检查保存的 Cookie
#[tauri::command]
pub async fn check_saved_cookie(managed: State<'_, ManagedState>) -> Result<(), String> {
    let state = managed.state.read().await;
    home_vm::check_cookie(&*managed.ctx, &state);
    Ok(())
}

/// 前端调用: 启动扫码登录
#[tauri::command]
pub async fn start_qr_login(managed: State<'_, ManagedState>) -> Result<(), String> {
    login_vm::start_login_flow(&*managed.ctx);
    Ok(())
}

/// 前端调用: 获取当前状态快照 (前端初始化/同步用)
#[tauri::command]
pub async fn get_state(managed: State<'_, ManagedState>) -> Result<StateSnapshot, String> {
    let state = managed.state.read().await;
    Ok(StateSnapshot::from(&*state))
}
```

**设计要点**: Tauri command 只是一层薄封装，真正的逻辑在 `weibo-viewmodel` 中。

#### 3.5.3 `main.rs`

```rust
// crates/weibo-tauri/src/main.rs

mod commands;
mod events;
mod tauri_context;

use std::sync::Arc;
use tokio::sync::RwLock;
use weibo_viewmodel::app_state::AppState;
use tauri_context::TauriContext;
use commands::ManagedState;

fn main() {
    // TLS provider
    let _ = rustls::crypto::CryptoProvider::install_default(
        rustls::crypto::aws_lc_rs::default_provider()
    );

    tauri::Builder::default()
        .setup(|app| {
            let state = Arc::new(RwLock::new(AppState::new()));
            let ctx = Arc::new(TauriContext::new(app.handle().clone(), state.clone()));
            app.manage(ManagedState { state, ctx });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::auth::check_saved_cookie,
            commands::auth::start_qr_login,
            commands::auth::get_state,
            commands::chat::load_contacts,
            commands::chat::select_contact,
            commands::chat::send_message,
            commands::chat::load_older_messages,
            commands::timeline::get_timeline,
            commands::timeline::load_more_timeline,
        ])
        .run(tauri::generate_context!())
        .expect("启动 Tauri 应用失败");
}
```

---

## 四、QR 登录轮询的特殊处理

QR 轮询是一个 **长生命周期的异步循环** (prepare → poll → poll → ... → confirm → exchange)。
它需要跨多次 `spawn_task` 保持 `QrLogin` 实例的状态。

### 4.1 方案: 将 QrLogin 存入 AppState

```rust
// app_state.rs
pub struct AppState {
    // ...
    /// QR 登录会话 (仅在登录流程中有值)
    pub qr_session: Option<QrSession>,
}

pub struct QrSession {
    pub login: weibo_model::QrLogin,  // QR 登录协议状态机
    pub polling: bool,                 // 是否正在轮询
}
```

### 4.2 轮询实现

```rust
// login_vm.rs

/// 启动登录: prepare → 显示 QR → 开始轮询
pub fn start_login_flow<C: VMContext<State = AppState>>(ctx: &C) {
    ctx.spawn_task(
        async { auth_service::prepare_qr().await },
        |state, result| {
            match result {
                Ok((login, png_bytes)) => {
                    state.phase = LoginPhase::WaitingScan { ... };
                    state.qr_session = Some(QrSession { login, polling: true });
                    // 启动轮询定时器 (1 秒后)
                    schedule_next_poll(ctx);
                }
                Err(e) => state.phase = LoginPhase::Error(...),
            }
        },
    );
}

/// 调度下一次轮询
fn schedule_next_poll<C: VMContext<State = AppState>>(ctx: &C) {
    ctx.schedule_after(1000, |state| {
        if let Some(ref session) = state.qr_session {
            if session.polling {
                // 再次 spawn 一次 poll 请求
                do_poll_once(ctx, state);
            }
        }
    });
}
```

### 4.3 问题: `schedule_after` 回调中无法再调 `ctx`

`schedule_after` 的回调签名是 `FnOnce(&mut State)`，此时没有 `ctx` 可用。

**解决方案**: 在 VMContext trait 中增加一个方法，让回调可以继续调度:

```rust
pub trait VMContext: Send + Sync + 'static {
    type State: Send + 'static;

    fn notify(&self);

    fn spawn_task<F, T, C>(&self, task: F, on_done: C) where ...;

    fn schedule_after<C>(&self, millis: u64, callback: C) where ...;

    /// 获取 Context 的克隆 (Arc 包装), 用于在回调中继续调度。
    fn clone_ctx(&self) -> Arc<dyn VMContext<State = Self::State>>;
}
```

然后:

```rust
pub fn start_login_flow(ctx: &Arc<dyn VMContext<State = AppState>>) {
    let ctx2 = ctx.clone();
    ctx.spawn_task(
        async { auth_service::prepare_qr().await },
        move |state, result| {
            // ...
            schedule_next_poll(&ctx2);  // ← 可以继续调度
        },
    );
}
```

或者更优雅的方案：让 `spawn_task` 的回调同时接收 `ctx`:

```rust
pub trait VMContext: Send + Sync + 'static {
    type State: Send + 'static;

    fn notify(&self);

    /// 回调同时接收 &dyn VMContext, 允许链式调度
    fn spawn_task<F, T, C>(&self, task: F, on_done: C)
    where
        F: Future<Output = T> + Send + 'static,
        T: Send + 'static,
        C: FnOnce(&mut Self::State, T, &dyn VMContext<State = Self::State>) + Send + 'static;

    fn schedule_after<C>(&self, millis: u64, callback: C)
    where
        C: FnOnce(&mut Self::State, &dyn VMContext<State = Self::State>) + Send + 'static;
}
```

**最终签名** (推荐):

```rust
// 回调中可以继续 spawn / schedule, 实现多步异步链
ctx.spawn_task(
    async { auth_service::prepare_qr().await },
    |state, result, ctx| {      // ← 第三个参数是 ctx 自身
        state.phase = LoginPhase::WaitingScan { ... };
        schedule_next_poll(ctx); // 继续调度
    },
);
```

---

## 五、测试策略

### 5.1 MockContext — 用于 ViewModel 单元测试

```rust
// crates/weibo-viewmodel/src/context.rs (或 tests/ 中)

#[cfg(test)]
pub struct MockContext {
    state: std::cell::RefCell<AppState>,
    notified: std::cell::Cell<u32>,
}

#[cfg(test)]
impl VMContext for MockContext {
    type State = AppState;

    fn notify(&self) {
        self.notified.set(self.notified.get() + 1);
    }

    fn spawn_task<F, T, C>(&self, task: F, on_done: C)
    where
        F: Future<Output = T> + Send + 'static,
        T: Send + 'static,
        C: FnOnce(&mut AppState, T, &dyn VMContext<State = AppState>) + Send + 'static,
    {
        // 测试中同步执行:
        let rt = tokio::runtime::Runtime::new().unwrap();
        let result = rt.block_on(task);
        on_done(&mut self.state.borrow_mut(), result, self);
    }

    fn schedule_after<C>(&self, _millis: u64, callback: C)
    where
        C: FnOnce(&mut AppState, &dyn VMContext<State = AppState>) + Send + 'static,
    {
        // 测试中立即执行:
        callback(&mut self.state.borrow_mut(), self);
    }
}
```

**测试示例**:

```rust
#[test]
fn test_login_flow_sets_waiting_scan() {
    let ctx = MockContext::new(AppState::new());
    login_vm::start_login_flow(&ctx);
    assert!(matches!(ctx.state().phase, LoginPhase::WaitingScan { .. }));
    assert!(ctx.notified() > 0);
}
```

### 5.2 各层测试范围

| Crate | 测试重点 | 方式 |
|---|---|---|
| `weibo-domain` | 序列化/反序列化、枚举转换 | 普通单元测试 |
| `weibo-infra` | HTTP mock、WS 协议解析 | `mockito` + 单元测试 |
| `weibo-model` | 业务逻辑、API 解析 | mock infra + 单元测试 |
| `weibo-viewmodel` | **状态变迁、流程编排** | MockContext + 同步执行 |
| `weibo-tauri` | IPC 集成 | Tauri test utils + e2e |

---

## 六、分阶段实施计划

### Phase 0: Workspace 骨架 (1 天)

- [ ] 创建 `weibo-rs/` 目录结构
- [ ] 创建 workspace `Cargo.toml`
- [ ] 创建 5 个 crate 的 `Cargo.toml` (均可编译, 内容为空 lib)
- [ ] 验收: `cargo build --workspace` 编译通过

### Phase 1: 搬迁无耦合层 (1 天)

- [ ] `weibo-domain`: 从 `src/domain/mod.rs` 搬入，拆分文件，加 Serialize
- [ ] `weibo-infra`: 从 `src/infra/` 搬入，调整 `use` 路径
- [ ] `weibo-model`: 从 `src/model/` + `src/qr_login.rs` 搬入，调整 `use` 路径
- [ ] 验收: `cargo test -p weibo-domain -p weibo-infra -p weibo-model` 通过

### Phase 2: VMContext trait + AppState (2 天)

- [ ] 编写 `context.rs` — 完整 trait 签名 (含回调中的 ctx 参数)
- [ ] 编写 `app_state.rs` — 纯状态结构
- [ ] 编写 MockContext
- [ ] 验收: `cargo build -p weibo-viewmodel` 通过

### Phase 3: ViewModel 逻辑迁移 (3 天)

- [ ] `login_vm.rs` — QR 登录全流程 (prepare → poll → exchange → home)
- [ ] `home_vm.rs` — Cookie 验证 + 首页加载
- [ ] `chat_vm.rs` — 联系人加载、选中、发送、加载更多、WS 消息处理
- [ ] 为每个 vm 编写单元测试 (MockContext)
- [ ] 验收: `cargo test -p weibo-viewmodel` 全部通过

### Phase 4: Tauri 应用壳 (2 天)

- [ ] 创建 `weibo-tauri` crate + `tauri.conf.json`
- [ ] 实现 `TauriContext` (impl VMContext)
- [ ] 实现 `commands/` (薄封装，调用 viewmodel 函数)
- [ ] 实现 `events.rs` (状态序列化 payload)
- [ ] `main.rs` 组装
- [ ] 验收: `cargo tauri dev` 能启动窗口，后端命令可被调用

### Phase 5: 前端实现 (4-5 天)

- [ ] Vue 3 + Vite + Tailwind 初始化
- [ ] TypeScript 类型对齐 domain
- [ ] Pinia store + Tauri invoke/listen
- [ ] 视图组件逐个实现 (Login → Home → Chat)
- [ ] 虚拟滚动 (消息列表、联系人列表、时间线)
- [ ] 验收: 完整功能流程跑通

### Phase 6: 集成验证 + 清理 (1 天)

- [ ] 端到端测试: 冷启动、热启动、聊天、登出
- [ ] 删除旧 `src/` 中的 GPUI 代码
- [ ] 更新 CLAUDE.md、README
- [ ] 验收: workspace 干净编译, 无遗留 gpui 引用

---

## 七、从旧代码到新代码的映射表

### 7.1 文件级映射

| 旧文件 | 新位置 | 改动 |
|---|---|---|
| `src/domain/mod.rs` | `crates/weibo-domain/src/*.rs` | 拆分 + 加 Serialize |
| `src/infra/*.rs` | `crates/weibo-infra/src/*.rs` | 调整 use 路径 |
| `src/model/*.rs` | `crates/weibo-model/src/*.rs` | 调整 use 路径 |
| `src/qr_login.rs` | `crates/weibo-model/src/qr_login.rs` | 调整 use 路径 |
| `src/logger.rs` | `crates/weibo-infra/src/logger.rs` | 宏跨 crate 导出 |
| `src/viewmodel/root_vm.rs` | `crates/weibo-viewmodel/src/app_state.rs` | 提取纯状态，移除 GPUI |
| `src/viewmodel/login_vm.rs` | `crates/weibo-viewmodel/src/login_vm.rs` | 用 VMContext 重写 |
| `src/viewmodel/home_vm.rs` | `crates/weibo-viewmodel/src/home_vm.rs` | 用 VMContext 重写 |
| `src/viewmodel/chat_vm.rs` | `crates/weibo-viewmodel/src/chat_vm.rs` | 用 VMContext 重写 |
| `src/view/*.rs` (全部) | `frontend/src/` (Vue 组件) | 完全重写为 HTML/CSS/JS |
| `src/view/theme.rs` | `frontend/src/styles/theme.css` | 色值搬迁 |
| `src/main.rs` | `crates/weibo-tauri/src/main.rs` | Tauri 入口 |
| (新增) | `crates/weibo-viewmodel/src/context.rs` | VMContext trait |
| (新增) | `crates/weibo-tauri/src/tauri_context.rs` | impl VMContext |
| (新增) | `crates/weibo-tauri/src/commands/*.rs` | Tauri IPC |

### 7.2 概念级映射

| GPUI 概念 | 新架构中的对应 |
|---|---|
| `Entity<AppRoot>` | `Arc<RwLock<AppState>>` (Tauri managed) |
| `WeakEntity<AppRoot>` | `Arc<RwLock<AppState>>` (clone) |
| `cx.notify()` | `VMContext::notify()` → `app.emit(...)` |
| `cx.spawn(\|this, cx\| async { this.update(...) })` | `VMContext::spawn_task(async_fn, callback)` |
| `Timer::after(dur).await` | `VMContext::schedule_after(ms, callback)` |
| `ListState` | 前端 vue-virtual-scroller 组件 |
| `FocusHandle` | 前端 `ref` + `focus()` |
| `Render trait` | Vue 组件 `<template>` |
| `div().flex().px_4().child(...)` | HTML + Tailwind `<div class="flex px-4">` |
| `cx.listener(\|this, event, ...\|)` | Tauri command + Pinia action |

---

## 八、Workspace Cargo.toml

```toml
# weibo-rs/Cargo.toml

[workspace]
resolver = "2"
members = [
    "crates/weibo-domain",
    "crates/weibo-infra",
    "crates/weibo-model",
    "crates/weibo-viewmodel",
    "crates/weibo-tauri",
]

[workspace.package]
version = "0.2.0"
edition = "2021"
license = "MIT"

[workspace.dependencies]
# 共享依赖版本统一管理
serde = { version = "1", features = ["derive"] }
serde_json = "1"
anyhow = "1"
tokio = { version = "1", features = ["full"] }
```

---

## 九、风险与决策记录

| 风险 | 影响 | 缓解措施 |
|---|---|---|
| VMContext trait 泛型过复杂 | 编译错误难调试 | 先用关联类型，避免 dyn trait object |
| QR 轮询需要跨多次 spawn 保持状态 | 设计复杂度 | QrLogin 存入 AppState |
| logger 宏跨 crate 导出 | 编译问题 | 在 infra crate 用 `#[macro_export]`, 其他 crate 用 `weibo_infra::log_info!` |
| model 层的 `async fn` + Tauri 的 tokio runtime | 运行时冲突 | Tauri 2.x 自带 tokio runtime，model 直接 `.await` |
| 前端状态与后端状态同步 | 数据不一致 | 单向数据流: 后端 emit → 前端 listen, 前端 invoke → 后端更新 |

---

## 十、最终架构优势

1. **ViewModel 可独立测试** — MockContext 同步执行，无需启动任何 UI 框架
2. **UI 可替换** — 未来若要换回原生 UI (如 iced、slint)，只需实现新的 VMContext
3. **编译隔离** — 改前端不重编译 ViewModel；改 ViewModel 不重编译 infra/model
4. **关注点分离清晰** — 每个 crate 职责单一，依赖方向单向向下
5. **符合 TDD** — 先写 VMContext trait + MockContext + 测试，再接 Tauri 实现
