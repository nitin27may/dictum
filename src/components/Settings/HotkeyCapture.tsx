import { useEffect, useRef, useState } from "react";
import { unregisterHotkeys, registerHotkey } from "../../services/tauri-bridge";

interface HotkeyCaptureProps {
  value: string;
  onChange: (hotkey: string) => void;
  disabled?: boolean;
}

export function HotkeyCapture({ value, onChange, disabled }: HotkeyCaptureProps) {
  const [isCapturing, setIsCapturing] = useState(false);
  const [captured, setCaptured] = useState<string[]>([]);
  const ref = useRef<HTMLButtonElement>(null);
  const prevHotkeyRef = useRef(value);

  // Unregister global shortcut when entering capture mode,
  // re-register previous hotkey if capture is cancelled (blur without commit).
  useEffect(() => {
    if (isCapturing) {
      prevHotkeyRef.current = value;
      unregisterHotkeys().catch(console.error);
    }
  }, [isCapturing]);

  useEffect(() => {
    if (!isCapturing) return;

    const handleKeyDown = (e: KeyboardEvent) => {
      e.preventDefault();

      const parts: string[] = [];
      if (e.metaKey) parts.push("Meta");
      if (e.ctrlKey) parts.push("Control");
      if (e.altKey) parts.push("Alt");
      if (e.shiftKey) parts.push("Shift");

      const key = e.key;
      if (!["Meta", "Control", "Alt", "Shift"].includes(key)) {
        // Capitalize key name for Tauri shortcut parser compatibility
        const keyName = key === " " ? "Space" : key.length === 1 ? key.toUpperCase() : key;
        parts.push(keyName);
      }

      setCaptured(parts);
    };

    const handleKeyUp = (e: KeyboardEvent) => {
      e.preventDefault();

      // Commit if we have a non-modifier key
      const nonModifiers = captured.filter(
        (k) => !["Meta", "Control", "Alt", "Shift"].includes(k)
      );

      if (nonModifiers.length > 0) {
        const hotkey = captured.join("+");
        setIsCapturing(false);
        setCaptured([]);
        onChange(hotkey);
      }
    };

    window.addEventListener("keydown", handleKeyDown);
    window.addEventListener("keyup", handleKeyUp);

    return () => {
      window.removeEventListener("keydown", handleKeyDown);
      window.removeEventListener("keyup", handleKeyUp);
    };
  }, [isCapturing, captured, onChange]);

  const handleBlur = () => {
    if (isCapturing) {
      setIsCapturing(false);
      setCaptured([]);
      // Re-register the previous hotkey since capture was cancelled
      registerHotkey(prevHotkeyRef.current).catch(console.error);
    }
  };

  const displayValue = isCapturing
    ? captured.length > 0
      ? captured.join("+")
      : "Press keys..."
    : value;

  return (
    <button
      ref={ref}
      type="button"
      disabled={disabled}
      onClick={() => {
        setIsCapturing(true);
        setCaptured([]);
        ref.current?.focus();
      }}
      onBlur={handleBlur}
      className={`
        px-4 py-2 rounded-md font-mono text-sm border transition-all min-w-32 text-left
        ${isCapturing
          ? "border-teal-500 bg-teal-500/10 text-teal-300 ring-1 ring-teal-500/50"
          : "border-zinc-600 bg-zinc-800 text-zinc-200 hover:border-zinc-500"
        }
        ${disabled ? "opacity-40 cursor-not-allowed" : "cursor-pointer"}
      `}
    >
      {displayValue}
    </button>
  );
}
