/**
 * useRecordingFlow — Overlay UI state driven by Rust events.
 *
 * Rust owns the full hot path (record → transcribe → inject).
 * This hook just updates the Zustand store based on state events
 * emitted by Rust, driving the overlay animations.
 */

import { getCurrentWindow } from "@tauri-apps/api/window";
import { listen } from "@tauri-apps/api/event";
import { useEffect } from "react";
import { useRecordingStore } from "../store/recordingStore";

export function useRecordingFlow() {
  const store = useRecordingStore();

  useEffect(() => {
    let mounted = true;
    const unlisteners: Array<() => void> = [];

    const setup = async () => {
      const win = getCurrentWindow();

      unlisteners.push(
        await win.listen("recording-started", () => {
          if (!mounted) return;
          store.startRecording();
        })
      );

      unlisteners.push(
        await win.listen<number>("recording-tick", (e) => {
          if (!mounted) return;
          store.setDuration(e.payload);
        })
      );

      unlisteners.push(
        await win.listen("processing-started", () => {
          if (!mounted) return;
          store.setProcessing();
        })
      );

      unlisteners.push(
        await win.listen<number>("recording-success", (e) => {
          if (!mounted) return;
          store.setSuccess(`${"x".repeat(e.payload)}`); // char count placeholder
          // hide is handled by Rust after delay
        })
      );

      unlisteners.push(
        await win.listen<string>("recording-error", (e) => {
          if (!mounted) return;
          store.setError(e.payload);
          // hide + reset handled by Rust after delay
        })
      );

      // Auto-reset store when overlay window is hidden
      unlisteners.push(
        await win.listen("tauri://blur", () => {
          if (!mounted) return;
          setTimeout(() => {
            if (mounted) store.reset();
          }, 100);
        })
      );
    };

    setup().catch((e) => console.error("useRecordingFlow setup failed:", e));

    return () => {
      mounted = false;
      unlisteners.forEach((fn) => fn());
    };
  }, []);

  return { phase: store.phase };
}
