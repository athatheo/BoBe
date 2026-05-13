# BoBe Voice — Definitive Plan (May 2026)

> **Status**: authoritative. Supersedes `docs/voice-production-plan.md` and `docs/voice-pivot-mapping.md` (kept for archaeology). Grounded by three parallel May-2026 research agents (stack picks, UX patterns, architecture boundary) plus the prior 12+4 agent research.
>
> **Branch**: `voice-on-pivot` off `feat/copilot-sdk-pivot`. Spike preserved at tag `voice-spike-archive`.

---

## 1. Stack picks (May 2026)

| Layer | Pick | Variant | Why this, not alternatives |
|---|---|---|---|
| **STT** | sherpa-onnx 1.13.1 | **Moonshine base int8 (English, offline)** | We VAD-gate the buffer before STT, so we don't need true streaming — `OfflineRecognizer` is sufficient and simpler. Moonshine base is ~70MB, no silence hallucinations (vs Whisper encoder-decoder), MIT. Daemon-portable (Apple SpeechAnalyzer = Mac-only, FluidAudio Parakeet = Swift/ANE-only). If partial transcripts in the UI become a requirement later, swap to streaming Moonshine variant. |
| **TTS** | sherpa-onnx 1.13.1 | **Kokoro v1.0 (82M)**, voice slot id (default `af_bella`) | TTS Arena #1 open-weight (ELO ~1505, no v1.5/v2 exists). Apache-2.0. CPU-runnable. Chatterbox is GPU-only + autoregressive end-token bugs; XTTS-v2 is dead post-Coqui-shutdown Dec 2025; AVSpeechSynthesizer is Mac-only and not SOTA. |
| **Acoustic VAD** | sherpa-onnx 1.13.1 `VoiceActivityDetector` | **Silero v6.2.1 ONNX** (Feb 2025 release) | Silero v6.2.1 specifically addressed the short-inter-speech-silence miss case that TEN VAD's older-Silero benchmark claimed to beat. Silero is MIT (clean); TEN is Apache-2.0-with-additional-conditions + BSD-2 + BSD-3 (legal review burden). Silero ships in LiveKit Agents, Pipecat, Whisper.cpp — dozens of production agents. Same M1 RTF tier as TEN (~0.016). Daemon-side (co-locate with smart-turn so both see same audio frames). |
| **Semantic VAD** | `ort` 2.0.0-rc.12 directly | **smart-turn v3.1** (8MB int8 ONNX) | Q1 2026 retrained drop-in for v3.0. 12ms CPU on M-class. NOT in sherpa-onnx, so loaded directly via ort. Whisper mel preprocessing reimplemented in Rust. LiveKit's 135M SmolLM-v2 turn-detector is 17× bigger for marginal gain. |
| **Wake-word** | `ort` 2.0.0-rc.12 directly | **Custom-trained "Hey BoBe" openWakeWord ONNX** | Defaults are CC-BY-NC-SA (commercial-blocking). Picovoice Porcupine charges $6k/yr for custom. Train via Pipecat Colab notebook + piper-sample-generator (archived — mirror first). Client-side. |
| **LLM** | github-copilot-sdk | **0.1.x** (already in pivot Cargo.toml) | Native `session.abort()`, `DeliveryMode::{Immediate, Enqueue}`, `session.subscribe()`. No alternative needed. |
| **Codec** | `opus = "0.3"` (Rust) + swift-opus pinned revision (Swift) | 20ms frames, 24kbps VOIP | Industry standard for voice. |
| **AEC** | AVAudioEngine | **VPIO on both input + output nodes**, single engine | Apple's full-duplex stack. `voiceProcessingOtherAudioDuckingConfiguration` with `.min` for soft barge-in. Half-duplex (mute-mic-during-TTS) is non-standard in 2026. |

**Two onnxruntime instances** loaded at startup (sherpa-onnx-sys for STT/TTS/VAD, ort for smart-turn + wake-word). This is unavoidable — smart-turn isn't in sherpa-onnx. Memory cost: ~30MB extra for the duplicate runtime, acceptable.

**Total model bundle**: ~200MB (Moonshine base ~70MB + Kokoro 85MB + smart-turn 8MB + Silero v6.2.1 ~2MB + wake-word ~250KB + espeak-ng-data ~30MB).

**Latency budget P95 (STT-final → first TTS audio)**:
- TEN VAD per-frame: 10ms
- smart-turn v3.1 inference: 12-65ms
- STT first partial (Moonshine Small Streaming on M-class): 80-150ms
- STT final (cache reuse): ~100ms
- LLM TTFT (Copilot SDK): 300-600ms
- Kokoro TTS time-to-first-audio (sherpa-onnx CPU on M4 Pro): 250-500ms
- Localhost network: <10ms

