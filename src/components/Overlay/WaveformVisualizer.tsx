import { motion } from "framer-motion";

const BAR_COUNT = 14;
// Phase offsets for organic non-uniform movement
const PHASE_OFFSETS = Array.from({ length: BAR_COUNT }, (_, i) =>
  Math.sin((i / BAR_COUNT) * Math.PI * 2) * 0.5
);

interface WaveformVisualizerProps {
  level: number; // 0–1
}

export function WaveformVisualizer({ level }: WaveformVisualizerProps) {
  return (
    <div className="flex items-center gap-[3px] h-10">
      {PHASE_OFFSETS.map((offset, i) => {
        // Each bar gets slightly different height based on phase offset
        const barLevel = Math.max(0.08, level * (0.6 + Math.abs(offset) * 0.8));
        const heightPx = Math.round(4 + barLevel * 36); // 4–40px

        return (
          <motion.div
            key={i}
            className="w-[3px] rounded-full bg-gradient-to-t from-teal-400 to-blue-500 origin-center"
            animate={{ height: heightPx }}
            transition={{
              type: "spring",
              stiffness: 300,
              damping: 20,
              delay: offset * 0.02,
            }}
            style={{ minHeight: 4 }}
          />
        );
      })}
    </div>
  );
}
