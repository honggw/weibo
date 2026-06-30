//! Tab 导航相关的数据模型

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub enum ActiveTab {
    Home,
    Chat,
}