**Total**: ~850-1150ms. **The ≤1.2s ceiling holds only when LLM TTFT ≤600ms.** If LLM is slower, pre-cached fillers (M4.5.4) save perceived latency, not stack tuning.

---

## 2. Architecture: client / daemon boundary

**Rule**: daemon owns logic + state + models; client owns audio I/O + low-latency hints.

```
┌────────────────── CLIENT (Swift macOS) ──────────────────┐
│                                                            │
│  Mic → AVAudioEngine VPIO ──┐                              │
│         (AEC + AGC + NS)    │                              │
│                              ▼                              │
│  RMS gate (cheap)  ─→ vad.hint (optional bandwidth-saver)  │
│  Opus encode (16k mono, 20ms VOIP)                         │
│                              │                              │
│  openWakeWord (always-on,   │                              │
│   ~1% CPU, "Hey BoBe")      │                              │
│   on-mic, never streams pre-trigger                        │
│                              │                              │
│         WebSocket frame  ────┼──── /voice/stream ────────► │
│                              │                              │
│                                                            │
│  Audio playback queue       ◄─── tts.chunk (24k Opus)     │
│   (AVAudioPlayerNode +      ◄─── filler.chunk             │
│    crossfade + ducking)     ◄─── truncate                  │
│                              │                              │
│  Playback position reporter ─→ playback.ack                │
│   (every 100ms in .speaking)                               │
│                              │                              │
│  Barge-in DETECTION (RMS    │                              │
│   + playback overlap)       ─→ barge_in                    │
│                              │                              │
│  State mirror (UI only)     ◄─── state (authoritative)     │
└────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌────────────────── DAEMON (Rust) ──────────────────────────┐
│                                                            │
│  WS handler at /voice/stream                              │
│   Opus decode → PCM 16k mono buffer                       │
│         │                                                  │
│         ▼                                                  │
│  TEN VAD (per-frame, ~10ms)  ←── authoritative            │
│   speech_start → emit state listening                     │
│   speech_end   → fire smart-turn                          │
│         │                                                  │
│         ▼                                                  │
│  smart-turn v3.1 (semantic, 12ms)                         │
│   P(complete) < 0.7? → keep listening                     │
│   P(complete) ≥ 0.7? → acquire UserMessageGuard, commit   │
│         │                                                  │
│         ▼                                                  │
│  STT (Moonshine Small Streaming, cache reuse)             │
│   final transcript                                         │
│         │                                                  │
│         ▼                                                  │
│  runtime_session.handle_user_message_with_observer        │
│   → workers.chat().send(ChatPrompt::text(transcript))     │
│   → session.subscribe() for ChatDelta::MessageDelta       │
│                                                            │
│  Observer at message_handler.rs:103 (currently |_| {}):   │
│   ┌──────────────────────────────────────┐                │
│   │ each MessageDelta text →             │                │
│   │   markdown_stripper.feed(running)    │                │
│   │   sentence_buffer.feed(clean)        │                │
│   │   sentence_tx.send(complete sentence)│                │
│   └──────────────────────────────────────┘                │
│                                                            │
│  Sentence consumer task:                                  │
│   per-sentence Kokoro spawn_blocking                      │
│   → Opus 20ms frame encode                                │
│   → WS tts.chunk                                          │
│                                                            │
│  Filler trigger (800ms TTFT timer + tool intent keying)   │
│   → cached Opus or LLM-nano filler → WS filler.chunk      │
│                                                            │
│  Stall watchdog (inter-token gap, exponential)            │
│                                                            │
│  Barge-in handler:                                        │
│   client barge_in event + TEN VAD confirmation +          │
│    min_words gate (3 during TTS, 1 idle) →               │
│    session.abort() + WS truncate{keep_ms} + clear queue   │
│                                                            │
│  Conversation persistence (via existing chat path,        │
│   reuses ConversationService)                             │
│                                                            │
│  Memory.md auto-injection (via BobeHooks::SessionStart;   │
│   voice gets memory context for free)                     │
└────────────────────────────────────────────────────────────┘
```

**Critical**: voice transcripts go through the **same `workers.chat()` session** as typed messages (date-keyed Chat WorkerClass). Same session id = unified history, memory.md auto-injection, MCP tools, conversation persistence. We do NOT add `WorkerClass::Voice`.

---

## 3. WS protocol surface (`/voice/stream`)

Single persistent WS. Binary = raw audio (Opus). Text = JSON control.

### Client → daemon (6 message types)

