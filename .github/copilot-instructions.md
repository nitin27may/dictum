# Wisper — Copilot Instructions

## Build & Run

```bash
npm run tauri dev         # Dev server + Tauri app (hot reload)
npm run tauri build       # Production build
npm run type-check        # TypeScript strict check (no emit)
cd src-tauri && cargo test # Rust tests
cargo test test_name      # Single Rust test (from src-tauri/)
```

No frontend test runner is configured. Linting/formatting tools are not set up.

## Architecture

Tauri 2.x desktop app: Rust backend for system-level concerns, React/TypeScript frontend for all business logic.

### Rust Does 4 Things Only
1. **Audio capture** — cpal stream at 16kHz mono f32, RMS-based level metering, hound WAV encoding
2. **Global hotkey** — `tauri-plugin-global-shortcut`, **must register on main thread** (macOS requirement — never in async spawn)
3. **Text injection** — clipboard save → set text → Cmd+V paste → restore clipboard (500ms window)
4. **Tauri commands** — thin wrappers in `src-tauri/src/commands/` exposing Rust to TypeScript

### Data Flow
```
Rust emits events → useRecordingFlow hook listens → Zustand store updates → React renders
```
The frontend never calls Tauri `invoke()` directly. All IPC goes through `src/services/tauri-bridge.ts` which provides typed wrappers. This isolates Tauri from business logic for testability and future mobile portability.

### Two Windows (Hash Routing)
- `#overlay` — always-on-top, transparent, click-through (`set_ignore_cursor_events(true)`), no decorations
- `#settings` — standard decorated window, opened from tray menu

The app hides from the macOS Dock via `LSUIElement` (set programmatically with unsafe ObjC in `lib.rs`).

### State Machine
```
IDLE → [hotkey hold ≥200ms] → RECORDING → [release] → PROCESSING → SUCCESS/ERROR → IDLE
```
Taps under 200ms are ignored. 60-second hard cap on recording with countdown.

## Key Conventions

### TypeScript
- **Path alias**: `@/*` maps to `src/*` (configured in tsconfig.json)
- **Zod schemas as single source of truth**: Types are inferred via `z.infer<typeof Schema>`, not manually declared. Settings validation uses `safeParse()` with fallback to defaults.
- **Zustand stores**: Factory-function style returning state + actions in one object. `recordingStore` is ephemeral; `settingsStore` persists via Tauri store plugin with lazy import for browser compatibility.
- **Custom errors**: `TranscriptionError` class with optional `statusCode`; retry logic distinguishes transient (5xx) from permanent (4xx) failures.
- **API key precedence**: Settings UI → `VITE_*` env vars → throw.

### Rust
- **Shared state**: `AppState` struct with `Arc<Mutex<T>>` fields for audio capture, hotkey, and config. Accessed via Tauri `State<'_, AppState>` injection.
- **Error handling**: Commands return `Result<T, String>` — `.map_err(|e| e.to_string())` converts anyhow errors. Log with `log::error!()` before returning.
- **SendStream wrapper**: Unsafe `impl Send` for cpal's non-Send `Stream`, justified by exclusive Mutex access.
- **Platform-conditional deps**: macOS uses `core-foundation`/`objc`; Windows uses `windows` crate. Gated with `#[cfg(target_os)]`.

### Environment
Copy `.env.local.example` to `.env.local`. All env vars are prefixed `VITE_` (exposed to frontend via Vite). API keys set in the Settings UI take precedence over env vars.

## Critical Constraints

- **Overlay must use `set_ignore_cursor_events(true)`** or it blocks all mouse events system-wide
- **Microphone silence detection**: Check RMS ≈ 0 before sending to Whisper API — silent WAVs waste API calls and return empty results
- **Audio buffer**: 60s cap = ~3.8MB at 16kHz f32
- **Clipboard injection**: The only reliable method for Electron apps (VS Code, Slack); AX API doesn't work on them
- **Whisper cold start**: First API call takes 2-5s; retry logic with 3s timeout is built in
