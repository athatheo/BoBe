# Voice architecture

Mode B only: the Swift client owns speech recognition (via FluidAudio on
Apple Neural Engine), the Rust daemon owns LLM + text-to-speech (Kokoro via
sherpa-onnx). Transcripts flow over WebSocket, not audio.

Status as of M6.B: English + Mandarin ship; Spanish / Greek / Korean /
Japanese are surfaced in the language picker but marked "(coming soon)"
until those locales are wired up (Qwen3 itself already supports them).

## Per-language engine matrix

| Language | Status | STT engine | EOU source | Model size | macOS min |
|---|---|---|---|---|---|
| English (`en`) | ✅ ships | FluidAudio `StreamingEouAsrManager` (Parakeet EOU 120M) | Built-in (ANE-tuned debounce) | ~600 MB | 14 |
| Mandarin (`zh`) | ✅ ships | FluidAudio `Qwen3StreamingManager` + `VadManager` | VAD-driven silence timer | ~1.75 GB | **15** |
| Spanish (`es`) | 🕒 planned | Qwen3 (same engine, just flip the config) | VAD-driven | — | 15 |
| Greek (`el`) | 🕒 planned | Qwen3 | VAD-driven | — | 15 |
| Korean (`ko`) | 🕒 planned | Qwen3 | VAD-driven | — | 15 |
| Japanese (`ja`) | 🕒 planned | Qwen3 | VAD-driven | — | 15 |

Engine selection in `VoicePipeline` is a simple switch on the effective
language:

```swift
private var activeStt: any VoiceSttEngine {
    switch self.effectiveLanguage {
    case "en": return self.parakeetStt
    default:  return self.qwen3Stt
    }
}
```

`effectiveLanguage` is `sessionSttLanguage ?? activeSttLanguage`. The
session value is captured at `connect()` so the engine doesn't swap under
live audio when settings change mid-call; `activeSttLanguage` mirrors
`DaemonSettings.voiceSttLanguage` and is updated by `refreshDaemonState()`.

## Lifecycle, deps, and on-disk layout

### Deps the user needs acquired

| Dep | Size | Lives in | Owner | Required for |
|---|---|---|---|---|
| `bobe-daemon` binary | ~50 MB | `.app/Contents/MacOS/` | Bundled at build | Everything |
| Kokoro TTS model | ~340 MB | `~/.bobe/models/kokoro-multi-lang-v1_0/` | Daemon (via `/voice/install`) | TTS playback |
| Filler library | RAM only | n/a | Daemon synth at boot from Kokoro | Tool fillers / TTFT bridge |
| FluidAudio Parakeet EOU | ~600 MB | `~/Library/Application Support/FluidAudio/Models/parakeet-eou-streaming/` | Swift app via FluidAudio SDK | English STT |
| FluidAudio Qwen3-ASR + Silero VAD | ~1.75 GB | `~/Library/Application Support/FluidAudio/Models/qwen3-asr-0.6b-coreml/` + `silero-vad-coreml/` | Swift app via FluidAudio SDK | Mandarin (or any non-English) STT |
| Mic permission | OS-level | TCC | macOS | Recording |
| `voice.enabled = true` | n/a | `~/.bobe/config.toml` | User setting | Master gate |

Two caches in different roots, reflecting where ownership lives. Kokoro is
daemon-side because TTS runs there and the daemon must stay cross-platform.
FluidAudio is Mac-only and the SDK manages its own cache for cross-app
sharing — we don't override `ModelRegistry.baseURL`.

### Boot flow

1. **First launch** — `SetupWindowManager.isOnboardingCompleted == false`.
   `WelcomeWizard` runs. The Voice step (`VoiceSetupStepView`) kicks off
   both downloads in parallel: `DaemonClient.startVoiceInstall()` (Kokoro)
   and `VoicePipeline.ensureSttLoaded()` (Parakeet via FluidAudio SDK).
   Progress polls every 500ms; the wizard's "Continue" enables only when
   `allReady` (daemon `installed.allPresent` AND `sttStatus == .ready`).

