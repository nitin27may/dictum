import { create } from "zustand";
import { DEFAULT_SETTINGS, Settings, SettingsSchema } from "../types/settings";

// Lazy import — Tauri store plugin only available in Tauri context
async function getTauriStore() {
  const { Store } = await import("@tauri-apps/plugin-store");
  return Store.load("wisper-settings.json");
}

interface SettingsStore {
  settings: Settings;
  isLoaded: boolean;
  load: () => Promise<void>;
  save: (partial: Partial<Settings>) => Promise<void>;
  setApiKey: (provider: "openai" | "azure", key: string) => Promise<void>;
}

export const useSettingsStore = create<SettingsStore>((set, get) => ({
  settings: DEFAULT_SETTINGS,
  isLoaded: false,

  load: async () => {
    try {
      const store = await getTauriStore();
      const raw = await store.get<Record<string, unknown>>("settings");
      if (raw) {
        const parsed = SettingsSchema.safeParse(raw);
        if (parsed.success) {
          set({ settings: parsed.data, isLoaded: true });
        } else {
          // Schema mismatch — use defaults and re-save
          console.warn("Settings schema mismatch, resetting to defaults", parsed.error);
          set({ settings: DEFAULT_SETTINGS, isLoaded: true });
          await store.set("settings", DEFAULT_SETTINGS);
          await store.save();
        }
      } else {
        // First run — check for WISPER_OPENAI_API_KEY env var via Tauri
        const defaultSettings = { ...DEFAULT_SETTINGS };
        set({ settings: defaultSettings, isLoaded: true });
      }
    } catch (err) {
      console.error("Failed to load settings:", err);
      set({ settings: DEFAULT_SETTINGS, isLoaded: true });
    }
  },

  save: async (partial: Partial<Settings>) => {
    const current = get().settings;
    const updated = { ...current, ...partial };
    const parsed = SettingsSchema.safeParse(updated);
    if (!parsed.success) {
      throw new Error(`Invalid settings: ${parsed.error.message}`);
    }
    set({ settings: parsed.data });
    try {
      const store = await getTauriStore();
      await store.set("settings", parsed.data);
      await store.save();
      // Rust sync is handled by App.tsx useEffect reacting to api config changes
    } catch (err) {
      console.error("Failed to persist settings:", err);
    }
  },

  setApiKey: async (provider: "openai" | "azure", key: string) => {
    const { settings, save } = get();
    if (provider === "openai") {
      await save({
        api: {
          ...settings.api,
          openai: { ...settings.api.openai, apiKey: key },
        },
      });
    } else {
      await save({
        api: {
          ...settings.api,
          azure: { ...settings.api.azure, apiKey: key },
        },
      });
    }
  },
}));
