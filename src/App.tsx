import { getCurrentWindow } from "@tauri-apps/api/window";
import { useEffect } from "react";
import { Overlay } from "./components/Overlay/Overlay";
import { SettingsWindow } from "./components/Settings/SettingsWindow";
import { useSettingsStore } from "./store/settingsStore";
import { invoke } from "@tauri-apps/api/core";
import type { ApiConfig } from "./types/settings";

/** Merges VITE_* env fallbacks before pushing config to Rust. */
function pushApiConfig(api: ApiConfig) {
  const config = { ...api };
  // OpenAI env fallback
  if (!config.openai.apiKey && import.meta.env.VITE_OPENAI_API_KEY) {
    config.openai = { ...config.openai, apiKey: import.meta.env.VITE_OPENAI_API_KEY };
  }
  // Azure env fallbacks
  if (!config.azure.endpoint && import.meta.env.VITE_AZURE_ENDPOINT) {
    config.azure = { ...config.azure, endpoint: import.meta.env.VITE_AZURE_ENDPOINT };
  }
  if (!config.azure.apiKey && import.meta.env.VITE_AZURE_API_KEY) {
    config.azure = { ...config.azure, apiKey: import.meta.env.VITE_AZURE_API_KEY };
  }
  if (import.meta.env.VITE_AZURE_WHISPER_DEPLOYMENT) {
    config.azure = { ...config.azure, whisperDeployment: import.meta.env.VITE_AZURE_WHISPER_DEPLOYMENT };
  }
  if (import.meta.env.VITE_AZURE_GPT_DEPLOYMENT) {
    config.azure = { ...config.azure, gptDeployment: import.meta.env.VITE_AZURE_GPT_DEPLOYMENT };
  }
  if (import.meta.env.VITE_AZURE_API_VERSION) {
    config.azure = { ...config.azure, apiVersion: import.meta.env.VITE_AZURE_API_VERSION };
  }
  // Auto-select azure provider if azure keys are set but no explicit provider chosen via UI
  if (config.provider === "openai" && !config.openai.apiKey && config.azure.apiKey && config.azure.endpoint) {
    config.provider = "azure";
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
  const api = useSettingsStore((s) => s.settings.api);
  useEffect(() => {
    if (!isLoaded) return;
    pushApiConfig(api);
  }, [isLoaded, api]);

  if (ROUTE === "settings") {
    return <SettingsWindow />;
  }

  return <Overlay />;
}