2. **Subsequent launches** — `BoBeApp.startApp()` skips the wizard and
   calls `repromptVoiceSetupIfModelsMissing()`:
   - Fetches settings; if `voice.enabled == false`, no-op.
   - Polls `/voice/install/status.installed.allPresent` (Kokoro on disk).
   - Calls `FluidAudioModelPresence.isInstalled()` — a synchronous
     filesystem check for the Parakeet `.mlmodelc` sentinel files.
   - If either side missing → re-shows the wizard with the install step.

3. **Mic button readiness** — `MicButton.task` runs
   `VoicePipeline.bootstrapSttPresence()` unconditionally (no permission
   gate). This is a 3-syscall filesystem check that sets `sttStatus` to
   `.ready` instantly when Parakeet is cached. No 2-second spinner flicker
   on warm-cache launches. The actor's real `loadModels` warms ANE in the
   background; if the user taps before it's done, audio frames are dropped
   on the floor (≤2s warm time) — they're rare in practice because the
   user takes longer than 2s to open the overlay + tap.

4. **First mic tap (cold)** — `prewarm()` configures AVAudioEngine + VPIO,
   `connect()` opens the WS, sends `Hello{language, voice_id, speed}`.
   Daemon replies `HelloAck{voice_pack, playback_rate}` and `state(Listening)`.
   FluidAudio's `StreamingEouAsrManager` receives buffered audio, emits
   partials (→ `transcript_partial` over WS), fires `eouCallback` on natural
   pause → `transcript_final` over WS.

5. **Daemon turn lifecycle** — `voice/control.rs::handle_control_text`
   routes `TranscriptFinal` to `voice/modes/transcript_in::dispatch_from_control`
   which spawns a task running `voice/run_text_turn::run_text_turn` (the
   single convergence body: `try_begin_user_message`, `state(Thinking)`,
   `TranscriptFinal` echo, `state(Speaking)`, spawn filler watchdog +
   Kokoro task, call `RuntimeSession::handle_user_message_with_observer`,
   stream LLM deltas through `SentencePipeline` → Kokoro → Opus → WS,
   send `TtsEnd`, send `state(Listening)`).

### Failure modes

| What happens | Recovery |
|---|---|
| `voice.enabled = false` | Mic button hidden. No prompts. User toggles in Settings → Voice. |
| Mic permission denied | `MicButton` shows `mic.slash.circle.fill` with reduced opacity. Tap routes to System Preferences. |
| FluidAudio model missing post-onboarding | `BoBeApp.repromptVoiceSetupIfModelsMissing` opens the wizard automatically at boot. |
| FluidAudio model partially downloaded | `FluidAudioModelPresence.isInstalled` returns false → treated as missing → wizard re-runs the install (idempotent — FluidAudio SDK resumes / re-fetches missing files). |
| FluidAudio load hangs | 60s `withTimeout` in `VoicePipeline.ensureSttLoaded` → `sttStatus = .failed` → MicButton drops into needsSetup state with warning badge. |
| Daemon Kokoro missing | Settings → Voice → Models card shows "Missing — install required". Daemon `/voice/install` reinstalls. |
| Mode A leftover files on disk | Cleaned up at daemon boot (`cleanup_legacy_mode_a_files`); idempotent silent no-op once gone. |

## Wire protocol

Mode B only — no Binary audio from client to server.

### Client → daemon

```jsonc
// Handshake (sent once after WS connect)
{
  "type": "hello",
  "session_id": "voice-1234",
  "playback_rate": 24000,         // Kokoro native; validated server-side
  "voice_id": "af_bella",         // optional override (Kokoro persona slot)
  "speed": 1.0,                   // optional override
  "language": "en"                // optional, defaults to "en" server-side
}

// Streaming ASR output
{ "type": "transcript_partial", "turn_id": "voice_<uuid>", "text": "..." }
{ "type": "transcript_final",   "turn_id": "voice_<uuid>", "text": "..." }

// Playback control
{ "type": "barge_in", "ts_ms": 0, "playback_ms_played": 0 }
{ "type": "playback_ack", "chunk_id": 0, "played_ms": 0 }
{ "type": "wake", "phrase": "hey bobe", "score": 0.9, "ts_ms": 0 }
{ "type": "control", "action": "abort" | "mute" | "unmute" | "reset" }
```

### Daemon → client

