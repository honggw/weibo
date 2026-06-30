// ============================================================================
// ChatView — master-detail layout: contact list + message panel
// ============================================================================

import { useEffect, useRef } from "react";
import { useAppStore } from "../hooks/useAppState";
import { loadContacts } from "../tauri";
import ContactList from "../components/ContactList";
import MessagePanel from "../components/MessagePanel";

export default function ChatView() {
  const contacts = useAppStore((s) => s.contacts);
  const contactsLoading = useAppStore((s) => s.contactsLoading);
  const selectedUid = useAppStore((s) => s.selectedUid);
  const storeSync = useAppStore((s) => s.syncFromBackend);

  const loadedRef = useRef(false);

  // Load contacts on mount
  useEffect(() => {
    if (loadedRef.current) return;
    loadedRef.current = true;

    const doLoad = async () => {
      try {
        await loadContacts();
        await storeSync();
      } catch (e) {
        console.error("loadContacts failed:", e);
      }
    };
    doLoad();
  }, [storeSync]);

  return (
    <div className="h-full flex">
      {/* Contact list sidebar */}
      <div className="w-72 flex-shrink-0 border-r border-white/5 bg-card/50">
        <ContactList
          contacts={contacts}
          loading={contactsLoading}
          selectedUid={selectedUid}
        />
      </div>

      {/* Message panel */}
      <div className="flex-1 min-w-0">
        {selectedUid ? (
          <MessagePanel />
        ) : (
          <div className="h-full flex items-center justify-center text-text-secondary">
            <p>选择一个会话开始聊天</p>
          </div>
        )}
      </div>
    </div>
  );
}
