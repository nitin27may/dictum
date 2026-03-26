import { useEffect, useState } from "react";
import { enable, disable, isEnabled } from "@tauri-apps/plugin-autostart";
import { registerHotkey } from "../../services/tauri-bridge";
import { useSettingsStore } from "../../store/settingsStore";
import { ApiConfigSection } from "./ApiConfigSection";
import { HotkeyCapture } from "./HotkeyCapture";

type Tab = "general" | "api" | "about";

export function SettingsWindow() {
  const [activeTab, setActiveTab] = useState<Tab>("general");
  const { settings, save, isLoaded, load } = useSettingsStore();
  const [hotkeyError, setHotkeyError] = useState<string | null>(null);
  const [autostart, setAutostart] = useState(false);

  useEffect(() => {
    if (!isLoaded) load();
  }, [isLoaded, load]);

  useEffect(() => {
    isEnabled().then(setAutostart).catch(console.error);
  }, []);

  const handleHotkeyChange = async (hotkey: string) => {
    setHotkeyError(null);
    try {
      await registerHotkey(hotkey);
      await save({ hotkey });
    } catch (err) {
      setHotkeyError(err instanceof Error ? err.message : String(err));
    }
  };

  const handleMaxDurationChange = (secs: number) => {
    save({ general: { ...settings.general, maxRecordingSeconds: secs } });
  };

  if (!isLoaded) {
    return (
      <div className="flex items-center justify-center h-full">
        <div className="text-zinc-500 text-sm">Loading settings...</div>
      </div>
    );
  }

  return (
    <div className="flex flex-col h-screen bg-zinc-900 text-zinc-100">
      {/* Title bar */}
      <div className="flex items-center px-5 py-4 border-b border-zinc-800">
        <h1 className="text-base font-semibold text-zinc-100">Dictum Settings</h1>
      </div>

      {/* Tab bar */}
      <div className="flex gap-1 px-4 pt-3 border-b border-zinc-800">
        {(["general", "api", "about"] as Tab[]).map((tab) => (
          <button
            key={tab}
            type="button"
            onClick={() => setActiveTab(tab)}
            className={`px-4 py-2 text-sm rounded-t-md transition-colors capitalize ${
              activeTab === tab
                ? "text-zinc-100 border-b-2 border-teal-500 -mb-px"
                : "text-zinc-400 hover:text-zinc-200"
            }`}
          >
            {tab === "api" ? "API" : tab.charAt(0).toUpperCase() + tab.slice(1)}
          </button>
        ))}
      </div>

      {/* Content */}
      <div className="flex-1 overflow-y-auto px-5 py-5">
        {activeTab === "general" && (
          <div className="space-y-6">
            <Section title="Hotkey">
              <div className="flex items-center gap-4">
                <HotkeyCapture
                  value={settings.hotkey}
                  onChange={handleHotkeyChange}
                />
                <span className="text-xs text-zinc-500">
                  Hold to record, release to transcribe
                </span>
              </div>
              {hotkeyError && (
                <p className="text-xs text-red-400 mt-1">{hotkeyError}</p>
              )}
            </Section>

            <Section title="Recording">
              <div className="flex items-center gap-4">
                <label className="text-sm text-zinc-300">Max duration</label>
                <select
                  value={settings.general.maxRecordingSeconds}
                  onChange={(e) => handleMaxDurationChange(Number(e.target.value))}
                  className="bg-zinc-800 border border-zinc-600 rounded-md px-3 py-1.5 text-sm text-zinc-200"
                >
                  {[30, 60, 90, 120].map((s) => (
                    <option key={s} value={s}>
                      {s}s
                    </option>
                  ))}
                </select>
              </div>
            </Section>

            <Section title="Startup">
              <div className="flex items-center justify-between">
                <div>
                  <span className="text-sm text-zinc-300">Launch at login</span>
                  <p className="text-xs text-zinc-500 mt-0.5">Start Dictum automatically when you log in</p>
                </div>
                <button
                  type="button"
                  role="switch"
                  aria-checked={autostart}
                  onClick={async () => {
                    try {
                      if (autostart) {
                        await disable();
                        setAutostart(false);
                      } else {
                        await enable();
                        setAutostart(true);
                      }
                    } catch (err) {
                      console.error("Autostart toggle failed:", err);
                    }
                  }}
                  className={`relative inline-flex h-5 w-9 items-center rounded-full transition-colors ${
                    autostart ? "bg-teal-600" : "bg-zinc-700"
                  }`}
                >
                  <span
                    className={`inline-block h-3.5 w-3.5 rounded-full bg-white transition-transform ${
                      autostart ? "translate-x-[18px]" : "translate-x-[3px]"
                    }`}
                  />
                </button>
              </div>
            </Section>

            <Section title="Permissions">
              <PermissionsStatus />
            </Section>
          </div>
        )}

        {activeTab === "api" && <ApiConfigSection />}

        {activeTab === "about" && <AboutSection />}
      </div>
    </div>
  );
}

