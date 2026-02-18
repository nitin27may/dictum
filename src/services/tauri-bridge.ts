/**
 * tauri-bridge.ts — Typed invoke() wrappers.
 *
 * All Tauri IPC calls go through this file.
 * This isolation layer enables swapping implementations
 * without touching business logic (Phase 3 mobile support).
 */

import { invoke } from "@tauri-apps/api/core";
import { listen, UnlistenFn } from "@tauri-apps/api/event";

// ── Audio ─────────────────────────────────────────────────────────────────────

export async function startRecording(): Promise<void> {
  await invoke("start_recording");
}

export async function stopRecording(): Promise<Uint8Array> {
  const bytes = await invoke<number[]>("stop_recording");
  return new Uint8Array(bytes);
}

export async function getAudioDevices(): Promise<string[]> {
  return invoke<string[]>("get_audio_devices");
}

export async function checkMicrophonePermission(): Promise<boolean | null> {
  return invoke<boolean | null>("check_microphone_permission");
}

export async function requestMicrophonePermission(): Promise<boolean | null> {
  return invoke<boolean | null>("request_microphone_permission");
}

// ── Injection ─────────────────────────────────────────────────────────────────

export async function injectText(text: string): Promise<void> {
  await invoke("inject_text", { text });
}

export async function checkAccessibilityPermission(): Promise<boolean> {
  return invoke<boolean>("check_accessibility_permission");
}

// ── Settings / Hotkey ─────────────────────────────────────────────────────────

export async function registerHotkey(shortcut: string): Promise<void> {
  await invoke("register_hotkey", { shortcut });
}

export async function getPlatform(): Promise<string> {
  return invoke<string>("get_platform");
}

export async function getCurrentHotkey(): Promise<string> {
  return invoke<string>("get_current_hotkey");
}

// ── Event listeners ───────────────────────────────────────────────────────────

export function onHotkeyPressed(handler: () => void): Promise<UnlistenFn> {
  return listen("hotkey-pressed", handler);
}

export function onHotkeyReleased(handler: () => void): Promise<UnlistenFn> {
  return listen("hotkey-released", handler);
}

export function onAudioLevel(handler: (level: number) => void): Promise<UnlistenFn> {
  return listen<number>("audio-level", (event) => handler(event.payload));
}

export function onAudioSilenceDetected(handler: () => void): Promise<UnlistenFn> {
  return listen("audio-silence-detected", handler);
}
