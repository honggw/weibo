// ============================================================================
// MessageInput — text input + send button + emoji toggle
// ============================================================================

import { useRef, useCallback, KeyboardEvent } from "react";

interface Props {
  draftText: string;
  onDraftChange: (v: string) => void;
  onSend: (text: string) => void;
  onToggleEmoji: () => void;
  showEmoji: boolean;
}

export default function MessageInput({
  draftText,
  onDraftChange,
  onSend,
  onToggleEmoji,
  showEmoji,
}: Props) {
  const inputRef = useRef<HTMLTextAreaElement>(null);

  const handleSend = useCallback(() => {
    if (!draftText.trim()) return;
    onSend(draftText);
    if (inputRef.current) {
      inputRef.current.value = "";
      inputRef.current.style.height = "auto";
    }
  }, [draftText, onSend]);

  const handleKeyDown = useCallback(
    (e: KeyboardEvent<HTMLTextAreaElement>) => {
      if (e.key === "Enter" && !e.shiftKey) {
        e.preventDefault();
        handleSend();
      }
    },
    [handleSend]
  );

  // Auto-resize textarea
  const handleInput = useCallback(
    (e: React.ChangeEvent<HTMLTextAreaElement>) => {
      onDraftChange(e.target.value);
      const el = e.target;
      el.style.height = "auto";
      el.style.height = Math.min(el.scrollHeight, 120) + "px";
    },
    [onDraftChange]
  );

  return (
    <div className="flex items-end gap-2 p-3 bg-card/30">
      {/* Emoji toggle */}
      <button
        onClick={onToggleEmoji}
        className={`p-2 rounded-lg transition-colors ${
          showEmoji
            ? "bg-accent/30 text-accent"
            : "text-text-secondary hover:text-white hover:bg-white/10"
        }`}
        title="表情"
      >
        <svg className="w-5 h-5" fill="none" viewBox="0 0 24 24" stroke="currentColor">
          <path
            strokeLinecap="round"
            strokeLinejoin="round"
            strokeWidth={2}
            d="M14.828 14.828a4 4 0 01-5.656 0M9 10h.01M15 10h.01M21 12a9 9 0 11-18 0 9 9 0 0118 0z"
          />
        </svg>
      </button>

      {/* Input */}
      <textarea
        ref={inputRef}
        value={draftText}
        onChange={handleInput}
        onKeyDown={handleKeyDown}
        placeholder="输入消息..."
        rows={1}
        className="flex-1 px-3 py-2 bg-bg border border-white/10 rounded-lg text-sm text-text-primary placeholder-text-secondary resize-none focus:outline-none focus:border-accent/50"
      />

      {/* Send button */}
      <button
        onClick={handleSend}
        disabled={!draftText.trim()}
        className={`px-4 py-2 rounded-lg text-sm font-medium transition-colors ${
          draftText.trim()
            ? "bg-accent text-white hover:bg-orange-600"
            : "bg-white/5 text-text-secondary cursor-not-allowed"
        }`}
      >
        发送
      </button>
    </div>
  );
}
