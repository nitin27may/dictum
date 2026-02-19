import { motion } from "framer-motion";

export function SuccessFlash() {
  return (
    <div className="flex items-center gap-2.5">
      {/* Checkmark circle */}
      <motion.div
        className="flex items-center justify-center w-5 h-5 rounded-full bg-emerald-500/25 flex-shrink-0"
        initial={{ scale: 0.4, opacity: 0 }}
        animate={{ scale: 1, opacity: 1 }}
        transition={{ type: "spring", stiffness: 500, damping: 18 }}
      >
        <svg width="10" height="10" viewBox="0 0 10 10" fill="none">
          <motion.path
            d="M1.5 5l2.5 2.5L8.5 2"
            stroke="#34d399"
            strokeWidth="1.5"
            strokeLinecap="round"
            strokeLinejoin="round"
            initial={{ pathLength: 0 }}
            animate={{ pathLength: 1 }}
            transition={{ duration: 0.25, delay: 0.08 }}
          />
        </svg>
      </motion.div>

      <motion.span
        className="text-emerald-400 text-[13px] font-normal"
        initial={{ opacity: 0 }}
        animate={{ opacity: 1 }}
        transition={{ delay: 0.12 }}
      >
        Done
      </motion.span>
    </div>
  );
}
