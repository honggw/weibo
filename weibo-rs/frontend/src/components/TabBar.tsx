// ============================================================================
// TabBar — Home / Chat tab switcher with unread badge
// ============================================================================

import { logout as tauriLogout } from "../tauri";

interface Props {
  activeTab: "home" | "chat";
  onTabChange: (tab: "home" | "chat") => void;
  dmUnread: number;
}

export default function TabBar({ activeTab, onTabChange, dmUnread }: Props) {
  const handleLogout = async () => {
    try {
      await tauriLogout();
    } catch (e) {
      console.error("Logout failed:", e);
    }
  };

  return (
    <div className="flex items-center justify-between bg-header-bg px-4 py-2 select-none">
      {/* Tabs */}
      <div className="flex gap-1">
        <TabButton
          active={activeTab === "home"}
          onClick={() => onTabChange("home")}
          label="首页"
          icon={
            <svg className="w-5 h-5" fill="none" viewBox="0 0 24 24" stroke="currentColor">
              <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2}
                d="M3 12l2-2m0 0l7-7 7 7M5 10v10a1 1 0 001 1h3m10-11l2 2m-2-2v10a1 1 0 01-1 1h-3m-6 0a1 1 0 001-1v-4a1 1 0 011-1h2a1 1 0 011 1v4a1 1 0 001 1m-6 0h6" />
            </svg>
          }
        />
        <TabButton
          active={activeTab === "chat"}
          onClick={() => onTabChange("chat")}
          label="聊天"
          badge={dmUnread > 0 ? dmUnread : undefined}
          icon={
            <svg className="w-5 h-5" fill="none" viewBox="0 0 24 24" stroke="currentColor">
              <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2}
                d="M8 12h.01M12 12h.01M16 12h.01M21 12c0 4.418-4.03 8-9 8a9.863 9.863 0 01-4.255-.949L3 20l1.395-3.72C3.512 15.042 3 13.574 3 12c0-4.418 4.03-8 9-8s9 3.582 9 8z" />
            </svg>
          }
        />
      </div>

      {/* Logout */}
      <button
        onClick={handleLogout}
        className="px-3 py-1.5 text-sm text-text-secondary hover:text-red-400 hover:bg-logout-btn rounded-lg transition-colors"
        title="登出"
      >
        登出
      </button>
    </div>
  );
}

// ---------------------------------------------------------------------------
// TabButton sub-component
// ---------------------------------------------------------------------------

function TabButton({
  active,
  onClick,
  label,
  icon,
  badge,
}: {
  active: boolean;
  onClick: () => void;
  label: string;
  icon: React.ReactNode;
  badge?: number;
}) {
  return (
    <button
      onClick={onClick}
      className={`relative flex items-center gap-2 px-4 py-2 rounded-lg transition-colors ${
        active
          ? "bg-accent text-white"
          : "text-text-secondary hover:text-white hover:bg-white/10"
      }`}
    >
      {icon}
      <span className="text-sm font-medium">{label}</span>
      {badge !== undefined && badge > 0 && (
        <span className="absolute -top-1 -right-1 min-w-[18px] h-[18px] flex items-center justify-center bg-red-500 text-white text-xs font-bold rounded-full px-1">
          {badge > 99 ? "99+" : badge}
        </span>
      )}
    </button>
  );
}
