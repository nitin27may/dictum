# Wisper

Hold a hotkey, speak, release — your words appear wherever your cursor is. Wisper is a macOS menu-bar app that captures audio, transcribes it via OpenAI or Azure OpenAI Whisper, and injects the text directly into the focused application.

Built with [Tauri 2](https://tauri.app/) (Rust + React/TypeScript).

---

## How it works

1. Hold **Alt+Space** (configurable) for at least 200ms — recording starts
2. Speak — a waveform overlay shows at the bottom of your screen
3. Release — audio is encoded and sent to Whisper; text is injected at the cursor
4. Short taps (<200ms) are ignored and the keypress is passed through to the active app

The entire hot path (record → transcribe → inject) runs in Rust. The overlay window only receives lightweight state events for animation.

```
IDLE → [hold ≥ 200ms] → RECORDING → PROCESSING → SUCCESS | ERROR → IDLE
     → [tap < 200ms]  → keypress replayed → IDLE
```

---

## Features

- **Global hotkey** — works system-wide, including in terminals, browsers, Electron apps
- **Always-on-top overlay** — transparent, click-through, positioned above the Dock
- **Dual provider** — OpenAI Whisper or Azure OpenAI, switchable per session
- **Smart Keywords** — say `rephrase as email` or `rewrite as bullet points` mid-dictation; GPT reformats the text before injection
- **Autostart** — optional login item via macOS LaunchAgent
- **Tray-only** — no Dock icon; app lives in the menu bar
- **60-second cap** — hard limit with a T-10s countdown in the overlay

### Smart Keywords

When enabled in Settings → Smart Keywords, trigger phrases anywhere in your dictation invoke GPT to reformat the result before it is typed:

| Say | Result |
|-----|--------|
| `rephrase as email` | Formatted email with greeting, body, sign-off |
| `rephrase as bullet points` | Bulleted list |
| `rewrite as slack message` | Concise, semi-casual workplace tone |
| `format as professional` | Polished, clear prose |
| `rephrase as summary` | 1–3 sentence condensation |
| `format as code comment` | Brief, technical, explains the "why" |

The trigger phrase is stripped from the transcription before the clean text is sent to GPT.

---

## Requirements

- macOS 13 Ventura or later
- Rust 1.93+ (`rustup update stable`)
- Node.js 20+
- **Accessibility permission** — required for text injection via AppleScript (System Settings → Privacy & Security → Accessibility)
- **Microphone permission** — prompted on first use

---

## Getting started

### Clone and install

```bash
git clone https://github.com/your-org/wisper.git
cd wisper
npm install
```

### Configure API keys

**Option A — environment variable (dev)**

Create `.env.local` in the project root:

```env
# OpenAI
VITE_OPENAI_API_KEY=sk-...

# Azure OpenAI (optional — overrides provider auto-selection)
VITE_AZURE_ENDPOINT=https://your-resource.openai.azure.com
VITE_AZURE_API_KEY=your-azure-key
VITE_AZURE_WHISPER_DEPLOYMENT=whisper
VITE_AZURE_GPT_DEPLOYMENT=gpt-4o-mini
VITE_AZURE_API_VERSION=2024-02-01
```

**Option B — Settings UI (runtime)**

Open the tray icon → Settings → API tab. Keys are persisted via the Tauri store plugin (`wisper-settings.json`).

### Run in development

```bash
npm run tauri dev
```

### Build for distribution

```bash
npm run tauri build
```

The `.dmg` and `.app` bundle appear in `src-tauri/target/release/bundle/`.

---

## Project structure

```
wisper/
├── src/                          # React/TypeScript frontend
│   ├── App.tsx                   # Window routing + settings/API sync to Rust
│   ├── components/
│   │   ├── Overlay/              # Recording overlay UI (waveform, states)
│   │   └── Settings/             # Settings window (API config, hotkey)
│   ├── hooks/
│   │   ├── useRecordingFlow.ts   # Listens to Rust state events, drives Zustand
│   │   └── useAudioLevels.ts     # Real-time audio level events → waveform
│   ├── services/
│   │   └── transcription.ts      # Pure fetch — OpenAI / Azure Whisper POST
│   ├── store/
│   │   ├── recordingStore.ts     # Zustand state machine (IDLE / RECORDING / ...)
│   │   └── settingsStore.ts      # Settings with Tauri store persistence
│   └── types/
│       └── settings.ts           # Zod schemas + TypeScript types
│
└── src-tauri/src/                # Rust backend
    ├── main.rs                   # Entry point
    ├── lib.rs                    # App builder, tray, window positioning, command registration
    ├── flow.rs                   # Full recording lifecycle (press → stop → transcribe → inject)
    ├── keywords.rs               # Smart keyword detection and GPT rephrase
    ├── audio/
    │   ├── capture.rs            # cpal stream, RMS level events, 60s buffer
    │   └── encoder.rs            # f32 PCM → WAV bytes (hound)
    ├── hotkey/mod.rs             # Global shortcut registration (main thread)
    ├── injection/
    │   └── macos.rs              # Clipboard set + Cmd+V paste via AppleScript
    └── commands/
        ├── audio_commands.rs     # start_recording, stop_recording, get_audio_devices
        ├── injection_commands.rs # inject_text, check_accessibility_permission
        └── settings_commands.rs  # register_hotkey, get_platform, set_api_config
```

---

## Architecture

### Rust owns the hot path

The recording lifecycle runs entirely in Rust (`flow.rs`):

```
hotkey press
  └─ wait 200ms (tap-through guard)
       └─ start cpal mic stream
            └─ emit audio-level events → waveform UI
hotkey release
  └─ stop stream, collect Vec<f32> samples
       └─ encode WAV (hound, 16kHz mono)
            └─ POST to Whisper API (reqwest multipart)
                 └─ [optional] Smart Keywords → GPT rephrase
                      └─ inject text (clipboard + Cmd+V)
                           └─ emit success/error → overlay UI
```

Moving the hot path to Rust eliminates fragile async IPC setup in a hidden overlay window.

### Text injection

Text is injected via clipboard paste — the only reliable method across all macOS apps including Electron apps (VS Code, Slack, Figma):

1. Backspace — removes the non-breaking space macOS inserts when `Option+Space` fires
2. `pbcopy` — writes transcription to clipboard
3. `osascript` — sends `Cmd+V` to the frontmost application

The clipboard is not saved/restored to avoid triggering clipboard manager apps (Alfred, Paste, etc.).

### Hotkey architecture

`tauri-plugin-global-shortcut` registers on the Tauri main thread — a macOS requirement. Raw key events (press + release) arrive via a dedicated rdev thread and are forwarded through a channel. Hold detection compares keydown timestamp to keyup; presses under 200ms are replayed as-is.

### Audio

Audio is captured at the device's native sample rate and resampled to 16kHz mono f32 PCM before WAV encoding. The buffer is hard-capped at 60 seconds (~3.8MB).

---

## Configuration

All settings are persisted to `~/Library/Application Support/com.wisper.app/wisper-settings.json`.

| Setting | Default | Description |
|---------|---------|-------------|
| Hotkey | `Alt+Space` | Global push-to-talk trigger |
| Provider | `openai` | `openai` or `azure` |
| Whisper model | `whisper-1` | OpenAI model name |
| GPT model | `gpt-4o-mini` | Used for Smart Keywords rephrasing |
| Smart Keywords | disabled | Voice-triggered GPT reformatting |
| Launch at login | disabled | macOS LaunchAgent autostart |

---

## Development

```bash
# Dev server + Tauri app
npm run tauri dev

# TypeScript type check (no build)
npm run type-check

# Rust check (from src-tauri/)
cargo check

# Run Rust tests
cargo test
```

### Environment variables

| Variable | Purpose |
|----------|---------|
| `VITE_OPENAI_API_KEY` | OpenAI API key (dev fallback) |
| `VITE_AZURE_ENDPOINT` | Azure OpenAI resource URL |
| `VITE_AZURE_API_KEY` | Azure subscription key |
| `VITE_AZURE_WHISPER_DEPLOYMENT` | Azure Whisper deployment name |
| `VITE_AZURE_GPT_DEPLOYMENT` | Azure GPT deployment name |
| `VITE_AZURE_API_VERSION` | Azure API version (default `2024-02-01`) |

---

## Known gotchas

**Accessibility permission required** — text injection uses AppleScript (`System Events keystroke`). If the permission is not granted, the overlay will show an error after transcription. Grant it in System Settings → Privacy & Security → Accessibility.

**Clipboard is briefly overwritten** — during injection there is a ~500ms window where the clipboard contains the transcribed text. Binary clipboard contents (images, files) are not preserved — avoided intentionally to prevent double-paste from clipboard managers.

**Whisper cold start** — the first API call after a period of inactivity can take 2–5 seconds. One automatic retry with a 3-second delay is built in.

**Alt+Space on macOS** — Option+Space types a non-breaking space in the frontmost app before the global shortcut fires. Wisper sends a Backspace via AppleScript immediately on injection to remove it.

**Windows distribution** — requires an EV code signing certificate for SmartScreen clearance (~$400/year). See [`docs/windows-code-signing.md`](docs/windows-code-signing.md).

---

## Tech stack

| Layer | Technology |
|-------|-----------|
| Desktop runtime | Tauri 2.x |
| Backend | Rust (tokio async, reqwest, cpal, hound) |
| Frontend | React 18, TypeScript strict, Tailwind CSS, Framer Motion |
| State | Zustand 5 |
| Validation | Zod |
| AI | OpenAI Whisper, GPT-4o-mini (optional) |
| AI (Azure) | Azure OpenAI Whisper + GPT deployments |
| Persistence | tauri-plugin-store |
| Hotkey | tauri-plugin-global-shortcut + rdev |

---

## License

MIT
