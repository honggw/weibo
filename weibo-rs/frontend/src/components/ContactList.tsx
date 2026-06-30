// ============================================================================
// ContactList — virtual-scrolled contact list with search
// ============================================================================

import { useCallback, useMemo } from "react";
import { Virtuoso } from "react-virtuoso";
import { useAppStore } from "../hooks/useAppState";
import { selectContact } from "../tauri";
import type { Contact } from "../types";

interface Props {
  contacts: Contact[];
  loading: boolean;
  selectedUid: string | null;
}

export default function ContactList({ contacts, loading, selectedUid }: Props) {
  const storeSync = useAppStore((s) => s.syncFromBackend);
  const searchText = useAppStore((s) => s.searchText);
  const setSearchText = useAppStore((s) => s.setSearchText);

  // Filter by search
  const filtered = useMemo(() => {
    if (!searchText.trim()) return contacts;
    const q = searchText.toLowerCase();
    return contacts.filter(
      (c) =>
        c.screen_name.toLowerCase().includes(q) ||
        c.last_message.toLowerCase().includes(q)
    );
  }, [contacts, searchText]);

  const handleSelect = useCallback(
    async (contact: Contact) => {
      try {
        await selectContact(contact.user_id, contact.is_group);
        await storeSync();
      } catch (e) {
        console.error("selectContact failed:", e);
      }
    },
    [storeSync]
  );

  if (loading) {
    return (
      <div className="h-full flex items-center justify-center">
        <div className="animate-spin w-6 h-6 border-2 border-accent border-t-transparent rounded-full" />
      </div>
    );
  }

  return (
    <div className="h-full flex flex-col">
      {/* Search */}
      <div className="p-3">
        <input
          type="text"
          value={searchText}
          onChange={(e) => setSearchText(e.target.value)}
          placeholder="搜索会话..."
          className="w-full px-3 py-2 bg-bg border border-white/10 rounded-lg text-sm text-text-primary placeholder-text-secondary focus:outline-none focus:border-accent/50"
        />
      </div>

      {/* List */}
      <div className="flex-1">
        <Virtuoso
          data={filtered}
          itemContent={(_, contact) => (
            <ContactRow
              contact={contact}
              selected={contact.user_id === selectedUid}
              onClick={() => handleSelect(contact)}
            />
          )}
        />
      </div>
    </div>
  );
}

// ---------------------------------------------------------------------------
// ContactRow
// ---------------------------------------------------------------------------

function ContactRow({
  contact,
  selected,
  onClick,
}: {
  contact: Contact;
  selected: boolean;
  onClick: () => void;
}) {
  return (
    <div
      onClick={onClick}
      className={`flex items-center gap-3 px-3 py-3 cursor-pointer transition-colors border-b border-white/5 ${
        selected
          ? "bg-accent/20 border-l-2 border-l-accent"
          : "hover:bg-white/[0.04] border-l-2 border-l-transparent"
      }`}
    >
      {/* Avatar */}
      <div className="relative flex-shrink-0">
        <div
          className={`w-11 h-11 rounded-full flex items-center justify-center text-white text-sm font-bold ${
            contact.is_group ? "bg-green-600/60" : "bg-accent/40"
          }`}
        >
          {contact.is_group ? "群" : contact.screen_name.charAt(0)}
        </div>
        {contact.unread_count > 0 && (
          <span className="absolute -top-1 -right-1 min-w-[16px] h-[16px] flex items-center justify-center bg-red-500 text-white text-[10px] font-bold rounded-full px-1">
            {contact.unread_count > 99 ? "99+" : contact.unread_count}
          </span>
        )}
      </div>

      {/* Info */}
      <div className="min-w-0 flex-1">
        <div className="flex items-center justify-between mb-0.5">
          <span className="text-sm font-medium text-text-primary truncate">
            {contact.screen_name}
            {contact.is_group && (
              <span className="ml-1 text-xs text-green-400">[群]</span>
            )}
          </span>
          <span className="text-xs text-text-secondary flex-shrink-0 ml-1">
            {contact.last_time}
          </span>
        </div>
        <p className="text-xs text-text-secondary truncate">
          {contact.last_message || "暂无消息"}
        </p>
      </div>
    </div>
  );
}