| Type | Payload | When |
|---|---|---|
| `hello` | `{session_id, capture_rate=16000, playback_rate=24000, codec="opus"}` | On connect |
| `audio.in` | binary Opus packet (20ms @ 16k) | Continuous during mic-active |
| `vad.hint` | `{kind: "speech_start"\|"speech_end", rms_dbfs, ts}` | Optional fast hint (bandwidth-saver, daemon authoritative) |
| `barge_in` | `{ts, playback_ms_played}` | Client detects local speech during TTS playback |
| `wake` | `{phrase, score, ts}` | Wake-word fires (M5.1) |
| `playback.ack` | `{chunk_id, played_ms}` every 100ms in `.speaking` | For server-side truncation math |

### Daemon → client (7 message types)

| Type | Payload | When |
|---|---|---|
| `state` | `{phase: idle\|listening\|thinking\|speaking\|cancelling, turn_id}` | Authoritative on every transition |
| `transcript.partial` | `{turn_id, text}` | Optional, post-Moonshine partial decode |
| `transcript.final` | `{turn_id, text}` | After smart-turn confirms + STT final |
| `tts.chunk` | binary Opus packet (20ms @ 24k) + `{chunk_id, turn_id}` header | Per Kokoro-synthesized sentence |
| `tts.end` | `{turn_id}` | All sentences flushed |
| `filler.chunk` | binary Opus + flag `preemptible: true` | 800ms TTFT or stall |
| `truncate` | `{turn_id, keep_ms}` | Post barge-in; client drops queued audio beyond keep_ms |
| `error` | `{code, message}` | Non-fatal warning |

Sample-rate strategy: **NO conversion at the boundary**. Uplink 16k (STT native), downlink 24k (Kokoro native). AVAudioEngine handles device-rate resampling internally on the client. Daemon closes WS with `rate_mismatch` if `hello` disagrees.

---

## 4. UX patterns to ship

| Pattern | Pick | Justification |
|---|---|---|
| **Endpointing** | TEN VAD acoustic + smart-turn v3.1 semantic. Session-scope adaptive delay (300ms–3000ms EMA per LiveKit pattern). Manual sensitivity slider as escape hatch. | No production product ships persisted per-user adaptive yet. Semantic VAD covers ~90% of the win. |
| **Barge-in** | 3-event canonical: cancel-LLM / truncate-transcript-to-played_ms / clear-tts-buffer. `min_words=3 during TTS, =1 idle` (Pipecat). | Still production canonical. |
| **Backchannel filter** | 3-word minimum during BoBe-speaking + 2s `false_interruption_pending` window (LiveKit `resume_false_interruption=True` pattern). | Filters "yeah", "uh-huh", coughs. |
| **Hammering pushback** | Ack+FIFO-merge within 1.5s → escalate after 3× in 5s with "let me finish this one first". Via Copilot SDK `Immediate` mode for merge, `Enqueue` for queueing. | Pipecat v0.0.99 default. Per-persona phrasing via system prompt, NOT hardcoded library. |
| **BTW classifier** | Tri-state on side-channel utterance: cancel ("wait/never mind") → `abort()`; addendum ("also/and X") → `Immediate`; ack ("ok/thanks") → drop or `Enqueue`. | Cheap regex/rule classifier on STT final. |
| **Filler audio** | Hybrid: 20-phrase cached library (pre-built to repo) for 800ms TTFT trigger + LLM-nano contextual filler for tool calls >1.5s. ElevenLabs Jan 12 2026 pattern (`use_llm_generated_message` analog). Single trigger per turn cap. | Cached for ack moments (<100ms latency), LLM-nano for tool-context. |
| **Stall watchdog** | 500ms inter-token soft (cached filler) → 2s audible re-ack → 5s "still working, want me to try a different approach?" | Exponential backoff. Max 3 stalls per turn. |
| **Wake-word UX** | "Hey BoBe" via openWakeWord (custom-trained). Visual chip in overlay + 80ms chirp (mutable). 2s "not me" dismiss button. Battery-aware: disable on battery <30%. | 3-syllable wins for prosody. Local-only is table-stakes 2026. |
| **Voice modes** | Conversational (default, full-duplex), Dictation (no LLM response, text-only capture), Focus (mic muted, PTT only). | Aligns with Claude Code voice mode (Mar 2026) input-only pattern as one option. |
| **Persona pushback** | System-prompt-driven. Include 3-5 verbatim pushback example phrases per soul in the prompt. NOT a hardcoded phrase library. | LiveKit "behaviors you can actually hear" guidance — phrasing must match soul persona. |
| **Unified UI** | Voice transcripts render in the existing overlay chat history as normal messages (same `ChatMessage` model, sender `.user` or `.bobe`). Source flag for icon. | ChatGPT Nov 2025 unified voice+text confirms this is the right shape. BoBe overlay already does it. |