function Section({ title, children }: { title: string; children: React.ReactNode }) {
  return (
    <div>
      <h2 className="text-xs font-medium text-zinc-400 uppercase tracking-wider mb-3">
        {title}
      </h2>
      <div className="space-y-3">{children}</div>
    </div>
  );
}

function PermissionsStatus() {
  const [micOk, setMicOk] = useState<boolean | null>(null);
  const [axOk, setAxOk] = useState<boolean | null>(null);
  const [requesting, setRequesting] = useState(false);

  const checkPermissions = () => {
    import("../../services/tauri-bridge").then(
      ({ checkAccessibilityPermission, checkMicrophonePermission }) => {
        checkAccessibilityPermission().then(setAxOk);
        checkMicrophonePermission().then(setMicOk);
      }
    );
  };

  useEffect(() => {
    checkPermissions();
  }, []);

  const handleRequestMic = async () => {
    setRequesting(true);
    try {
      const { requestMicrophonePermission } = await import("../../services/tauri-bridge");
      const result = await requestMicrophonePermission();
      setMicOk(result);
    } finally {
      setRequesting(false);
    }
  };

  return (
    <div className="space-y-3">
      <div className="flex items-center justify-between">
        <PermissionRow
          label="Microphone"
          status={micOk}
          hint="Required for recording."
        />
        {micOk !== true && (
          <button
            type="button"
            onClick={handleRequestMic}
            disabled={requesting}
            className="text-xs px-3 py-1 bg-zinc-700 hover:bg-zinc-600 text-zinc-300 rounded-md transition-colors disabled:opacity-50"
          >
            {requesting ? "Requesting..." : "Request Access"}
          </button>
        )}
      </div>
      <PermissionRow
        label="Accessibility"
        status={axOk}
        hint="Required for text injection. Add app in System Settings → Privacy → Accessibility."
      />
    </div>
  );
}

function PermissionRow({
  label,
  status,
  hint,
}: {
  label: string;
  status: boolean | null;
  hint: string;
}) {
  return (
    <div className="flex items-start gap-3">
      <div
        className={`w-2 h-2 rounded-full mt-1.5 flex-shrink-0 ${
          status === true
            ? "bg-emerald-500"
            : status === false
              ? "bg-red-500"
              : "bg-zinc-600"
        }`}
      />
      <div>
        <span className="text-sm text-zinc-300">{label}</span>
        {status === false && (
          <p className="text-xs text-zinc-500 mt-0.5">{hint}</p>
        )}
      </div>
    </div>
  );
}

function AboutSection() {
  return (
    <div className="space-y-4 text-sm text-zinc-400">
      <div>
        <p className="text-zinc-100 font-semibold text-base">Dictum</p>
        <p className="text-zinc-500 mt-1">Version 0.1.0</p>
      </div>
      <p>
        Push-to-talk speech-to-text. Hold the hotkey to record, release to
        transcribe and inject at your cursor.
      </p>
      <div className="space-y-1">
        <p className="text-zinc-500 text-xs uppercase tracking-wider font-medium">
          Stack
        </p>
        <p>Tauri 2 · React · Rust · OpenAI Whisper</p>
      </div>
    </div>
  );
}
