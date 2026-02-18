# Wisper — Project Instructions for Claude Code

## Tech Stack
- **Framework**: Tauri 2.x (Rust backend + React/TypeScript frontend)
- **UI**: React 18, TypeScript strict mode, Tailwind CSS, Framer Motion
- **State**: Zustand 5 (stores), Zod (validation/schemas)
- **Audio**: cpal (Rust, 16kHz mono f32), hound (WAV encoding)
- **AI**: Azure OpenAI Whisper (primary), OpenAI API (fallback)
- **HTTP**: reqwest (Rust, multipart)

## Architecture Decisions

### Rust Scope (Minimal by Design)
Rust handles only 4 system-level concerns:
1. **Audio capture** — cpal stream, level metering, buffer
2. **Global hotkey** — tauri-plugin-global-shortcut on main thread (macOS requirement)
3. **Text injection** — clipboard + paste (MVP), AX API (Phase 2)
4. **Tauri commands** — thin wrappers exposing Rust functionality to TypeScript

All business logic lives in TypeScript. Transcription is a fetch call in `services/transcription.ts`.

### Text Injection Strategy
- **MVP (Phase 1)**: Clipboard + Cmd+V paste — works in all apps including Electron apps
- **Phase 2**: macOS AX API injection for non-clipboard approach (AXManualAccessibility=true for Electron)
- Known quirk: 500ms window where clipboard contains transcription — documented

### Hotkey Architecture
- `tauri-plugin-global-shortcut` registers on main thread (NEVER in async spawn)
- Hold detection: track keydown timestamp, compare on keyup; ignore < 200ms taps
- Uses rdev for raw key events (keydown + keyup) via dedicated thread + channel

## Key Architectural Constraints

1. **Hotkey MUST initialize on Tauri main thread** — macOS requires this
2. **Overlay window**: always-on-top, click-through (`set_ignore_cursor_events(true)`), transparent, no decorations
3. **Microphone silence**: detect RMS ≈ 0, surface actionable error (don't send silent WAV to API)
4. **API keys**: stored via Tauri store plugin with machine UUID-derived encryption; never plaintext
5. **Audio buffer**: 60s hard cap = ~3.8MB at 16kHz f32; enforce with T-10s UI countdown

## State Machine
```
IDLE → [hotkey down] → RECORDING → [released < 200ms] → IDLE (ignore tap)
RECORDING → [released ≥ 200ms] → PROCESSING → SUCCESS | ERROR → IDLE
```

## File Responsibilities

| File | Owns |
|------|------|
| `src-tauri/src/main.rs` | Entry point, tray setup, window positioning |
| `src-tauri/src/lib.rs` | App builder, command registration, AppState |
| `src-tauri/src/audio/capture.rs` | cpal stream, level events, buffer Vec<f32> |
| `src-tauri/src/audio/encoder.rs` | f32 PCM → WAV bytes (hound) |
| `src-tauri/src/hotkey/mod.rs` | Global shortcut registration |
| `src-tauri/src/injection/macos.rs` | Clipboard save/set/paste/restore |
| `src-tauri/src/commands/audio_commands.rs` | start_recording, stop_recording, transcribe_audio |
| `src-tauri/src/commands/injection_commands.rs` | inject_text, check_accessibility_permission |
| `src-tauri/src/commands/settings_commands.rs` | register_hotkey, get_platform |
| `src/services/transcription.ts` | Azure OpenAI / OpenAI Whisper API POST |
| `src/services/tauri-bridge.ts` | All invoke() wrappers — isolates Tauri from business logic |
| `src/hooks/useRecordingFlow.ts` | Full recording lifecycle orchestration |
| `src/store/recordingStore.ts` | Zustand state machine |
| `src/store/settingsStore.ts` | Settings with Tauri store persistence |

## Dev Commands
```bash
npm run tauri dev       # Start dev server + Tauri app
npm run tauri build     # Production build
cargo test              # Run Rust tests (from src-tauri/)
npm run type-check      # TypeScript check without build
```

## Phase Boundaries
- **Phase 1 (MVP)**: macOS only, clipboard injection, Azure/OpenAI Whisper
- **Phase 2**: Windows, AX injection, GPT formatting profiles, audio device picker, history
- **Phase 3**: Tauri Mobile (keep transcription.ts pure fetch — no Tauri imports)

## Critical Gotchas (See Plan for Full Details)
- Clipboard restore timing: 500ms delay, binary clipboard preserved
- Whisper cold start: 2-5s first call; 1-retry with 3s timeout built-in
- Electron app injection (VS Code, Slack): clipboard paste is the only reliable method
- Windows distribution: EV code signing cert required (~$400/yr)
- `set_ignore_cursor_events(true)` required on overlay or it blocks mouse events
