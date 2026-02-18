import { useEffect, useRef } from "react";
import { onAudioLevel } from "../services/tauri-bridge";
import { useRecordingStore } from "../store/recordingStore";

/**
 * Listens to Tauri audio-level events and updates the store via
 * requestAnimationFrame to keep UI updates in sync with the render cycle.
 */
export function useAudioLevels(active: boolean) {
  const setAudioLevel = useRecordingStore((s) => s.setAudioLevel);
  const pendingLevel = useRef<number | null>(null);
  const rafHandle = useRef<number | null>(null);
  const unlistenRef = useRef<(() => void) | null>(null);

  useEffect(() => {
    if (!active) {
      // Reset level when not recording
      setAudioLevel(0);
      return;
    }

    let mounted = true;

    const startListening = async () => {
      const unlisten = await onAudioLevel((level) => {
        // Normalize: typical RMS for speech is 0.01–0.3; scale to 0–1
        const normalized = Math.min(1, level * 5);
        pendingLevel.current = normalized;

        if (rafHandle.current === null) {
          rafHandle.current = requestAnimationFrame(() => {
            rafHandle.current = null;
            if (pendingLevel.current !== null && mounted) {
              setAudioLevel(pendingLevel.current);
              pendingLevel.current = null;
            }
          });
        }
      });

      if (mounted) {
        unlistenRef.current = unlisten;
      } else {
        unlisten();
      }
    };

    startListening();

    return () => {
      mounted = false;
      if (rafHandle.current !== null) {
        cancelAnimationFrame(rafHandle.current);
        rafHandle.current = null;
      }
      if (unlistenRef.current) {
        unlistenRef.current();
        unlistenRef.current = null;
      }
      setAudioLevel(0);
    };
  }, [active, setAudioLevel]);
}
