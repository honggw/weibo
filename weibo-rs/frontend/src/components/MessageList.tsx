// ============================================================================
// MessageList — virtual-scrolled message list with load-older on scroll to top
// ============================================================================

import { useRef, useCallback, useMemo } from "react";
import { Virtuoso, VirtuosoHandle } from "react-virtuoso";
import { useAppStore } from "../hooks/useAppState";
import type { ChatMessage } from "../types";

interface Props {
  onLoadOlder: () => void;
  hasMore: boolean;
}

export default function MessageList({ onLoadOlder, hasMore }: Props) {
  const messages = useAppStore((s) => s.messages);
  const messagesLoading = useAppStore((s) => s.messagesLoading);
  const myUid = useAppStore((s) => s.myUid);

  const virtuosoRef = useRef<VirtuosoHandle>(null);
  const loadingRef = useRef(false);
  const initialScrollDone = useRef(false);

  // Build list items with time separators
  const listItems = useMemo(() => buildListItems(messages), [messages]);

  // Scroll to bottom on initial load
  const handleItemsRendered = useCallback(
    (renderedItems: unknown[]) => {
      if (!initialScrollDone.current && renderedItems.length > 0) {
        initialScrollDone.current = true;
        setTimeout(() => {
          virtuosoRef.current?.scrollToIndex({
            index: listItems.length - 1,
            align: "end",
          });
        }, 100);
      }
    },
    [listItems.length]
  );

  // Load older when scrolling to top
  const handleStartReached = useCallback(() => {
    if (loadingRef.current || !hasMore) return;
    loadingRef.current = true;
    onLoadOlder();
    setTimeout(() => {
      loadingRef.current = false;
    }, 1000);
  }, [onLoadOlder, hasMore]);

  if (messagesLoading) {
    return (
      <div className="h-full flex items-center justify-center">
        <div className="animate-spin w-6 h-6 border-2 border-accent border-t-transparent rounded-full" />
      </div>
    );
  }

  if (messages.length === 0) {
    return (
      <div className="h-full flex items-center justify-center text-text-secondary text-sm">
        <p>暂无消息</p>
      </div>
    );
  }

  return (
    <Virtuoso
      ref={virtuosoRef}
      data={listItems}
      startReached={handleStartReached}
      itemsRendered={handleItemsRendered}
      initialTopMostItemIndex={listItems.length - 1}
      itemContent={(_, item) => {
        if (item.type === "separator") {
          return (
            <div className="flex items-center justify-center py-3">
              <span className="text-xs text-text-secondary bg-card/80 px-3 py-0.5 rounded-full">
                {item.label}
              </span>
            </div>
          );
        }
        const msg = item.message!;
        const isSelf = msg.sender_id === myUid;
        return <MessageBubble msg={msg} isSelf={isSelf} />;
      }}
      components={{
        Header: () =>
          hasMore ? (
            <div className="flex items-center justify-center py-3">
              <div className="animate-spin w-4 h-4 border-2 border-accent border-t-transparent rounded-full" />
            </div>
          ) : (
            <div className="text-center py-3 text-text-secondary text-xs">
              — 没有更早的消息 —
            </div>
          ),
      }}
    />
  );
}

// ---------------------------------------------------------------------------
// MessageBubble
// ---------------------------------------------------------------------------

function MessageBubble({ msg, isSelf }: { msg: ChatMessage; isSelf: boolean }) {
  const isSystem = msg.msg_type === "System";
  const isRecall = msg.msg_type === "Recall";

  if (isSystem) {
    return (
      <div className="flex justify-center py-2">
        <span className="text-xs text-text-secondary bg-white/5 px-3 py-1 rounded-full">
          {msg.text}
        </span>
      </div>
    );
  }

  if (isRecall) {
    return (
      <div className="flex justify-center py-2">
        <span className="text-xs text-text-secondary italic">
          {msg.sender_name} 撤回了一条消息
        </span>
      </div>
    );
  }

  return (
    <div
      className={`flex gap-3 px-4 py-2 ${isSelf ? "flex-row-reverse" : ""}`}
    >
      <div className="w-8 h-8 rounded-full bg-accent/30 flex items-center justify-center text-accent text-xs font-bold flex-shrink-0">
        {msg.sender_name.charAt(0)}
      </div>

      <div className={`max-w-[70%] ${isSelf ? "items-end" : "items-start"}`}>
        {!isSelf && (
          <div className="text-xs text-text-secondary mb-1">
            {msg.sender_name}
          </div>
        )}
        <div
          className={`px-3 py-2 rounded-2xl text-sm leading-relaxed whitespace-pre-wrap break-words ${
            isSelf
              ? "bg-accent text-white rounded-br-md"
              : "bg-card text-text-primary rounded-bl-md"
          }`}
        >
          {msg.text}
        </div>
        <div
          className={`text-[10px] text-text-secondary mt-0.5 ${
            isSelf ? "text-right" : "text-left"
          }`}
        >
          {msg.created_at || formatTime(msg.timestamp)}
        </div>
      </div>
    </div>
  );
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

interface ListItem {
  type: "separator" | "message";
  label?: string;
  message?: ChatMessage;
}

function buildListItems(msgs: ChatMessage[]): ListItem[] {
  const result: ListItem[] = [];
  for (let i = 0; i < msgs.length; i++) {
    const msg = msgs[i];
    const needsSeparator =
      i === 0 ||
      (msg.timestamp > 0 &&
        msgs[i - 1].timestamp > 0 &&
        msg.timestamp - msgs[i - 1].timestamp > 300);
    if (needsSeparator && msg.timestamp > 0) {
      result.push({
        type: "separator",
        label: formatSeparatorTime(msg.timestamp),
      });
    }
    result.push({ type: "message", message: msg });
  }
  return result;
}

function formatTime(ts: number): string {
  if (!ts) return "";
  const d = new Date(ts * 1000);
  return d.toLocaleTimeString("zh-CN", {
    hour: "2-digit",
    minute: "2-digit",
  });
}

function formatSeparatorTime(ts: number): string {
  const d = new Date(ts * 1000);
  const now = new Date();
  const isToday =
    d.getDate() === now.getDate() &&
    d.getMonth() === now.getMonth() &&
    d.getFullYear() === now.getFullYear();
  const yesterday = new Date(now.getTime() - 86400000);

  if (isToday) {
    return d.toLocaleTimeString("zh-CN", {
      hour: "2-digit",
      minute: "2-digit",
    });
  } else if (
    d.getDate() === yesterday.getDate() &&
    d.getMonth() === yesterday.getMonth()
  ) {
    return `昨天 ${d.toLocaleTimeString("zh-CN", { hour: "2-digit", minute: "2-digit" })}`;
  } else {
    return (
      d.toLocaleDateString("zh-CN", {
        month: "2-digit",
        day: "2-digit",
      }) +
      ` ${d.toLocaleTimeString("zh-CN", { hour: "2-digit", minute: "2-digit" })}`
    );
  }
}