---

## 4.5. System architecture (Rust + Swift module layout)

### Rust daemon (post-Phase 0)

```
BoBeService/src/
├── speech/
│   ├── mod.rs                    # re-exports
│   ├── engines/                  # one trait + one impl per file
│   │   ├── mod.rs
│   │   ├── stt.rs                # SttEngine trait
│   │   ├── tts.rs                # TtsEngine trait
│   │   ├── vad.rs                # AcousticVad trait
│   │   ├── turn.rs               # SemanticTurn trait
│   │   ├── moonshine.rs          # SherpaMoonshineStt impl
│   │   ├── kokoro.rs             # SherpaKokoroTts impl
│   │   ├── silero.rs             # SherpaSileroVad impl (Silero v6.2.1)
│   │   └── smart_turn.rs         # OrtSmartTurn impl (v3.1, mel preproc in-Rust)
│   ├── protocol.rs               # 13-message WS DTOs
│   ├── session.rs                # VoiceSession actor (per-WS connection)
│   ├── sentence_buffer.rs        # LiveKit `_basic_sent.split_sentences` port (~50 LOC)
│   ├── markdown_strip.rs         # streaming stripper (running-buffer + diff)
│   └── pipeline/
│       ├── mod.rs
│       ├── vad_pipeline.rs       # frame → Silero → smart-turn → commit-ready signal
│       └── tts_pipeline.rs       # sentence → Kokoro spawn_blocking → Opus → WS
├── api/handlers/voice.rs         # thin WS upgrade; delegates to VoiceSession
├── model_manager/                # new module (NOT extending binary_manager)
│   ├── mod.rs
│   ├── source.rs                 # ModelSource trait
│   ├── hf.rs                     # HuggingFaceSource via hf-hub
│   └── installer.rs              # atomic install + sha verify + `current` symlink
├── app_state.rs                  # +voice_stt, voice_tts, voice_vad, voice_smart_turn,
│                                 #  voice_session_handle (ArcSwap<Option<VoiceSessionHandle>>)
└── runtime/
    ├── message_handler.rs        # +handle_message_with_observer (text passes |_| {})
    └── session.rs                # +handle_user_message_with_observer
```

### Swift client (post-Phase 0)

```
BoBeMacUI/BoBe/Voice/
├── VoicePipeline.swift           # @MainActor Observable; owns state, WS, audio engines
├── VoiceWS.swift                 # URLSessionWebSocketTask wrapper + protocol enc/dec
├── AudioCapture.swift            # AVAudioEngine VPIO input + RMS hint + Opus encode
├── AudioPlayback.swift           # AVAudioPlayerNode queue + crossfade + ducking
├── VoiceState.swift              # State enum + transition rules
├── MicButton.swift               # Overlay button view (integrates with composerSection)
└── VoiceProtocol.swift           # 13-message Codable structs
```

### Voice session actor (per WS connection, Rust)

```rust
struct VoiceSession {
    session_id: String,
    pcm_buffer: Vec<f32>,                          // 16k mono PCM accumulator
    vad_state: VadState,                           // listening / capturing / committed
    silero: Arc<dyn AcousticVad>,
    smart_turn: Arc<dyn SemanticTurn>,
    stt: Arc<dyn SttEngine>,
    tts: Arc<dyn TtsEngine>,
    runtime_session: Arc<RuntimeSession>,          // for try_begin_user_message + chat path
    sentence_tx: mpsc::Sender<String>,             // → tts_pipeline task
    current_turn: Option<TurnState>,               // turn_id, _guard, handler JoinHandle
}
```

### State machine (daemon-authoritative; client mirrors for UI)

```
Idle ──speech_start (Silero)─▶ Listening
   ▲                              │
   │                              ▼
   │              silence (700ms) + smart_turn(complete)
   │                              │
   │              smart_turn(incomplete) → Listening (reset)
   │                              │
   │                              ▼
   │                          Capturing ──STT done──▶ Thinking
   │                                                     │
   │                                                     ▼
   │                                  first sentence ready → Speaking
   │                                                     │
   └──────────── all sentences done ────────────────────┘
                                                         │
client.barge_in + min_words≥3 (during Speaking) ────────▶ Cancelling
                                                         │
                                                drain + abort ─▶ Idle
```

### Single-flight invariant

- `UserMessageGuard` acquired by VoiceSession at smart-turn confirmation (Capturing → Thinking transition)
- Held through STT + LLM stream + last TTS Opus frame sent
- Dropped on `tts.end` emission OR on barge-in cancel cleanup
- `barge_in` event does NOT acquire the guard (the cancelling task already holds it)

