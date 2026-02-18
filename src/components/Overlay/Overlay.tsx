import { AnimatePresence, motion } from "framer-motion";
import { useAudioLevels } from "../../hooks/useAudioLevels";
import { useRecordingFlow } from "../../hooks/useRecordingFlow";
import { useRecordingStore } from "../../store/recordingStore";
import { ErrorIndicator } from "./ErrorIndicator";
import { ProcessingIndicator } from "./ProcessingIndicator";
import { SuccessFlash } from "./SuccessFlash";
import { WaveformVisualizer } from "./WaveformVisualizer";

export function Overlay() {
  const { phase } = useRecordingFlow();
  const { audioLevel, durationSecs, charCount, error } = useRecordingStore();

  useAudioLevels(phase === "RECORDING");

  const isVisible = phase !== "IDLE";

  return (
    <div className="flex items-end justify-center w-full h-full p-3 pointer-events-none">
      <AnimatePresence>
        {isVisible && (
          <motion.div
            key="pill"
            initial={{ opacity: 0, y: 16, scale: 0.95 }}
            animate={{ opacity: 1, y: 0, scale: 1 }}
            exit={{ opacity: 0, y: 8, scale: 0.97 }}
            transition={{ type: "spring", stiffness: 400, damping: 30 }}
            className="flex items-center px-5 h-[58px] rounded-full bg-zinc-900/92 backdrop-blur-md border border-zinc-700/50 shadow-2xl"
            style={{ minWidth: 200, maxWidth: 400 }}
          >
            <AnimatePresence mode="wait">
              {phase === "RECORDING" && (
                <motion.div
                  key="recording"
                  className="flex items-center gap-4"
                  initial={{ opacity: 0 }}
                  animate={{ opacity: 1 }}
                  exit={{ opacity: 0 }}
                  transition={{ duration: 0.15 }}
                >
                  {/* Mic indicator dot */}
                  <motion.div
                    className="w-2 h-2 rounded-full bg-red-500 flex-shrink-0"
                    animate={{ opacity: [1, 0.3, 1] }}
                    transition={{ duration: 1, repeat: Infinity }}
                  />
                  <WaveformVisualizer level={audioLevel} />
                  <DurationBadge secs={durationSecs} />
                </motion.div>
              )}

              {phase === "PROCESSING" && (
                <motion.div
                  key="processing"
                  initial={{ opacity: 0 }}
                  animate={{ opacity: 1 }}
                  exit={{ opacity: 0 }}
                  transition={{ duration: 0.15 }}
                >
                  <ProcessingIndicator />
                </motion.div>
              )}

              {phase === "SUCCESS" && (
                <motion.div
                  key="success"
                  initial={{ opacity: 0 }}
                  animate={{ opacity: 1 }}
                  exit={{ opacity: 0 }}
                  transition={{ duration: 0.15 }}
                >
                  <SuccessFlash charCount={charCount} />
                </motion.div>
              )}

              {phase === "ERROR" && (
                <motion.div
                  key="error"
                  initial={{ opacity: 0 }}
                  animate={{ opacity: 1 }}
                  exit={{ opacity: 0 }}
                  transition={{ duration: 0.15 }}
                >
                  <ErrorIndicator message={error ?? "An error occurred"} />
                </motion.div>
              )}
            </AnimatePresence>
          </motion.div>
        )}
      </AnimatePresence>
    </div>
  );
}

function DurationBadge({ secs }: { secs: number }) {
  const mins = Math.floor(secs / 60);
  const s = secs % 60;
  const label = mins > 0 ? `${mins}:${String(s).padStart(2, "0")}` : `0:${String(secs).padStart(2, "0")}`;
  const isNearMax = secs >= 50;

  return (
    <span
      className={`text-xs font-mono tabular-nums ml-1 ${isNearMax ? "text-amber-400" : "text-zinc-400"}`}
    >
      {label}
    </span>
  );
}
