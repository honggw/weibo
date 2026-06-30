// ============================================================================
// QrDisplay — fetches QR code PNG from backend and renders it
// ============================================================================

import { useState, useEffect, useRef } from "react";
import { getQrImage } from "../tauri";
import { useAppStore } from "../hooks/useAppState";

export default function QrDisplay() {
  const [qrB64, setQrB64] = useState<string | null>(null);
  const [status, setStatus] = useState<string>("");
  const phase = useAppStore((s) => s.phase);
  const pollRef = useRef<ReturnType<typeof setInterval> | null>(null);

  useEffect(() => {
    let cancelled = false;

    async function load() {
      try {
        const resp = await getQrImage();
        if (cancelled) return;
        setQrB64(resp.qr_base64);
        setStatus(resp.status);
      } catch {
        // retry on next poll
      }
    }

    // Initial load
    load();

    // Poll for QR updates while in waiting_scan phase
    if (phase === "waiting_scan") {
      pollRef.current = setInterval(load, 1000);
    }

    return () => {
      cancelled = true;
      if (pollRef.current) {
        clearInterval(pollRef.current);
        pollRef.current = null;
      }
    };
  }, [phase]);

  return (
    <div className="w-56 h-56 border-4 border-qr-border rounded-2xl flex items-center justify-center bg-card overflow-hidden">
      {qrB64 ? (
        <img
          src={`data:image/png;base64,${qrB64}`}
          alt="QR Code"
          className="w-52 h-52 rounded-xl object-contain"
        />
      ) : (
        <div className="flex flex-col items-center gap-3 text-text-secondary">
          <svg
            className="w-16 h-16 animate-pulse"
            fill="none"
            viewBox="0 0 24 24"
            stroke="currentColor"
          >
            <path
              strokeLinecap="round"
              strokeLinejoin="round"
              strokeWidth={1.5}
              d="M12 4v1m6 11h2m-6 0h-2m0 0H8m4 0v-4m0 0V8m0 0V6m-4 6h2m6 0h2M6 6h.01M18 18h.01"
            />
          </svg>
          <span className="text-sm">{status || "加载二维码中..."}</span>
        </div>
      )}
    </div>
  );
}
