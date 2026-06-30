// ============================================================================
// Application state management — Zustand store
// ============================================================================

import { create } from "zustand";
import type { StateSnapshot, ActiveTab, ChatMessage, Contact, GroupInfo, Emotion } from "../types";
import * as tauri from "../tauri";

// ---------------------------------------------------------------------------
// Store shape
// ---------------------------------------------------------------------------

interface AppStore {
  // ---- Backend-synced state ----
  phase: string;
  phaseData: { status: string | null; hasQr: boolean };
  activeTab: ActiveTab;
  dmUnread: number;
  timelineTitle: string;
  timelineItems: { user_name: string; text: string }[];
  hasMoreTimeline: boolean;

  // ---- Chat state (frontend-managed, fetched async) ----
  contacts: Contact[];
  contactsLoading: boolean;
  myUid: string;
  selectedUid: string | null;
  messages: ChatMessage[];
  messagesLoading: boolean;
  hasMoreMessages: boolean;
  groupInfo: GroupInfo | null;
  emotions: Emotion[];

  // ---- UI-only state ----
  draftText: string;
  showEmojiPanel: boolean;
  searchText: string;
  qrPolling: boolean;

  // ---- Actions ----
  syncFromBackend: () => Promise<void>;
  setQrPolling: (v: boolean) => void;
  setDraftText: (v: string) => void;
  setShowEmojiPanel: (v: boolean) => void;
  setSearchText: (v: string) => void;
}

export const useAppStore = create<AppStore>((set, get) => ({
  // Initial state
  phase: "checking_cookie",
  phaseData: { status: "正在初始化...", hasQr: false },
  activeTab: "home" as ActiveTab,
  dmUnread: 0,
  timelineTitle: "",
  timelineItems: [],
  hasMoreTimeline: false,

  contacts: [],
  contactsLoading: true,
  myUid: "",
  selectedUid: null,
  messages: [],
  messagesLoading: false,
  hasMoreMessages: true,
  groupInfo: null,
  emotions: [],

  draftText: "",
  showEmojiPanel: false,
  searchText: "",
  qrPolling: false,

  // ---- Sync from backend ----
  syncFromBackend: async () => {
    try {
      const snap: StateSnapshot = await tauri.getState();
      set({
        phase: snap.phase,
        phaseData: {
          status: snap.phase_data.status,
          hasQr: snap.phase_data.has_qr,
        },
        activeTab: snap.active_tab,
        dmUnread: snap.dm_unread,
        timelineTitle: snap.timeline_title,
        timelineItems: snap.timeline_items,
        hasMoreTimeline: snap.has_more_timeline,
      });
    } catch (e) {
      console.error("syncFromBackend failed:", e);
    }
  },

  setQrPolling: (v) => set({ qrPolling: v }),
  setDraftText: (v) => set({ draftText: v }),
  setShowEmojiPanel: (v) => set({ showEmojiPanel: v }),
  setSearchText: (v) => set({ searchText: v }),
}));
