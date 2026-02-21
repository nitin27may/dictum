import { AnimatePresence, motion } from "framer-motion";
import { useEffect } from "react";
import { useAudioLevels } from "../../hooks/useAudioLevels";
import { useRecordingFlow } from "../../hooks/useRecordingFlow";
import { useRecordingStore } from "../../store/recordingStore";
import { ErrorIndicator } from "./ErrorIndicator";
import { ProcessingIndicator } from "./ProcessingIndicator";
import { SuccessFlash } from "./SuccessFlash";
import { WaveformVisualizer } from "./WaveformVisualizer";

export function Overlay() {
  const { phase } = useRecordingFlow();
  const { audioLevel, durationSecs, error, processingLabel } = useRecordingStore();
  const reset = useRecordingStore((s) => s.reset);

  useAudioLevels(phase === "RECORDING");

  // Auto-dismiss: reset to IDLE after success or error.
  // The overlay window stays always-on (transparent + click-through);
  // only the pill animates in/out via AnimatePresence.
  useEffect(() => {
    if (phase === "SUCCESS") {
      const t = setTimeout(reset, 900);
      return () => clearTimeout(t);
    }
    if (phase === "ERROR") {
      const t = setTimeout(reset, 1800);
      return () => clearTimeout(t);
    }
  }, [phase, reset]);

  const isVisible = phase !== "IDLE";

  return (
    // Full overlay area — transparent, click-through
    <div className="flex items-end justify-center w-full h-full pb-3 pointer-events-none">
      <AnimatePresence>
        {isVisible && (
          <motion.div
            key="pill"
            initial={{ opacity: 0, y: 12, scale: 0.94 }}
            animate={{ opacity: 1, y: 0, scale: 1 }}
            exit={{ opacity: 0, y: 6, scale: 0.97 }}
            transition={{ type: "spring", stiffness: 420, damping: 32 }}
            className="flex items-center h-[52px] px-5 rounded-full"
            style={{
              background: "rgba(14, 14, 16, 0.93)",
              border: "1px solid rgba(255,255,255,0.09)",
              boxShadow: "0 8px 40px rgba(0,0,0,0.55), 0 1px 0 rgba(255,255,255,0.04) inset",
              backdropFilter: "blur(20px)",
              WebkitBackdropFilter: "blur(20px)",
              minWidth: 160,
              maxWidth: 380,
            }}
          >
            <AnimatePresence mode="wait">
              {phase === "RECORDING" && (
                <motion.div
                  key="recording"
                  className="flex items-center gap-3"
                  initial={{ opacity: 0 }}
                  animate={{ opacity: 1 }}
                  exit={{ opacity: 0 }}
                  transition={{ duration: 0.12 }}
                >
                  {/* Pulsing red recording dot */}
                  <motion.div
                    className="w-[7px] h-[7px] rounded-full bg-red-500 flex-shrink-0"
                    animate={{ opacity: [1, 0.35, 1] }}
                    transition={{ duration: 1.1, repeat: Infinity, ease: "easeInOut" }}
                  />

                  {/* Waveform driven by mic audio level */}
                  <WaveformVisualizer level={audioLevel} />

                  {/* Duration — only show after 1s so it doesn't flash on short taps */}
                  {durationSecs > 0 && (
                    <motion.span
                      className="text-white/40 text-[11px] font-mono tabular-nums ml-1"
                      initial={{ opacity: 0 }}
                      animate={{ opacity: 1 }}
                      transition={{ duration: 0.2 }}
                      style={{ letterSpacing: "0.02em" }}
                    >
                      {formatDuration(durationSecs)}
                    </motion.span>
                  )}
                </motion.div>
              )}

              {phase === "PROCESSING" && (
                <motion.div
                  key="processing"
                  initial={{ opacity: 0 }}
                  animate={{ opacity: 1 }}
                  exit={{ opacity: 0 }}
                  transition={{ duration: 0.12 }}
                >
                  <ProcessingIndicator label={processingLabel} />
                </motion.div>
              )}

              {phase === "SUCCESS" && (
                <motion.div
                  key="success"
                  initial={{ opacity: 0 }}
                  animate={{ opacity: 1 }}
                  exit={{ opacity: 0 }}
                  transition={{ duration: 0.12 }}
                >
                  <SuccessFlash />
                </motion.div>
              )}

              {phase === "ERROR" && (
                <motion.div
                  key="error"
                  initial={{ opacity: 0 }}
                  animate={{ opacity: 1 }}
                  exit={{ opacity: 0 }}
                  transition={{ duration: 0.12 }}
                >
                  <ErrorIndicator message={error ?? "Something went wrong"} />
                </motion.div>
              )}
            </AnimatePresence>
          </motion.div>
        )}
      </AnimatePresence>
    </div>
  );
}

function formatDuration(secs: number): string {
  const m = Math.floor(secs / 60);
  const s = secs % 60;
  return m > 0 ? `${m}:${String(s).padStart(2, "0")}` : `0:${String(s).padStart(2, "0")}`;
}