### Barge-in three-event protocol on pivot

1. Drop the `Stream<ChatDelta>` returned from `worker.send()` → AbortGuard at `workers/chat.rs:26-45` fires `session.abort()` automatically (no manual SDK call needed)
2. Send `truncate { turn_id, keep_ms = client_played_ms }` over WS → client drops queued audio beyond `keep_ms` and reports back its actual playback position
3. Daemon updates persisted assistant turn to `text.truncate(char_offset_at_played_ms)` so memory matches what the user actually heard

### Observer hook at `message_handler.rs:103`

Currently `|_| {}` for text chat. For voice, replace with:

```rust
move |delta_str: &str| {
    let clean = markdown_strip.feed(delta_str);    // running-buffer + diff
    for sentence in sentence_buffer.feed(clean) {
        let _ = sentence_tx.try_send(sentence);    // → Kokoro spawn_blocking task
    }
}
```

Text chat callers continue with `|_| {}`. Zero behavioral impact.

---

## 5. Code mapping onto pivot architecture

Confirmed from Phase -1 mapping (`docs/voice-pivot-mapping.md`):

| Pivot integration point | Voice usage |
|---|---|
| `runtime/response_streamer.rs:73-82` — `stream_chat_delta_response<F: FnMut(&str) + Send>` | Voice's sentence-buffering closure goes here. Text chat keeps `|_| {}` at `message_handler.rs:103`. |
| `runtime/session.rs:258-281` — `try_begin_user_message()` + `UserMessageGuard` | Voice acquires the same guard on smart-turn `speech_end`. Barge-in's `speech_start` does NOT take it (cancelling task holds it). |
| `copilot/hooks.rs:27-48` — `BobeHooks::SessionStart` | Memory.md auto-injects for free when voice routes through `workers.chat()`. |
| `copilot/registry.rs` — `WorkerRegistry::chat()` | Voice uses this. Reuse the existing date-keyed daily-rotation Chat session — do NOT add `WorkerClass::Voice`. |
| `copilot/workers/chat.rs:26-45` — `AbortGuard` | Drop the `Stream<ChatDelta>` returned from `worker.send()` → guard's `Drop::drop` fires `session.abort()` automatically. Barge-in just aborts the spawned message-handler task. |
| `services/conversation_service.rs:34-141` | Voice user turns use `append_user_turn_or_create_active`; assistant replies via existing `add_turn` (sync) or `begin_proactive_stream` pattern (streaming) |
| `binary_manager/{mod,download,extract}.rs` | DON'T refactor for ML models. Create sibling `model_manager/` module mirroring the patterns (chunked download, atomic rename, `watch::Sender<DownloadProgress>`). |
| `app_state.rs:20-36` | Voice fields: `voice_stt`, `voice_tts`, `voice_vad`, `voice_smart_turn`, `voice_session_handle: ArcSwap<Option<VoiceSessionHandle>>` |
| `api/router.rs:36-138` | New route: `.route("/voice/stream", get(handlers::voice::handle))` using `axum::extract::ws::WebSocketUpgrade` |
| `Features/Settings/SettingsWindow.swift` | New `VoicePanel.swift` in PREFERENCES group |
| `Views/Setup/WelcomeWizardSteps.swift:166-287` | Extend `PermissionsStepView` for mic permission (currently screen-capture only) |
| `Stores/BobeStore.swift` | Voice transcript append via existing `messages.append` pattern (add `source: .typed | .voice` field on ChatMessage) |

---

## 6. Implementation phases & commit sequence

### Phase 0 — Foundations (already partial; complete and commit)

