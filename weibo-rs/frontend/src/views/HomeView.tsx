// ============================================================================
// HomeView — main app shell with TabBar, Timeline (Home tab), Chat (Chat tab)
// ============================================================================

import { useState } from "react";
import { useAppStore } from "../hooks/useAppState";
import TabBar from "../components/TabBar";
import TimelineList from "../components/TimelineList";
import ChatView from "./ChatView";

type Tab = "home" | "chat";

export default function HomeView() {
  const dmUnread = useAppStore((s) => s.dmUnread);
  const [tab, setTab] = useState<Tab>("home");

  return (
    <div className="h-full flex flex-col bg-bg">
      {/* Tab Bar */}
      <TabBar activeTab={tab} onTabChange={setTab} dmUnread={dmUnread} />

      {/* Content */}
      <div className="flex-1 min-h-0">
        {tab === "home" ? <TimelineList /> : <ChatView />}
      </div>
    </div>
  );
}
