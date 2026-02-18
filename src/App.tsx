import { getCurrentWindow } from "@tauri-apps/api/window";
import { useEffect } from "react";
import { Overlay } from "./components/Overlay/Overlay";
import { SettingsWindow } from "./components/Settings/SettingsWindow";
import { useSettingsStore } from "./store/settingsStore";
import { invoke } from "@tauri-apps/api/core";
import type { ApiConfig } from "./types/settings";

/** Merges VITE_OPENAI_API_KEY env fallback before pushing config to Rust. */
function pushApiConfig(api: ApiConfig) {
  const config = { ...api };
  if (!config.openai.apiKey && import.meta.env.VITE_OPENAI_API_KEY) {
    config.openai = { ...config.openai, apiKey: import.meta.env.VITE_OPENAI_API_KEY };
  }
  invoke("set_api_config", { config }).catch(console.error);
}

type Route = "overlay" | "settings";

/**
 * Determine the current window synchronously.
 * Priority: Tauri window label → URL hash → default "overlay".
 */
function getRoute(): Route {
  try {
    const label = getCurrentWindow().label;
    return label === "settings" ? "settings" : "overlay";
  } catch {
    // Not in Tauri context (browser dev) — fall back to hash
    const hash = window.location.hash.replace("#", "");
    return hash === "settings" ? "settings" : "overlay";
  }
}

// Resolved once at module load — window label never changes.
const ROUTE: Route = getRoute();

export default function App() {
  const { load, isLoaded } = useSettingsStore();

  useEffect(() => {
    document.body.setAttribute("data-route", ROUTE);
  }, []);

  useEffect(() => {
    if (!isLoaded) load();
  }, [isLoaded, load]);

  // Push API config to Rust whenever settings load or change
  useEffect(() => {
    if (!isLoaded) return;
    const { settings } = useSettingsStore.getState();
    pushApiConfig(settings.api);
  }, [isLoaded]);

  if (ROUTE === "settings") {
    return <SettingsWindow />;
  }

  return <Overlay />;
}
