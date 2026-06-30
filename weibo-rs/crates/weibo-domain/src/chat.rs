//! 聊天相关的数据模型

use serde::{Deserialize, Serialize};

/// A conversation contact in the DM list.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Contact {
    pub user_id: String,
    pub screen_name: String,
    pub avatar: String,
    pub unread_count: u64,
    pub last_message: String,
    pub last_time: String,
    pub is_group: bool,
}

/// 消息类型枚举 (来自 HAR 中 type 字段)
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum MsgType {
    /// 普通消息 (type=321)
    Normal,
    /// 系统消息: 入群通知等 (type=322)
    System,
    /// 撤回消息 (type=344)
    Recall,
    /// 其他未知类型
    Other(u64),
}

impl MsgType {
    pub fn from_api(type_val: u64) -> Self {
        match type_val {
            321 => MsgType::Normal,
            322 => MsgType::System,
            344 => MsgType::Recall,
            v => MsgType::Other(v),
        }
    }
}

/// 媒体类型枚举 (来自 HAR 中 media_type 字段)
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum MediaType {
    /// 纯文本 (media_type=0)
    Text,
    /// 图片 (media_type=1, 有 fids 字段)
    Image,
    /// 引用/转发 (media_type=14, content 中包含引用块)
    Quote,
    /// 其他
    Other(u64),
}

impl MediaType {
    pub fn from_api(val: u64) -> Self {
        match val {
            0 => MediaType::Text,
            1 => MediaType::Image,
            14 => MediaType::Quote,
            v => MediaType::Other(v),
        }
    }
}

/// 单条聊天消息
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ChatMessage {
    pub id: String,
    pub sender_id: String,
    pub sender_name: String,
    /// 发送者头像 URL (来自 from_user.profile_image_url)
    pub sender_avatar: String,
    pub text: String,
    pub created_at: String,
    /// Unix 时间戳 (秒), 用于时间分组和格式化
    pub timestamp: u64,
    pub is_self: bool,
    /// 消息类型: Normal / System / Recall
    pub msg_type: MsgType,
    /// 媒体类型: Text / Image / Quote
    pub media_type: MediaType,
    /// 图片消息的文件 ID 列表 (media_type=1 时非空)
    /// 用于拼接缩略图 URL: https://upload.api.weibo.com/2/mss/msget_thumbnail?fid={}&high=240&width=240&source=209678993
    pub fids: Vec<String>,
    /// 消息发送者在群中的角色 (0=普通, 1=管理员, 4=群主)
    pub role: u8,
}

/// 微博表情
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Emotion {
    /// 表情文本标记, 如 "[不愧是你]"
    pub phrase: String,
    /// 表情图片 URL
    pub url: String,
}

/// 群信息
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GroupInfo {
    pub id: String,
    pub name: String,
    pub owner_uid: String,
    pub member_count: u64,
    pub members: Vec<GroupMember>,
}

/// 群成员
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GroupMember {
    pub uid: String,
    pub screen_name: String,
    pub avatar: String,
    pub is_admin: bool,
}