| # | Commit | Status | Scope |
|---|---|---|---|
| **0.a** | speech module + sherpa-onnx deps + bootstrap loader | code in worktree, cargo check passing | Port `speech/{mod,stt,tts,protocol,local_sherpa,local_kokoro}.rs`. Use `sherpa-onnx = "1.13"`. **AMENDMENT**: bump Moonshine to `sherpa-onnx-moonshine-base-en-int8` → `sherpa-onnx-moonshine-small-en-int8` once base path verified (config supports both; just point at the small streaming variant). Add `OnnxVoiceVad` (TEN) and `OnnxSmartTurn` engine traits. Bootstrap loads all four engines. |
| **0.b** | Voice WS handler shell + opus + protocol | next | `api/handlers/voice.rs` shell with the 13-message protocol. Route `/voice/stream`. Opus encode/decode. Connection lifecycle. No LLM yet; just echo transcript back. |
| **0.c** | Daemon VAD pipeline | | Wire TEN VAD per-frame; emit `state {listening}` on speech_start. Hook smart-turn on acoustic silence; commit only on P(complete) ≥ 0.7. Single-flight guard acquisition on speech_end. |
| **0.d** | Chat-pipeline observer + sentence streaming | | Refactor `MessageHandler::handle_message` to take `Option<observer>`. Voice variant `handle_message_with_observer`. Sentence buffer port from LiveKit `_basic_sent.split_sentences` (~50 LOC). Markdown stripper (running-buffer + diff). Kokoro per-sentence `spawn_blocking` + mpsc(4). Voice handler streams `tts.chunk` Opus frames preserving order. |
| **0.e** | Swift voice client | | `BoBeMacUI/BoBe/Voice/{VoicePipeline,MicButton}.swift`. AVAudioEngine VPIO on input AND output nodes (single engine). Opus encode/decode via swift-opus pinned. WS client, state mirror, audio playback queue with crossfade. RMS hint sender. Playback position reporter. NSMicrophoneUsageDescription in project.yml. |
| **0.f** | Welcome wizard mic step + Voice settings pane | | Extend `PermissionsStepView` for mic. New `Features/Settings/VoicePanel.swift` in PREFERENCES (mirrors EnginePanel.swift pattern). Settings DTO additions in `Models/SettingsTypes.swift` + Rust `config.rs`. |

**Gate for Phase 0**: end-to-end voice turn works on pivot. Click mic, speak, see transcript in overlay history, hear reply. Single-flight prevents text+voice overlap. Memory auto-injects. Latency P95 < 1.5s.

### Phase 1 — Production polish

| # | Commit | Scope |
|---|---|---|
| **1.a** | Model installer | New `model_manager/` module (NOT `binary_manager` extension). `hf-hub` for HuggingFace downloads with SHA pinning. Atomic install via `models/<id>/<version>/.complete` + `current →` symlink. `VoiceInstallService` mirroring `OllamaInstallService`. POST `/voice/install` endpoint + welcome wizard step. |
| **1.b** | Filler audio + 800ms TTFT trigger | 20-phrase cached library pre-built to `BoBeService/assets/fillers/` via `cargo run --bin gen_fillers` (committed Opus). Decode-once at boot. Trigger T1=800ms, T2=2500ms escalation. Intent-keyed on `ChatDelta::ToolStart`. Single per turn cap. Crossfade 80ms. |
| **1.c** | Barge-in 3-event | `min_words=3 during TTS, =1 idle`. False-interruption-pending state with 2s timeout. Truncate math via `playback.ack`. `session.abort()` via task drop. VPIO ducking config (`.min` advanced ducking). |
| **1.d** | Stall watchdog | Tokio `select!` + `Sleep::reset`. 500ms→2s→5s exponential. Separate stall-filler pool. Tool-call-aware (4s tool-budget timer suppresses inter-token watchdog). |
| **1.e** | BTW classifier + hammering | Tri-state classifier on side-channel STT. Ack+FIFO-merge within 1.5s. Hammering escalation after 3× in 5s. Persona-aware via system prompt addendum. |
| **1.f** | Observability | Rust `tracing` JSONL at `~/Library/Logs/BoBe/voice-trace.jsonl`. Swift `OSSignposter`. Turn_id propagation. HUD latency pill. Ring-buffer incident dump opt-in. Zero outbound. |
| **1.g** | Voice testing harness | 3-tier: unit (fake_stt/tts/vad), per-PR (WER on LibriSpeech 50, UTMOS+NISQA, smart-turn-data-v3.1-test ≥85%), nightly (synthetic conversation replay, latency SLA gate). |

### Phase 2 — M5 features

| # | Commit | Scope |
|---|---|---|
| **2.a** | Wake-word | Custom-trained openWakeWord "Hey BoBe" (Colab notebook + mirrored piper-sample-generator). Stage-2 verifier via sherpa-onnx KWS on 1.5s post-trigger buffer. Battery-aware (off <30%). macOS sleep handling via `IORegisterForSystemPower`. |
| **2.b** | Proactive voice routing | `AppState.voice_session_handle`. Proactive observer feeds voice TTS sink. `try_begin_proactive_message` guard parallel to user. |
| **2.c** | Voice modes | Settings: Conversational / Dictation / Focus. Dictation = no LLM, text-only capture. Focus = mic muted, mic-button PTT only. |

---

## 7. Per-milestone gates

