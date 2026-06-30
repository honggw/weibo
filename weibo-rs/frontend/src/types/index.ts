// ============================================================================
// TypeScript types — 与 Rust weibo-domain 类型对齐
// ============================================================================

/** Tab 导航 */
export type ActiveTab = "home" | "chat";

/** 登录阶段 */
export type LoginPhase =
  | "checking_cookie"
  | "loading"
  | "waiting_scan"
  | "exchanging"
  | "fetching_home"
  | "home_loaded"
  | "error";

/** 联系人 */
export interface Contact {
  user_id: string;
  screen_name: string;
  avatar: string;
  unread_count: number;
  last_message: string;
  last_time: string;
  is_group: boolean;
}

/** 消息类型 */
export type MsgType = "Normal" | "System" | "Recall" | { Other: number };

/** 媒体类型 */
export type MediaType = "Text" | "Image" | "Quote" | { Other: number };

/** 单条聊天消息 */
export interface ChatMessage {
  id: string;
  sender_id: string;
  sender_name: string;
  sender_avatar: string;
  text: string;
  created_at: string;
  timestamp: number;
  is_self: boolean;
  msg_type: string;
  media_type: string;
  fids: string[];
  role: number;
}

/** 表情 */
export interface Emotion {
  phrase: string;
  url: string;
}

/** 群信息 */
export interface GroupInfo {
  id: string;
  name: string;
  owner_uid: string;
  member_count: number;
  members: GroupMember[];
}

/** 群成员 */
export interface GroupMember {
  uid: string;
  screen_name: string;
  avatar: string;
  is_admin: boolean;
}

/** 时间线条目 */
export interface TimelineItem {
  user_name: string;
  text: string;
}

// ============================================================================
// State snapshot — from Rust events.rs StateSnapshot
// ============================================================================

export interface PhaseData {
  status: string | null;
  has_qr: boolean;
}

export interface TimelineItemSnapshot {
  user_name: string;
  text: string;
}

export interface StateSnapshot {
  phase: LoginPhase;
  phase_data: PhaseData;
  active_tab: ActiveTab;
  dm_unread: number;
  timeline_title: string;
  timeline_items: TimelineItemSnapshot[];
  has_more_timeline: boolean;
}
