//! 时间线相关的数据模型

use serde::{Deserialize, Serialize};

/// A single timeline item (post / hot-search entry)
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TimelineItem {
    pub user_name: String,
    pub text: String,
}
