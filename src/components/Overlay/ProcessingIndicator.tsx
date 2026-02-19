import { motion } from "framer-motion";

export function ProcessingIndicator() {
  return (
    <div className="flex items-center gap-3">
      {/* Arc spinner */}
      <div className="w-4 h-4 rounded-full border-[1.5px] border-white/20 border-t-white animate-spin flex-shrink-0" />

      {/* Label */}
      <span className="text-white/80 text-[13px] font-normal tracking-wide">
        Transcribing
        <AnimatedDots />
      </span>
    </div>
  );
}

function AnimatedDots() {
  return (
    <span className="inline-flex ml-0.5">
      {[0, 1, 2].map((i) => (
        <motion.span
          key={i}
          className="text-white/50"
          animate={{ opacity: [0.2, 1, 0.2] }}
          transition={{
            duration: 1.0,
            repeat: Infinity,
            delay: i * 0.18,
            ease: "easeInOut",
          }}
        >
          .
        </motion.span>
      ))}
    </span>
  );
}
