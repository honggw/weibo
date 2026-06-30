// ============================================================================
// EmojiPanel — emoji picker grid
// ============================================================================

import { useEffect, useState } from "react";
import type { Emotion } from "../types";
import { useAppStore } from "../hooks/useAppState";

// Fallback emoji list (when backend hasn't loaded emotions yet)
const FALLBACK_EMOJIS: Emotion[] = [
  { phrase: "[笑cry]", url: "" },
  { phrase: "[憧憬]", url: "" },
  { phrase: "[可爱]", url: "" },
  { phrase: "[并不简单]", url: "" },
  { phrase: "[good]", url: "" },
  { phrase: "[赞]", url: "" },
  { phrase: "[心]", url: "" },
  { phrase: "[鲜花]", url: "" },
  { phrase: "[抱抱]", url: "" },
  { phrase: "[加油]", url: "" },
  { phrase: "[doge]", url: "" },
  { phrase: "[喵喵]", url: "" },
  { phrase: "[二哈]", url: "" },
  { phrase: "[费解]", url: "" },
  { phrase: "[挖鼻]", url: "" },
  { phrase: "[吃惊]", url: "" },
  { phrase: "[允悲]", url: "" },
  { phrase: "[跪了]", url: "" },
  { phrase: "[摊手]", url: "" },
  { phrase: "[思考]", url: "" },
];

interface Props {
  onSelect: (phrase: string) => void;
}

export default function EmojiPanel({ onSelect }: Props) {
  const emotions = useAppStore((s) => s.emotions);
  const [list, setList] = useState<Emotion[]>(FALLBACK_EMOJIS);

  useEffect(() => {
    if (emotions.length > 0) {
      setList(emotions);
    }
  }, [emotions]);

  return (
    <div className="border-b border-white/5 bg-card/50 p-3">
      <div className="grid grid-cols-8 gap-1 max-h-32 overflow-y-auto">
        {list.map((em) => (
          <button
            key={em.phrase}
            onClick={() => onSelect(em.phrase)}
            className="px-1.5 py-1 text-xs text-text-primary hover:bg-accent/20 rounded transition-colors truncate"
            title={em.phrase}
          >
            {em.phrase}
          </button>
        ))}
      </div>
    </div>
  );
}
