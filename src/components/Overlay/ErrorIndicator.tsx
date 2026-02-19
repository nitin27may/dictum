import { motion } from "framer-motion";

interface ErrorIndicatorProps {
  message: string;
}

export function ErrorIndicator({ message }: ErrorIndicatorProps) {
  const display = message.length > 44 ? `${message.slice(0, 41)}…` : message;

  return (
    <div className="flex items-center gap-2.5">
      {/* Error dot */}
      <motion.div
        className="flex items-center justify-center w-5 h-5 rounded-full bg-red-500/25 flex-shrink-0"
        initial={{ scale: 0.4, opacity: 0 }}
        animate={{ scale: 1, opacity: 1 }}
        transition={{ type: "spring", stiffness: 500, damping: 18 }}
      >
        <svg width="10" height="10" viewBox="0 0 10 10" fill="none">
          <path
            d="M5 2.5v3M5 7.5h.01"
            stroke="#f87171"
            strokeWidth="1.5"
            strokeLinecap="round"
          />
        </svg>
      </motion.div>

      <motion.span
        className="text-red-400 text-[12px] font-normal leading-tight"
        initial={{ opacity: 0 }}
        animate={{ opacity: 1 }}
        transition={{ delay: 0.1 }}
      >
        {display}
      </motion.span>
    </div>
  );
}