```jsonc
// Handshake ack — sent once, before any state
{ "type": "hello_ack", "voice_pack": "af_bella", "playback_rate": 24000 }

// State machine + turn data
{ "type": "state", "phase": "listening" | "thinking" | "speaking" | "idle" | "...", "turn_id": "..." }
{ "type": "transcript_final", "turn_id": "...", "text": "..." }  // echo for UI
{ "type": "tts_end", "turn_id": "..." }
{ "type": "truncate", "turn_id": "...", "keep_ms": 0 }
{ "type": "error", "code": "...", "message": "..." }
```

Plus binary frames carrying TTS audio: `[8 bytes BE u64 chunk_id][1 byte flags][N bytes Opus]`.

## Module layout

### Rust daemon (`BoBeService/src/`)

```
speech/
├── protocol.rs                  # WS DTOs (Mode B only — no VoiceMode/capabilities)
├── tts.rs                       # trait TtsEngine
├── markdown_strip.rs            # TTS-input helper
├── sentence_buffer.rs           # TTS-input helper
└── providers/
    └── sherpa/
        └── kokoro_tts.rs        # LocalKokoroTts impl (Kokoro v1.0 multilingual)

voice/
├── cancel_phrases.rs            # English regex; runs on echoed client partials
├── context.rs                   # Per-WS bag of Arcs
├── control.rs                   # JSON message dispatcher (Hello + TranscriptPartial/Final + barge_in/control)
├── engines.rs                   # VoiceEnginesSnapshot { tts, filler_library }
├── filler_library.rs            # Pre-rendered Kokoro PCM clips
├── install_artifacts.rs         # Kokoro-only artifact catalog
├── install_service.rs           # Daemon download orchestrator
├── modes/
│   ├── mod.rs
│   └── transcript_in.rs         # Mode B convergence entry — spawns run_text_turn task
├── opus.rs                      # Outbound TTS Opus encoding
├── protocol_helpers.rs          # send_state/send_json/send_error/close_with_error
├── run_text_turn.rs             # SHARED convergence body — text → LLM → TTS → state cleanup
├── sentence_pipeline.rs         # markdown strip + sentence buffer → mpsc → Kokoro task
├── session.rs                   # VoiceSession + SessionVoiceConfig + VoiceDefaults
├── sinks.rs                     # Per-WS sink for hook-driven filler emission
├── telemetry.rs                 # Prometheus metrics
└── turn_flow.rs                 # barge-in cascade + handle_barge_in + abort_active_turn + MIN_WORDS_FOR_BARGE_IN
```

### Swift client (`BoBeMacUI/BoBe/Voice/`)

```
Voice/
├── MicButton.swift              # Mic icon stays a mic; switches on VoicePipeline.readiness
├── StopButton.swift             # Cancel-current-turn affordance
├── VoiceModelCard.swift         # Rich per-model card used in Settings → Voice → Models
├── VoiceModelRow.swift          # Minimal model row used in the wizard
├── VoicePartialCaption.swift    # Live partial transcript display
├── VoicePipeline.swift          # Orchestrator (audio capture, WS, FluidAudio drive, state)
├── VoiceProtocol.swift          # Wire DTOs mirroring speech/protocol.rs
├── VoiceReadiness.swift         # VoiceReadiness enum + readiness + refreshDaemonState / updatePermission
├── VoiceSttEngine.swift         # Protocol: loadModels / acceptAudio / finish / reset / cleanup
└── Providers/
    └── FluidAudio/
        ├── FluidAudioModelPresence.swift       # Parakeet FS presence check + observed-bytes progress
        ├── FluidAudioQwen3ModelPresence.swift  # Qwen3 + Silero VAD FS presence check + progress
        ├── FluidAudioStt.swift                 # Actor: StreamingEouAsrManager (Parakeet EOU, English)
        └── FluidAudioQwen3Stt.swift            # Actor: Qwen3StreamingManager + VadManager (multilingual)
```

### Swift settings + wizard

