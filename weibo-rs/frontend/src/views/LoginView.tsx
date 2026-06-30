// ============================================================================
// LoginView — QR code login flow UI
// ============================================================================

import { useEffect, useRef, useCallback } from "react";
import { useAppStore } from "../hooks/useAppState";
import { checkSavedCookie, startQrLogin, getState } from "../tauri";
import QrDisplay from "../components/QrDisplay";

export default function LoginView() {
  const phase = useAppStore((s) => s.phase);
  const phaseData = useAppStore((s) => s.phaseData);
  const syncFromBackend = useAppStore((s) => s.syncFromBackend);
  const qrPolling = useAppStore((s) => s.qrPolling);
  const setQrPolling = useAppStore((s) => s.setQrPolling);

  const pollRef = useRef<ReturnType<typeof setInterval> | null>(null);

  // On mount: try cookie, fallback to QR
  const startedRef = useRef(false);
  useEffect(() => {
    if (startedRef.current) return;
    startedRef.current = true;
    checkSavedCookie().catch(() => {
      // If cookie check fails, start QR login
      startQrLogin().catch(console.error);
    });
  }, []);

  // QR polling: when in waiting_scan, poll every second
  const startPolling = useCallback(() => {
    if (pollRef.current) return;
    setQrPolling(true);
    pollRef.current = setInterval(async () => {
      try {
        await syncFromBackend();
      } catch {
        // ignore
      }
    }, 1000);
  }, [syncFromBackend, setQrPolling]);

  const stopPolling = useCallback(() => {
    if (pollRef.current) {
      clearInterval(pollRef.current);
      pollRef.current = null;
    }
    setQrPolling(false);
  }, [setQrPolling]);

  useEffect(() => {
    if (phase === "waiting_scan") {
      startPolling();
    } else {
      stopPolling();
    }
    return () => stopPolling();
  }, [phase, startPolling, stopPolling]);

  // Handle start QR login button
  const handleStartQr = async () => {
    try {
      await startQrLogin();
      await syncFromBackend();
    } catch (e) {
      console.error(e);
    }
  };

  // Render based on phase
  const renderContent = () => {
    switch (phase) {
      case "checking_cookie":
        return (
          <div className="flex flex-col items-center gap-4">
            <div className="animate-spin w-10 h-10 border-4 border-accent border-t-transparent rounded-full" />
            <p className="text-text-secondary text-lg">
              {phaseData.status || "检查已保存的登录状态..."}
            </p>
          </div>
        );

      case "loading":
        return (
          <div className="flex flex-col items-center gap-4">
            <div className="animate-spin w-10 h-10 border-4 border-accent border-t-transparent rounded-full" />
            <p className="text-text-secondary text-lg">
              {phaseData.status || "加载中..."}
            </p>
          </div>
        );

      case "waiting_scan":
        return (
          <div className="flex flex-col items-center gap-6">
            <QrDisplay />
            <p className="text-text-primary text-lg text-center">
              {phaseData.status || "📱 请用微博手机客户端扫描二维码"}
            </p>
            {qrPolling && (
              <p className="text-text-secondary text-sm flex items-center gap-2">
                <span className="inline-block w-2 h-2 bg-green-400 rounded-full animate-pulse" />
                等待扫码中...
              </p>
            )}
          </div>
        );

      case "exchanging":
        return (
          <div className="flex flex-col items-center gap-4">
            <div className="animate-spin w-10 h-10 border-4 border-green-400 border-t-transparent rounded-full" />
            <p className="text-green-400 text-lg">
              {phaseData.status || "✅ 确认成功！获取票据..."}
            </p>
          </div>
        );

      case "fetching_home":
        return (
          <div className="flex flex-col items-center gap-4">
            <div className="animate-spin w-10 h-10 border-4 border-accent border-t-transparent rounded-full" />
            <p className="text-text-secondary text-lg">加载首页...</p>
          </div>
        );

      case "error":
        return (
          <div className="flex flex-col items-center gap-6">
            <div className="text-red-400 text-6xl">⚠️</div>
            <p className="text-red-400 text-lg text-center max-w-md">
              {phaseData.status || "发生未知错误"}
            </p>
            <button
              onClick={handleStartQr}
              className="px-6 py-3 bg-accent text-white rounded-lg hover:bg-orange-600 transition-colors"
            >
              重试扫码登录
            </button>
          </div>
        );

      default:
        return (
          <div className="flex flex-col items-center gap-6">
            <button
              onClick={handleStartQr}
              className="px-8 py-4 bg-accent text-white rounded-xl text-lg hover:bg-orange-600 transition-colors shadow-lg"
            >
              扫码登录
            </button>
          </div>
        );
    }
  };

  return (
    <div className="h-full flex flex-col items-center justify-center bg-bg p-8">
      {/* Logo / Title */}
      <div className="mb-10 text-center">
        <h1 className="text-4xl font-bold text-accent mb-2">微博</h1>
        <p className="text-text-secondary text-sm">PC 客户端</p>
      </div>

      {renderContent()}
    </div>
  );
}
