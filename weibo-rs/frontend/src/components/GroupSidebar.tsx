// ============================================================================
// GroupSidebar — overlay panel showing group members
// ============================================================================

import { useAppStore } from "../hooks/useAppState";

interface Props {
  onClose: () => void;
}

export default function GroupSidebar({ onClose }: Props) {
  const groupInfo = useAppStore((s) => s.groupInfo);

  if (!groupInfo) {
    return (
      <div className="absolute right-0 top-0 bottom-0 w-64 bg-bg border-l border-white/10 shadow-2xl z-10 flex items-center justify-center">
        <div className="animate-spin w-5 h-5 border-2 border-accent border-t-transparent rounded-full" />
      </div>
    );
  }

  return (
    <div className="absolute right-0 top-0 bottom-0 w-64 bg-bg border-l border-white/10 shadow-2xl z-10 flex flex-col">
      {/* Header */}
      <div className="flex items-center justify-between px-4 py-3 border-b border-white/5">
        <div>
          <h3 className="text-sm font-semibold text-text-primary">
            {groupInfo.name}
          </h3>
          <p className="text-xs text-text-secondary">
            {groupInfo.member_count} 名成员
          </p>
        </div>
        <button
          onClick={onClose}
          className="text-text-secondary hover:text-white text-lg leading-none"
        >
          ×
        </button>
      </div>

      {/* Member list */}
      <div className="flex-1 overflow-y-auto">
        {groupInfo.members.map((member) => (
          <div
            key={member.uid}
            className="flex items-center gap-3 px-4 py-2.5 border-b border-white/5 hover:bg-white/[0.02]"
          >
            <div className="w-8 h-8 rounded-full bg-accent/30 flex items-center justify-center text-accent text-xs font-bold flex-shrink-0">
              {(member.screen_name || member.uid).charAt(0)}
            </div>
            <div className="min-w-0 flex-1">
              <div className="flex items-center gap-1.5">
                <span className="text-sm text-text-primary truncate">
                  {member.screen_name || member.uid}
                </span>
                {member.is_admin && (
                  <span className="text-[10px] text-accent bg-accent/10 px-1 rounded">
                    管理
                  </span>
                )}
              </div>
            </div>
          </div>
        ))}
      </div>
    </div>
  );
}