- `Features/Settings/VoicePanel.swift` — Language picker, pause sensitivity, persona, speed, Models section with `VoiceModelCard` rows + global Reinstall.
- `Views/Setup/VoiceSetupStepView.swift` — Wizard voice step. Triggers both Kokoro (daemon) + Parakeet (FluidAudio) installs in parallel; `Continue` enables only when both complete.
- `App/SetupWindowManager.swift` — Hosts `WelcomeWizard` in a panel.
- `App/BoBeApp.swift::repromptVoiceSetupIfModelsMissing` — Re-pops wizard at startup if voice is enabled but a model went missing.

## State signals today

`VoicePipeline` (`BoBeMacUI/BoBe/Voice/VoicePipeline.swift`) is the single
source of truth for four underlying inputs:

- `sttStatus: SttStatus` — Swift FluidAudio Parakeet readiness
  (`.notLoaded / .downloading / .ready / .failed`).
- `installSnapshot: VoiceInstallSnapshot?` — daemon-side Kokoro install
  progress + presence; populated by `refreshDaemonState()`.
- `permission: AVAuthorizationStatus` — mic permission; `MicButton` writes
  back via `updatePermission(_:)` after the system grant/deny dialog.
- `voiceEnabled: Bool?` — mirror of `DaemonSettings.voiceEnabled`; populated
  by `refreshDaemonState()`.

A computed `readiness: VoiceReadiness` (in `Voice/VoiceReadiness.swift`)
collapses the four into a flat enum:

| Case | When | Mic UI |
|---|---|---|
| `.preparing` | Before first `refreshDaemonState()` returns | dimmed, no badge (transient) |
| `.disabledByUser` | `voice.enabled == false` in settings | badge, tap → settings |
| `.permissionMissing` | Mic permission `.denied`/`.restricted` | `mic.slash.circle.fill`, tap → System Preferences |
| `.installing` | Either Kokoro or Parakeet actively downloading | spinner overlay, tap → settings to see per-model progress |
| `.modelsMissing` | One side missing on disk, no install running | badge, tap → wizard |
| `.failed(String)` | STT load timed out or threw | badge, tap → settings |
| `.ready` | All four green | normal mic affordance |

Priority: 1) disabledByUser, 2) permissionMissing, 3) preparing (still
loading inputs), 4) failed, 5) installing, 6) modelsMissing, 7) ready.

`VoicePipeline.init` subscribes to `.bobeWelcomeCompleted` and
`.bobeVoiceConfigChanged` notifications so the snapshot auto-refreshes when
the wizard finishes or Settings → Voice mutates state. Consumers (MicButton,
VoicePanel, VoiceSetupStepView, `BoBeApp.repromptVoiceSetupIfModelsMissing`)
read `pipeline.readiness` for high-level gating; per-model UIs (cards,
wizard rows) still drill into the underlying inputs for progress percent.

## Adding a new STT engine

The `VoiceSttEngine` protocol unlocks drop-in engines (WhisperKit, Apple
SpeechAnalyzer, Moonshine for Linux daemon, etc.). Two reference impls
exist — `FluidAudioStt` (built-in EOU) and `FluidAudioQwen3Stt`
(VAD-driven EOU) — covering both common shapes.

1. Add an actor under `BoBe/Voice/Providers/<Name>/` conforming to
   `VoiceSttEngine`: `loadModels(onPartial:onEou:) async throws`,
   `acceptAudio(_ buffer: sending AVAudioPCMBuffer) async throws`,
   `finish() async throws -> String`, `reset() async throws`,
   `cleanup() async`.
2. Add a presence helper modeled on `FluidAudioModelPresence` /
   `FluidAudioQwen3ModelPresence` — synchronous filesystem check against
   sentinel files in the expected cache layout.
3. Extend `VoicePipeline.activeStt` and `activeModelIsInstalled` switches
   to route to the new engine for the language(s) it covers.
4. Add a row to the Settings → Voice → Models section (or extend an
   existing one if the new engine reuses a cached bundle).
5. If models are large (>100MB), the wizard already handles them via
   `pipeline.ensureSttLoaded()` — no per-engine wizard branching needed.

## Sources

- FluidAudio: https://github.com/FluidInference/FluidAudio
- Parakeet EOU model: https://huggingface.co/FluidInference/parakeet-realtime-eou-120m-coreml
- Kokoro TTS: https://github.com/k2-fsa/sherpa-onnx
- M6 plan: `~/.claude/plans/iridescent-percolating-octopus.md`