| Phase | Gate |
|---|---|
| 0.a | Speech module compiles. sherpa-onnx loads at startup. `cargo check` green. |
| 0.b | WS connects, accepts audio frames, echoes transcript. No LLM yet. |
| 0.c | TEN VAD trips silence. smart-turn confirms or rejects. STT runs only on confirmed turn-end. |
| 0.d | End-to-end voice turn: speak → reply audio. Sentences arrive in order. Markdown not spoken. |
| 0.e | Swift client wires up. Full-duplex with VPIO. UI mirrors daemon state. |
| 0.f | Welcome wizard mic step gates mic permission. Voice settings PATCH round-trips. |
| 1.a | Model install resumable across crashes. SHA mismatch surfaces. |
| 1.b | Filler fires 800±50ms when LLM artificially delayed. No filler when LLM fast. |
| 1.c | Barge-in halt <200ms p95. "Coughing" doesn't interrupt. "Wait, never mind" does. |
| 1.d | Stall filler at 800ms gap. No filler when sentence still playing. |
| 1.e | Hammering escalation at 2/3 commits visible in logs. BoBe doesn't talk over user. |
| 1.f | Every WS turn has turn_id in logs. HUD latency pill renders. |
| 1.g | CI runs tier-1 unit on every commit. Smart-turn ≥85% accuracy gate. |
| 2.a | FPPH <0.2/h on 1h dogfood. Doesn't fire on "OK Buddy". |
| 2.b | Proactive turn plays through voice if voice session active. |
| 2.c | Dictation mode captures without LLM round-trip. |

---

## 8. Risks & mitigations

| Risk | Mitigation |
|---|---|
| sherpa-onnx Kokoro TTFA on CPU lands 1-2s on M-class | Pre-cached fillers absorb. Fall back to `ort 2.0` + direct Kokoro-82M ONNX if needed. **Measure on M4 Pro before Phase 1.** |
| Moonshine Small Streaming endpoint API maturity | sherpa-onnx exposes endpoint API on all streaming models. **Verify works for Small Streaming variant before Phase 0.c.** Fallback: streaming zipformer English (still in sherpa-onnx). |
| TEN VAD license less clear than Silero MIT | Confirm exact terms on TEN-framework/ten-vad before commit. Swap to Silero v5 if unclear (operationally equivalent, +50ms tail latency). |
| sherpa-onnx not bundled prebuilt by default — needs build | `sherpa-onnx 1.13` features are `default, shared, static` only — no `download-binaries`. **Test cold build time on Linux CI before committing.** May need to vendor or use sherpa-onnx-sys directly. |
| Parakeet streaming via sherpa-onnx might ship later | Watch Issue #2918. If true streaming Parakeet ships, swap STT default — better WER, same Rust dep tree. |
| Apple SpeechAnalyzer historically round-tripped to cloud for some locales | Confirmed offline in macOS 26 docs but **measure with Little Snitch before exposing as engine option**. Not in daemon path; only for future client-side mode. |
| Pivot is moving (74+ commits ahead of main) | Phase 0 fast (<1 week) to land voice on pivot. Voice + pivot move forward together after. |
| `respond_to_message` is private (`message_handler.rs:71`) | Need to add `_with_observer` variant. Small refactor. |
| Welcome wizard already has 5 steps + Copilot CLI presence check | Add `microphonePermission` step between `permissions` and `done`, OR extend `PermissionsStepView` to handle both. |
| openWakeWord defaults are CC-BY-NC-SA | Custom training mandatory. Mirror piper-sample-generator (archived) to BoBe-controlled repo first. |
| SSE EventQueue bounded at 100 | Voice WS uses its own connection — no contention with SSE queue. |

---

## 9. What we're explicitly NOT doing (v1)

- **Persisted per-user adaptive endpointing** — no production product ships it; semantic VAD covers most of the win.
- **Vision-in-voice** (camera + speech same channel) — out of scope for v1.
- **Voice cloning** (Personal Voice, XTTS-v2-style) — Kokoro doesn't clone; defer.
- **Multi-language voice** — English-only on Phase 0 (Moonshine is English-only). Kokoro supports multilingual but voice picker is English defaults. Defer.
- **Cloud STT/TTS fallbacks** (Deepgram, ElevenLabs) — local-first stance. Defer until daemon-on-remote pattern is real.
- **Phone/SIP voice** — out of scope.
- **NotebookLM-style audio overview** (two BoBe instances chatting) — fun but not v1.
- **Apple SpeechAnalyzer / FluidAudio Parakeet as primary STT** — Mac-only ties us off Linux. Keep for future Mac-premium path.
- **HTTP/2 RST_STREAM hack for LLM abort** — pivot has `session.abort()` native, no workaround needed.

---

## 10. References

