import { motion } from "framer-motion";

interface SuccessFlashProps {
  charCount: number;
}

export function SuccessFlash({ charCount }: SuccessFlashProps) {
  return (
    <div className="flex items-center gap-3">
      {/* Checkmark */}
      <motion.div
        className="flex items-center justify-center w-6 h-6 rounded-full bg-emerald-500/20"
        initial={{ scale: 0.5, opacity: 0 }}
        animate={{ scale: 1, opacity: 1 }}
        transition={{ type: "spring", stiffness: 400, damping: 15 }}
      >
        <motion.svg
          width="14"
          height="14"
          viewBox="0 0 14 14"
          fill="none"
          initial={{ pathLength: 0 }}
          animate={{ pathLength: 1 }}
          transition={{ duration: 0.3, delay: 0.1 }}
        >
          <motion.path
            d="M2 7l3.5 3.5L12 3"
            stroke="#10b981"
            strokeWidth="2"
            strokeLinecap="round"
            strokeLinejoin="round"
            initial={{ pathLength: 0 }}
            animate={{ pathLength: 1 }}
            transition={{ duration: 0.3, delay: 0.1 }}
          />
        </motion.svg>
      </motion.div>

      <motion.span
        className="text-emerald-400 text-sm font-medium"
        initial={{ opacity: 0, x: -4 }}
        animate={{ opacity: 1, x: 0 }}
        transition={{ delay: 0.15 }}
      >
        {charCount} {charCount === 1 ? "character" : "characters"} injected
      </motion.span>
    </div>
  );
}
