import { motion } from "framer-motion";

export function ProcessingIndicator() {
  return (
    <div className="flex items-center gap-3">
      {/* Pulsing ring */}
      <div className="relative flex items-center justify-center w-6 h-6">
        <motion.div
          className="absolute inset-0 rounded-full border-2 border-zinc-400"
          animate={{ scale: [0.85, 1.15, 0.85], opacity: [0.4, 1, 0.4] }}
          transition={{ duration: 1.5, repeat: Infinity, ease: "easeInOut" }}
        />
        <div className="w-2 h-2 rounded-full bg-zinc-300" />
      </div>

      {/* Animated ellipsis */}
      <span className="text-zinc-300 text-sm font-medium tracking-wide">
        Processing
        <AnimatedEllipsis />
      </span>
    </div>
  );
}

function AnimatedEllipsis() {
  return (
    <span className="inline-flex">
      {[0, 1, 2].map((i) => (
        <motion.span
          key={i}
          animate={{ opacity: [0, 1, 0] }}
          transition={{
            duration: 1.2,
            repeat: Infinity,
            delay: i * 0.2,
            ease: "easeInOut",
          }}
        >
          .
        </motion.span>
      ))}
    </span>
  );
}