### Memory files (persist across sessions)
- `~/.claude/projects/-Users-john-Repos-bobrust/memory/reference_voice_production_patterns.md` — industry-canonical patterns from 12-agent research
- `~/.claude/projects/-Users-john-Repos-bobrust/memory/reference_voice_implementation_findings.md` — implementation specifics (some BoBe paths reflect spike branch; rely on this doc for pivot paths)

### Companion docs in this repo
- `docs/voice-pivot-mapping.md` — file-by-file pivot integration points (Phase -1 output)
- `docs/voice-production-plan.md` — earlier plan, superseded by this doc

### External sources (May 2026)
- **STT**: [Moonshine GitHub](https://github.com/moonshine-ai/moonshine), [Northflank STT 2026 benchmarks](https://northflank.com/blog/best-open-source-speech-to-text-stt-model-in-2026-benchmarks), [Modelslab Moonshine vs Whisper](https://modelslab.com/blog/audio-generation/moonshine-vs-whisper-asr-real-time-speech-2026), [sherpa-onnx Issue #2918 — Parakeet streaming](https://github.com/k2-fsa/sherpa-onnx/issues/2918), [Apple SpeechAnalyzer benchmarks](https://www.macstories.net/stories/hands-on-how-apples-new-speech-apis-outpace-whisper-for-lightning-fast-transcription/)
- **TTS**: [Kokoro-82M HF](https://huggingface.co/hexgrad/Kokoro-82M), [BentoML TTS 2026](https://www.bentoml.com/blog/exploring-the-world-of-open-source-text-to-speech-models)
- **VAD**: [TEN VAD GitHub](https://github.com/TEN-framework/ten-vad), [Agora TEN+Turn blog](https://www.agora.io/en/blog/making-voice-ai-agents-more-human-with-ten-vad-and-turn-detection/), [Picovoice VAD 2026](https://picovoice.ai/blog/best-voice-activity-detection-vad/)
- **Semantic turn**: [Pipecat smart-turn-v3 HF](https://huggingface.co/pipecat-ai/smart-turn-v3), [Daily.co smart-turn v3.1 accuracy](https://www.daily.co/blog/improved-accuracy-in-smart-turn-v3-1/), [Daily.co smart-turn v3.2 noisy environments](https://www.daily.co/blog/smart-turn-v3-2-handling-noisy-environments-and-short-responses/), [LiveKit turn-detector docs](https://docs.livekit.io/agents/logic/turns/turn-detector/)
- **Architecture**: [LiveKit pipeline architecture](https://livekit.com/blog/sequential-pipeline-architecture-voice-agents), [Pipecat docs](https://docs.pipecat.ai/), [OpenAI Realtime conversations](https://platform.openai.com/docs/guides/realtime-conversations), [Apple WWDC23 voice processing](https://developer.apple.com/videos/play/wwdc2023/10235/)
- **UX**: [LiveKit adaptive interruption](https://livekit.com/blog/adaptive-interruption-handling), [LiveKit turn detection](https://livekit.com/blog/turn-detection-voice-agents-vad-endpointing-model-based-detection), [ElevenLabs Jan 12 2026 changelog (LLM fillers)](https://elevenlabs.io/docs/changelog/2026/1/12), [Hamming voice stack 2026](https://hamming.ai/resources/best-voice-agent-stack)
- **Frameworks**: [LiveKit Agents releases](https://github.com/livekit/agents/releases), [Pipecat 1.0 GA](https://github.com/pipecat-ai/pipecat/releases), [Modal sub-1s voice bot](https://modal.com/blog/low-latency-voice-bot)

### Spike commits (reference only, do not rebase)
- Tag `voice-spike-archive` on the deleted spike branch — `git show voice-spike-archive:<path>` for any spike file as a reference

---

## 11. Open questions to validate before Phase 0.c

1. Does sherpa-onnx 1.13 expose endpoint API for Moonshine Small Streaming via `OfflineStream`? Or only via the OnlineRecognizer path? **Read sherpa-onnx 1.13 docs for `OnlineRecognizer` + `OnlineStream::is_endpoint()` before wiring 0.c.**
2. Is TEN VAD wrapped in sherpa-onnx 1.13 already, or do we need a separate ten-vad crate? **Check.**
3. What's the actual cold-build time for sherpa-onnx 1.13 on a clean Linux CI? **Measure once before committing Phase 0.a.**
4. Does Copilot SDK's `session.subscribe()` event stream preserve order across `assistant.message_delta` events under load? **Verify; document if not.**
5. Where exactly does `BobeStore.handleTextDelta` end on the assistant side (so voice can hook the same end-of-turn signal)? `BobeStore.swift:514-544` per mapping doc — confirm.
