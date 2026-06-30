// ============================================================================
// App root — phase-driven screen routing
// ============================================================================

import { useEffect } from "react";
import { useAppStore } from "./hooks/useAppState";
import { onStateChanged } from "./tauri";
import LoginView from "./views/LoginView";
import HomeView from "./views/HomeView";

export default function App() {
  const phase = useAppStore((s) => s.phase);
  const syncFromBackend = useAppStore((s) => s.syncFromBackend);

  // Initial sync + listen for state changes
  useEffect(() => {
    syncFromBackend();
    const unlisten = onStateChanged(() => {
      syncFromBackend();
    });
    return unlisten;
  }, [syncFromBackend]);

  // Phase-based routing
  switch (phase) {
    case "home_loaded":
      return <HomeView />;

    case "checking_cookie":
    case "loading":
    case "waiting_scan":
    case "scanned":
    case "exchanging":
    case "fetching_home":
    case "error":
    default:
      return <LoginView />;
  }
}
