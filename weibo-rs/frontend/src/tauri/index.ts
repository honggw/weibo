// ============================================================================
// Tauri IPC bridge — invoke commands + listen for events
// ============================================================================

import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import type { StateSnapshot } from "../types";

// ---------------------------------------------------------------------------
// Commands (invoke)
// ---------------------------------------------------------------------------

/** 检查已保存的 Cookie */
export async function checkSavedCookie(): Promise<void> {
  return invoke("check_saved_cookie");
}

/** 启动扫码登录 */
export async function startQrLogin(): Promise<void> {
  return invoke("start_qr_login");
}

/** 获取当前状态快照 */
export async function getState(): Promise<StateSnapshot> {
  return invoke("get_state");
}

/** 登出 */
export async function logout(): Promise<void> {
  return invoke("logout");
}

/** 加载联系人列表 */
export async function loadContacts(): Promise<void> {
  return invoke("load_contacts");
}

/** 选中联系人 */
export async function selectContact(
  uid: string,
  isGroup: boolean
): Promise<void> {
  return invoke("select_contact", { uid, isGroup });
}

/** 发送消息 */
export async function sendMessage(
  uid: string,
  text: string,
  isGroup: boolean
): Promise<void> {
  return invoke("send_message", { uid, text, isGroup });
}

/** 加载更早消息 */
export async function loadOlderMessages(): Promise<void> {
  return invoke("load_older_messages");
}

/** 加载更多时间线 */
export async function loadMoreTimeline(): Promise<void> {
  return invoke("load_more_timeline");
}

/** 获取 QR 码图片 (base64) */
export async function getQrImage(): Promise<QrImageResponse> {
  return invoke("get_qr_image");
}

/** 单次 QR 轮询 */
export async function pollQrOnce(): Promise<void> {
  return invoke("poll_qr_once");
}

/** QR 确认后继续 */
export async function confirmAndProceed(): Promise<void> {
  return invoke("confirm_and_proceed");
}

/** 刷新过期 QR */
export async function refreshQr(): Promise<void> {
  return invoke("refresh_qr");
}

/** 加载首页 */
export async function fetchHome(): Promise<void> {
  return invoke("fetch_home");
}

export interface QrImageResponse {
  has_qr: boolean;
  qr_base64: string | null;
  status: string;
}

// ---------------------------------------------------------------------------
// Events (listen)
// ---------------------------------------------------------------------------

/** 监听后端 state-changed 事件 */
export function onStateChanged(callback: () => void): () => void {
  const unlistenPromise = listen("state-changed", () => {
    callback();
  });

  // Return cleanup function
  let unlistenFn: (() => void) | null = null;
  unlistenPromise.then((fn) => {
    unlistenFn = fn;
  });

  return () => {
    if (unlistenFn) unlistenFn();
  };
}
