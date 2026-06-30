// ============================================================================
// TimelineList — virtual-scrolled timeline with infinite load-more
// ============================================================================

import { useCallback, useRef } from "react";
import { Virtuoso, VirtuosoHandle } from "react-virtuoso";
import { useAppStore } from "../hooks/useAppState";
import { loadMoreTimeline } from "../tauri";

export default function TimelineList() {
  const items = useAppStore((s) => s.timelineItems);
  const title = useAppStore((s) => s.timelineTitle);
  const hasMore = useAppStore((s) => s.hasMoreTimeline);
  const syncFromBackend = useAppStore((s) => s.syncFromBackend);

  const virtuosoRef = useRef<VirtuosoHandle>(null);
  const loadingRef = useRef(false);

  // Load more when scrolling near bottom
  const handleEndReached = useCallback(async () => {
    if (loadingRef.current || !hasMore) return;
    loadingRef.current = true;
    try {
      await loadMoreTimeline();
      await syncFromBackend();
    } catch (e) {
      console.error("loadMoreTimeline failed:", e);
    } finally {
      loadingRef.current = false;
    }
  }, [hasMore, syncFromBackend]);

  if (items.length === 0) {
    return (
      <div className="h-full flex items-center justify-center text-text-secondary">
        <div className="flex flex-col items-center gap-4">
          <div className="animate-spin w-8 h-8 border-3 border-accent border-t-transparent rounded-full" />
          <p>加载时间线...</p>
        </div>
      </div>
    );
  }

  return (
    <div className="h-full flex flex-col">
      {/* Title */}
      {title && (
        <div className="px-4 py-3 text-sm text-text-secondary border-b border-white/5 bg-header-bg/50">
          {title}
        </div>
      )}

      {/* Timeline */}
      <div className="flex-1">
        <Virtuoso
          ref={virtuosoRef}
          data={items}
          endReached={handleEndReached}
          itemContent={(index, item) => (
            <div className="px-4 py-3 border-b border-white/5 hover:bg-white/[0.02] transition-colors">
              <div className="flex items-start gap-3">
                {/* Avatar placeholder */}
                <div className="w-10 h-10 rounded-full bg-accent/30 flex items-center justify-center text-accent text-sm font-bold flex-shrink-0">
                  {item.user_name.charAt(0)}
                </div>
                <div className="min-w-0 flex-1">
                  <div className="flex items-baseline gap-2 mb-1">
                    <span className="font-semibold text-text-primary text-sm">
                      {item.user_name}
                    </span>
                  </div>
                  <p className="text-text-primary text-sm leading-relaxed whitespace-pre-wrap break-words">
                    {item.text}
                  </p>
                </div>
              </div>
            </div>
          )}
          components={{
            Footer: () =>
              hasMore ? (
                <div className="flex items-center justify-center py-4">
                  <div className="animate-spin w-5 h-5 border-2 border-accent border-t-transparent rounded-full" />
                </div>
              ) : (
                <div className="text-center py-4 text-text-secondary text-sm">
                  — 已经到底了 —
                </div>
              ),
          }}
        />
      </div>
    </div>
  );
}
