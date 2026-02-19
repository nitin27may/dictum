import { motion } from "framer-motion";

// Bell curve envelope — center bar is tallest for a natural waveform shape
const ENVELOPE = [0.35, 0.6, 0.82, 0.95, 1.0, 0.95, 0.82, 0.6, 0.35];
// Symmetric spring delays so bars animate outward from center
const DELAYS =   [0.04, 0.03, 0.02, 0.01, 0,    0.01, 0.02, 0.03, 0.04];

interface WaveformVisualizerProps {
  level: number; // 0–1, driven by RMS audio level
}

export function WaveformVisualizer({ level }: WaveformVisualizerProps) {
  return (
    <div className="flex items-center gap-[3px] h-7">
      {ENVELOPE.map((env, i) => {
        // Minimum height creates a subtle flat-line when silent
        const amplitude = Math.max(0.12, level) * env;
        const heightPx = Math.round(3 + amplitude * 24); // 3–27px

        return (
          <motion.div
            key={i}
            className="rounded-full bg-white"
            style={{ width: 2.5, minHeight: 3 }}
            animate={{
              height: heightPx,
              opacity: 0.55 + amplitude * 0.45,
            }}
            transition={{
              type: "spring",
              stiffness: 380,
              damping: 22,
              delay: DELAYS[i],
            }}
          />
        );
      })}
    </div>
  );
}
