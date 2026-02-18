import { motion } from "framer-motion";

interface ErrorIndicatorProps {
  message: string;
}

export function ErrorIndicator({ message }: ErrorIndicatorProps) {
  // Truncate long error messages for the compact overlay
  const display = message.length > 52 ? `${message.slice(0, 49)}…` : message;

  return (
    <div className="flex items-center gap-3">
      {/* Red flash icon */}
      <motion.div
        className="flex items-center justify-center w-6 h-6 rounded-full bg-red-500/20 flex-shrink-0"
        initial={{ scale: 0.5, opacity: 0 }}
        animate={{ scale: [1.2, 1], opacity: 1 }}
        transition={{ type: "spring", stiffness: 500, damping: 15 }}
      >
        <svg width="12" height="12" viewBox="0 0 12 12" fill="none">
          <path
            d="M6 2v4M6 9.5h.01"
            stroke="#ef4444"
            strokeWidth="1.5"
            strokeLinecap="round"
          />
        </svg>
      </motion.div>

      <motion.span
        className="text-red-400 text-xs font-medium leading-tight"
        initial={{ opacity: 0 }}
        animate={{ opacity: 1 }}
        transition={{ delay: 0.1 }}
      >
        {display}
      </motion.span>
    </div>
  );
}
