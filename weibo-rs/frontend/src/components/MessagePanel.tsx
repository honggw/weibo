// ============================================================================
// MessagePanel — message list + input area + group sidebar
// ============================================================================

import { useState, useCallback, useEffect, useRef } from "react";
import { useAppStore } from "../hooks/useAppState";
import { loadOlderMessages, sendMessage } from "../tauri";
import MessageList from "./MessageList";
import MessageInput from "./MessageInput";
import EmojiPanel from "./EmojiPanel";
import GroupSidebar from "./GroupSidebar";

export default function MessagePanel() {
  const selectedUid = useAppStore((s) => s.selectedUid);
  const contacts = useAppStore((s) => s.contacts);
  const hasMoreMessages = useAppStore((s) => s.hasMoreMessages);
  const showEmojiPanel = useAppStore((s) => s.showEmojiPanel);
  const setShowEmojiPanel = useAppStore((s) => s.setShowEmojiPanel);
  const draftText = useAppStore((s) => s.draftText);
  const setDraftText = useAppStore((s) => s.setDraftText);
  const storeSync = useAppStore((s) => s.syncFromBackend);

  const [showGroupSidebar, setShowGroupSidebar] = useState(false);

  // Find selected contact
  const contact = contacts.find((c) => c.user_id === selectedUid);
  const isGroup = contact?.is_group ?? false;

  // Load older messages on scroll to top
  const handleLoadOlder = useCallback(async () => {
    if (!hasMoreMessages) return;
    try {
      await loadOlderMessages();
      await storeSync();
    } catch (e) {
      console.error("loadOlderMessages failed:", e);
    }
  }, [hasMoreMessages, storeSync]);

  // Send message
  const handleSend = useCallback(
    async (text: string) => {
      if (!selectedUid || !text.trim()) return;
      try {
        setDraftText("");
        await sendMessage(selectedUid, text, isGroup);
        await storeSync();
      } catch (e) {
        console.error("sendMessage failed:", e);
      }
    },
    [selectedUid, isGroup, setDraftText, storeSync]
  );

  // Insert emoji
  const handleEmojiSelect = useCallback(
    (phrase: string) => {
      setDraftText(draftText + phrase);
      setShowEmojiPanel(false);
    },
    [draftText, setDraftText, setShowEmojiPanel]
  );

  if (!contact) {
    return (
      <div className="h-full flex items-center justify-center text-text-secondary">
        <p>加载中...</p>
      </div>
    );
  }

  return (
    <div className="h-full flex flex-col relative">
      {/* Header */}
      <div className="flex items-center justify-between px-4 py-3 border-b border-white/5 bg-header-bg/50">
        <div className="flex items-center gap-3">
          <div
            className={`w-8 h-8 rounded-full flex items-center justify-center text-white text-xs font-bold ${
              isGroup ? "bg-green-600/60" : "bg-accent/40"
            }`}
          >
            {isGroup ? "群" : contact.screen_name.charAt(0)}
          </div>
          <span className="font-medium text-text-primary text-sm">
            {contact.screen_name}
          </span>
        </div>
        {isGroup && (
          <button
            onClick={() => setShowGroupSidebar(!showGroupSidebar)}
            className="text-text-secondary hover:text-white text-sm px-2 py-1 rounded"
          >
            {showGroupSidebar ? "关闭成员" : "群成员"}
          </button>
        )}
      </div>

      {/* Messages */}
      <div className="flex-1 min-h-0">
        <MessageList onLoadOlder={handleLoadOlder} hasMore={hasMoreMessages} />
      </div>

      {/* Input area */}
      <div className="border-t border-white/5">
        {showEmojiPanel && <EmojiPanel onSelect={handleEmojiSelect} />}
        <MessageInput
          draftText={draftText}
          onDraftChange={setDraftText}
          onSend={handleSend}
          onToggleEmoji={() => setShowEmojiPanel(!showEmojiPanel)}
          showEmoji={showEmojiPanel}
        />
      </div>

      {/* Group sidebar overlay */}
      {showGroupSidebar && isGroup && (
        <GroupSidebar onClose={() => setShowGroupSidebar(false)} />
      )}
    </div>
  );
}
