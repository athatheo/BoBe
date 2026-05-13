# BoBe voice — full status

Comprehensive cold-start brief for the `voice-on-pivot` branch. Captures
everything shipped, everything in-flight, the architectural decisions that
shape both, and the genuinely-remaining work with implementation notes.

**Branch**: `voice-on-pivot` off `feat/copilot-sdk-pivot@f6bfaf5`
**Commits ahead of pivot base**: 50 (24 in the most recent autonomous run)
**Working tree as of writing**: ~5 files uncommitted, mid-refactor (see "In-flight" below)
**Build status**: would-build only after finishing the uncommitted hot-reload refactor; last green commit is `c22d092` (wizard step)
**Test status at `c22d092`**: 115/115 `cargo test`; `swift build` clean
**Plan of record**: `/Users/john/.claude/plans/iridescent-percolating-octopus.md`

---

## Table of contents

1. [Architecture overview](#1-architecture-overview)
2. [Lifecycle walk](#2-lifecycle-walk)
3. [What's done](#3-whats-done)
4. [What's in-flight (uncommitted)](#4-whats-in-flight-uncommitted)
5. [What's remaining](#5-whats-remaining)
6. [Research findings](#6-research-findings)
7. [Architectural decisions (D1–D13)](#7-architectural-decisions-d1d13)
8. [Known issues & risks](#8-known-issues--risks)
9. [References](#9-references)

---

## 1. Architecture overview

```
┌─────────────────────────────────────────────────────────────────────────┐
│ Swift client (BoBeMacUI)                                                │
│                                                                         │
│  AVAudioEngine (VPIO in+out)  ──┐                                       │
│  16kHz mono Float32             │                                       │
│  channelMap=[0] for VPIO        │                                       │
│  multichannel deinterleaved     │                                       │
│                                 ▼                                       │
│  Opus encoder 20ms VoIP    ▶ ws.send(Binary)                           │
│  ◀ ws.recv:                                                             │
│      JSON (state, transcript_partial, transcript_final,                 │
│            tts_end, truncate, error)                                    │
│      Binary (tts.chunk: 8B BE u64 chunk_id + 1B flags + Opus)           │
│                                                                         │
│  Welcome Wizard ──▶ poll GET /voice/install/status                      │
│                     POST /voice/install/start                           │
└─────────────────────────────────────────────────────────────────────────┘
                                 ▲
                                 │ WebSocket
                                 ▼
┌─────────────────────────────────────────────────────────────────────────┐
│ Daemon (BoBeService, Rust + axum 0.8.8)                                 │
│                                                                         │
│  /voice/stream  →  handle_socket (voice.rs ~830 LOC)                    │
│                    │                                                    │
│                    ├─ Keepalive: Ping every 25s, recv-timeout 60s       │
│                    ├─ Sink registry: install on Hello, RAII unregister  │
│                    │                                                    │
│                    Binary arm (every Opus frame):                       │
│                    1. Mute gate                                         │
│                    2. Opus decode → Vec<f32>                            │
│                    3. streaming_stt.accept_audio (Zipformer)            │
│                    4. streaming_stt.pop_partial → emit transcript_partial│
│                       └─ MinWords counter, cancel-phrase regex          │
│                    5. silero.accept (acoustic VAD)                      │
│                    6. has_segment? → spawn_turn task                    │
│                                                                         │
│  spawn_turn (per-turn task):                                            │
│    smart_turn.probability_complete  (8s window, mel [1,80,800])         │
│    │ < 0.7 → discard (mid-thought pause)                                │
│    │ ≥ 0.7 → continue                                                   │
│    try_begin_user_message (single-flight)                               │
│    voice_turn_active = true (RAII VoiceTurnFlag clears on drop)         │
│    state(Thinking) → state(Speaking)                                    │
│    spawn filler_watchdog (800ms; FillerKind::Thinking)                  │
│    spawn kokoro_task (1 Opus encoder reused across sentences)           │
│    session.set_model(..., reasoning_effort="low") if changed            │
│    session.send(MessageOptions::with_mode(Immediate))                   │
│      [BobeHooks fire mid-stream:                                        │
│         SessionStart       → memory.md inject                           │
│         UserPromptSubmitted → memory + (voice) LiveKit tone hint        │
│         PreToolUse         → (voice) per-tool filler emit via sink;     │
│                              compound filler if 2+ tools in 100ms;      │
│                              ask_user denied in voice mode              │
│         PostToolUse        → (voice) truncate >800-char results         │
│         ErrorOccurred      → log only (error filler is wired but        │
│                              not yet emitted)]                          │
│    observer(delta) → MarkdownStripper → SentenceBuffer                  │
│                   → sentence_tx (mpsc=16)                               │
│    kokoro task drains, synthesizes per sentence, encodes Opus,          │
│    pushes binary frames via out_tx                                      │
│    flush sentence_tx → kokoro task ends                                 │
│    tts_end, state(Listening)                                            │
│                                                                         │
│  Barge-in (3 paths, one shared `abort_active_turn` helper):             │
│    • Client RMS → barge_in WS msg → handle_barge_in                     │
│      (MinWords gate drops <3-word backchannel)                          │
│    • Streaming-STT partial matches cancel_phrases regex                 │
│      (inline; bypasses MinWords; no LLM round-trip)                     │
│    • Control{Abort|Reset} → handle_barge_in(0)                          │
│                                                                         │
│  /voice/install/{status,start,cancel}                                   │
│    VoiceInstallService — 4-artifact sequential downloads, watch::Sender │
│    progress, RAII cancel. on_complete callback (NOT YET WIRED through   │
│    pending hot-reload refactor) re-runs build_voice_engines_snapshot    │
│    and ArcSwap-stores into AppState.voice_engines.                      │
│                                                                         │
│  /metrics  →  Prometheus exposition: voice_stt_ms, voice_e2e_ms,        │
│               voice_smart_turn_inference_ms, 8 counters                 │
└─────────────────────────────────────────────────────────────────────────┘
                                 │
                                 ▼ JSON-RPC over stdio
┌─────────────────────────────────────────────────────────────────────────┐
│ github-copilot-sdk 0.1.0 (embedded-cli)                                 │
│  Session::send(MessageOptions::with_mode(DeliveryMode::Immediate))      │
│  Session::set_model(model, SetModelOptions::with_reasoning_effort(…))   │
│  Session::subscribe() → Stream<SessionEvent>                            │
│    assistant.message_delta → ChatDelta::MessageDelta                    │
│    tool.execution_start/complete → ChatDelta::ToolStart/Complete        │
│    session.idle → ChatDelta::Done                                       │
│  Hooks: SessionStart, UserPromptSubmitted, PreToolUse, PostToolUse,     │
│         ErrorOccurred                                                   │
└─────────────────────────────────────────────────────────────────────────┘
```

States: `idle → listening → capturing → thinking → speaking → cancelling → failed`. Daemon authoritative; client mirrors.

---

## 2. Lifecycle walk

Annotated walk of a single voice turn, including every architectural seam shipped on this branch. File:line refs are anchor points for someone picking up.

### A. Setup (one-time, via Welcome wizard)

1. **Wizard step 4 of 6** (`BoBeMacUI/.../Setup/WelcomeWizardSteps.swift::VoiceSetupStepView`):
   - On appear, GET `/voice/install/status`.
   - If `installed.allPresent == true` → "Continue."
   - Otherwise show "Install voice models" button.

2. **POST /voice/install/start** → `handlers/voice_install.rs::start` → `VoiceInstallService::start`:
   - Mutex-guarded `in_flight` JoinHandle; returns 409 if a job is already running.
   - Sequential downloads of 4 artifacts. Manifest is in `services/voice_install_service.rs::ARTIFACTS`.

3. **Per-artifact pipeline** (`services/voice_install_service.rs::install_one`):
   - `tempfile_dir()` → unique `/tmp/bobe-voice-install-<unix-nanos>/`.
   - `download_to` streams via `reqwest::Response::bytes_stream()`; writes to tempfile; emits `ModelProgress{kind, label, status:"downloading", bytes_downloaded, bytes_total, percent}` via `watch::Sender<VoiceInstallSnapshot>`.
   - Tarballs: `extract_and_install` → `spawn_blocking(bzip2::BzDecoder + tar::Archive::unpack)`. Renames `encoder*.onnx`/`decoder*.onnx`/`joiner*.onnx` to canonical names (Zipformer artifact ships them with `epoch-99-avg-1` suffixes).
   - Single-file artifacts: `tokio::fs::rename` from tempfile to final target.

4. **On Ok-complete**: the wired `on_complete: Arc<dyn Fn() -> BoxFuture<'static, ()> + Send + Sync>` (constructed in `bootstrap::mod.rs`) calls `build_voice_engines_snapshot().await` and `AppState.voice_engines.store(Arc::new(snapshot))`. This is the **hot-reload** path — installed today but **not yet compiling** because the consumer rewire in `copilot/registry.rs`, `copilot/hooks.rs`, `api/handlers/voice.rs` is uncommitted.

### B. Per-turn (every user utterance)

5. **Client** (`BoBeMacUI/.../Voice/VoicePipeline.swift`):
   - `prewarm()` sets up VPIO on both `inputNode` and `outputNode`. AVAudioPlayerNode attached via `mainMixerNode`. `setVoiceProcessingEnabled(true)` on both nodes — required for full-duplex AEC.
   - Multichannel input: `converter.channelMap = [0]` so we read AEC'd mic, not summed-with-reference.
   - `installTap(onBus: 0)` with `@Sendable` block + `DispatchQueue.main.async` hop (Swift 6 strict concurrency).
   - `Hello{session_id, capture_rate, playback_rate, codec, voice_id?, speed?}` — voice_id and speed are optional per-WS overrides over daemon defaults.

6. **Daemon `handle_socket`** (`api/handlers/voice.rs`):
   - Sole owner of `ws_tx`: dedicated writer task. Main loop, per-turn task, kokoro task all post via `out_tx: mpsc::Sender<Message>` clones.
   - Keepalive task: `tokio::time::interval(25s)` sends `Message::Ping`. Main loop wraps `rx.next()` in `tokio::time::timeout(60s, …)` so a silent dead connection closes after ~60s.
   - `SinkGuard` installs `out_tx.clone()` into `AppState.voice_sink` (single-slot) for the lifetime of the WS scope.
   - On `Hello`, constructs `SessionVoiceConfig { voice_id, speed }`. `speed` is `.clamp(0.5, 2.0)`.

7. **Binary arm (every frame)**:
   - Mute check.
   - `decoder.decode(packet, …)` → `Vec<f32>`.
   - `engines.stt.accept_audio(&samples)` → drives streaming Zipformer; internally `recognizer.is_ready` loop drains decoded frames.
   - `engines.stt.pop_partial()` is deduped against the last emission; returns `Some(text)` only when changed.
   - Emit `ServerMessage::TranscriptPartial { turn_id, text }` over the WS.
   - **Cancel-phrase check** (`voice/cancel_phrases.rs::is_cancel_phrase` — LazyLock regex, case-insensitive, word-anchored): if current_turn exists AND match → `abort_active_turn(s, &engines, &out_tx, last_acked_played_ms, "cancel_phrase")`. No LLM round-trip.
   - `engines.vad.accept(&samples)` — Silero acoustic VAD per-frame.
   - `engines.vad.has_segment()` → `pop_segment()` → if no current_turn, `spawn_turn(...)`. Else: increment `segments_dropped`, emit `voice.segment_backpressure_drop` warn, drop the segment.

8. **`spawn_turn` → `process_turn`** (per-turn `tokio::spawn`):
   - `engines.smart_turn.probability_complete(&segment.samples)` (`speech/smart_turn_onnx.rs::OnnxSmartTurn`):
     - Last 8s window with zero-pad at START (smart-turn was trained with pre-utterance silence).
     - Whisper-compatible mel features: `mel_spec::stft::Spectrogram::new(400, 160)` + `mel_spec::mel::MelSpectrogram::new(400, 16000.0, 80)`. Output transposed to `[N_MELS, N_FRAMES]` = `[80, 800]`.
     - tract-onnx `SimplePlan::run(tvec!(input))`. Sigmoid the scalar logit.
     - Threshold `TURN_COMPLETE_THRESHOLD = 0.7`. < 0.7 = discard segment (mid-thought pause).
   - `runtime_session.try_begin_user_message()` — single-flight admission via AtomicBool. 409s second voice turn.
   - **`voice_turn_active.store(true)` + `VoiceTurnFlag` RAII guard** — flips the flag AFTER single-flight admission to avoid the race where another worker class's hook fires during the brief window between flag-set and Err return. Hooks read via `voice_turn_active.load(Ordering::Acquire)`.
   - STT: `engines.stt.commit_final()` drains the streaming Zipformer's accumulated text + resets stream for next turn. `transcript_final{turn_id, text}` sent.
   - Spawn `filler_watchdog`: 800ms `tokio::time::sleep` then atomic-claim race against kokoro task for `first_audio_emitted: AtomicBool`. On win, emit `FillerKind::Thinking` PCM as Opus frames with `FLAG_FILLER`.
   - Spawn `kokoro_task`:
     - One `OpusEncoder` constructed via `voice/opus.rs::make_opus_encoder` — reused across all sentences in the turn (better entropy coding state).
     - Drain `sentence_rx`; per sentence, `spawn_blocking(tts.synthesize(text, voice_cfg.voice_id, voice_cfg.speed))`.
     - First emitted chunk sets `FLAG_FIRST_OF_TURN` + atomic-swaps `first_audio_emitted`.
   - `set_model(model, with_reasoning_effort)` — only fires if `last_effort` differs (cached on `CopilotChatWorker`).
   - `session.send(MessageOptions::new(text).with_mode(DeliveryMode::Immediate))` — atomic server-side interrupt of any prior turn. Defense-in-depth alongside the explicit `join.abort` path.
   - Observer (sync closure) feeds the SentencePipeline:
     - `MarkdownStripper` (`speech/markdown_strip.rs`): running-buffer regex stripper; suppresses emission while unclosed fence count is odd.
     - `SentenceBuffer` (`speech/sentence_buffer.rs`): LiveKit `_basic_sent` port; abbreviation-aware (`Dr.`, `Mr.`, `U.S.A.`, `e.g.`); emit-on-confirmed-terminator.
     - Confirmed sentences → `sentence_tx.try_send(...)`. Channel capacity 16; on full → `warn!("voice.sentence_channel_full_drop")`.
   - After stream done: `pipeline.flush()` for trailing unterminated text, drop `sentence_tx`, await `kokoro_task`.
   - `send_json(TtsEnd)`, `send_state(Listening)`, drop UserMessageGuard (releases single-flight).
   - Metrics: `voice_stt_ms`, `voice_smart_turn_inference_ms`, `voice_e2e_ms` histograms; `voice_turn_complete_total` counter.

### C. Barge-in (3 paths, one helper)

9. `abort_active_turn(s: &mut VoiceSession, engines: &VoiceEngines, out_tx, keep_ms, reason)` — the unified body. Steps:
   - `s.current_turn.take().join.abort()` → AbortGuard cascades to `session.abort()` → Kokoro task winds down via dropped `sentence_tx`.
   - `drop(join.await)` so the UserMessageGuard releases before the next turn admits.
   - `send_json(Truncate { keep_ms })`, `send_state(Listening)`.
   - `s.last_partial_text.clear()`, `engines.stt.reset()`.

10. **Three trigger paths**:
    - **RMS barge-in**: client tap detects RMS > -40 dBFS for 60ms during `.speaking` → `barge_in{ts_ms, playback_ms_played}`. Daemon's `handle_barge_in` applies MinWords gate (`s.last_partial_text.split_whitespace().count() < 3` → drop with `voice_barge_in_false_total` increment) then calls `abort_active_turn(..., "barge_in")` with `keep_ms = max(client_played_ms, s.last_acked_played_ms)`.
    - **Cancel phrase** (in Binary arm): regex match on partial → inline call to `abort_active_turn(..., "cancel_phrase")` with `keep_ms = s.last_acked_played_ms`. Bypasses MinWords.
    - **Control{Abort|Reset}**: `handle_barge_in(out_tx, session, engines, 0)`. Reset additionally `engines.vad.reset()` + `s.muted = false` + emits `state(Listening)`.

### D. Hooks (mid-turn, in SDK context)

11. **`BobeHooks` (`copilot/hooks.rs`)** fires from inside Copilot Session task. Knows nothing about the WS directly; reads `AppState.voice_sink` via shared `Arc<VoiceSink>` and `AppState.voice_engines` for the filler library (uncommitted — currently still uses owned `voice_filler_library`).

12. **PreToolUse**:
    - `ask_user` → `PreToolUseOutput { permission_decision: Some("deny"), permission_decision_reason: Some("voice mode cannot collect user input…") }`. Increments `voice_ask_user_blocked_total`.
    - Other tools → debounce 100ms via `Mutex<Option<Instant>>` (`last_pretool_at`). If within window → `FillerKind::ToolGeneric` ("Looking into a few things"). Else → `filler_for_tool(tool_name)` routes by lowercase substring (`web_search` → ToolWebSearch, `read`/`read_file` → ToolReadFile, default → ToolGeneric).
    - `emit_filler(&self.voice_sink, library, kind).await` — encodes cached PCM as Opus 20ms frames with `FLAG_FILLER`, pushes via the sink's `mpsc::Sender<Message>`.

13. **PostToolUse**: voice-only + result.len() > 800 chars → replace with `{ tool_name, summary: head + "…", success, truncated_for_voice: true, original_chars }`. Avoids reading file dumps aloud.

14. **UserPromptSubmitted**: memory.md inject first; if `voice_turn_active.load()` true, append the LiveKit voice-tone hint.

---

## 3. What's done

### Wave A — SDK plumbing (5/5)
| Task | Commit | Notes |
|------|--------|-------|
| A1 `DeliveryMode::Immediate` voice routing | `7505ff4` | ChatPrompt gains `voice_mode: bool`; voice_mode threads through message_handler → respond_to_message → send_via_chat_worker. Sets `with_mode(Immediate)` only on voice; text path keeps Enqueue default. |
| A2 WS keepalive ping + 60s recv timeout | `a74fe7d` | 25s Ping interval; `tokio::time::timeout(60s, rx.next())` wraps main rx. Pong arrival implicitly resets timeout. Swift mirror via `URLSessionWebSocketTask.sendPing` on a `@MainActor` Task. |
| A3 Mute/Unmute/Reset gating | `57739b2` | `VoiceSession.muted: bool` checked in Binary arm. Reset routes through `handle_barge_in(0)` + `engines.vad.reset()` + `s.muted = false`. |
| A4 UserPromptSubmitted voice tone | `08b5a1d` | `AppState.voice_turn_active: Arc<AtomicBool>` shared with WorkerRegistry. LiveKit voice-prompt template constant. Race fix in `7dbba9a` moves the store AFTER `try_begin_user_message` succeeds. |
| A5 `set_model` reasoning_effort | `b2e6c7b` | `CopilotChatWorker.model: Option<String>` captured at construction. Per-turn `set_model(model, with_reasoning_effort)` ("low" voice / "medium" text). Lean cleanup added `last_effort` Mutex<&'static str> to skip redundant RPCs. |

### Wave B — engine rewrite (2/2)
| Task | Commit | Notes |
|------|--------|-------|
| B1 smart-turn v3.2 ONNX | `fa28dae` | New `speech/smart_turn_onnx.rs::OnnxSmartTurn` via tract-onnx 0.21. mel_spec 0.3 for Whisper-compatible mel features (fft=400, hop=160, n_mels=80; 800 frames; transposed to [80, 800]). Last-8s window with zero-pad at START. Threshold 0.6-0.7. `StubSmartTurn` deleted. |
| B2 streaming Zipformer STT | `3e338db` | New `speech/streaming_stt.rs::LocalZipformerStt` wrapping sherpa-onnx 1.13.1 `OnlineRecognizer`. `StreamingSttEngine` trait: `accept_audio`/`pop_partial`/`commit_final`/`reset`. Per-frame feed in Binary arm; partial transcripts emitted over WS; `commit_final` at process_turn entry. Moonshine wrapper + old `SttEngine` trait deleted. |

### Wave C — hook features + UX (7/8, C6 pending)
| Task | Commit | Notes |
|------|--------|-------|
| C1 PreToolUse fillers + sink registry | `7420714` | Single-slot `VoiceSink` (`voice/sinks.rs`): `Arc<RwLock<Option<mpsc::Sender<Message>>>>`. `SinkGuard` RAII clear on drop. `emit_filler` encodes cached PCM → Opus → `FLAG_FILLER` chunks. `filler_for_tool(tool_name)` routes by lowercase substring. `ask_user` hard-denied in voice mode. |
| C2 typed FillerLibrary | `83d54c2` | `voice/filler_library.rs::FillerKind` enum: Thinking / ListenResume / LookupBridge / ToolWebSearch / ToolReadFile / ToolGeneric / ErrorReconnecting. `FillerLibrary::render(tts)` synthesizes all 7 phrases at boot via `futures::join_all` over `spawn_blocking` (parallelized in `7dbba9a`). 3 unit tests assert no-apology rule, no duplicates. |
| C3 MinWords barge-in gate | `1c27342` | `VoiceSession.last_partial_text` updated by B2's pop_partial; `handle_barge_in` drops barge-ins with `word_count < MIN_WORDS_FOR_BARGE_IN = 3` while turn is in flight. Cleared on new turn spawn. Pipecat default. |
| C4 compound parallel-tool filler | `87a2cf8` | `BobeHooks.last_pretool_at: Mutex<Option<Instant>>` (monotonic, post-lean — was AtomicU64<SystemTime>). 100ms debounce window. Second tool within window → `FillerKind::ToolGeneric` instead of stacked per-tool. |
| C5 PostToolUse summary | `9ef301b` | Voice-only truncation when `result.to_string().len() > 800`. Wrapped in JSON `{ tool_name, summary: head+"…", success, truncated_for_voice: true, original_chars }`. Anthropic-cookbook pattern. |
| C7 cancel phrases | `8afcb99` | `voice/cancel_phrases.rs::CANCEL_PHRASE_RE` LazyLock regex. Inline in Binary arm bypasses MinWords. `voice_cancel_phrase_match_total` counter. 12 unit tests covering matches + intentional misses ("I would never"). |
| C8 Hello voice_cfg | `4cb5a0f` | `ClientMessage::Hello` gains optional `voice_id`/`speed`. Stored on `VoiceSession.voice_cfg: SessionVoiceConfig`. Speed clamped 0.5–2.0. Swift mirror via `encodeIfPresent`. Lean pass dropped `voice_pack` field (was M5.x dead code). |
| **C6 voice settings pane** | **pending** | See §5. |

### Wave D — observability + polish (4/5, D5 pending)
| Task | Commit | Notes |
|------|--------|-------|
| D1 percentile telemetry + /metrics | `26acad4` | metrics 0.24 + metrics-exporter-prometheus 0.18. `PrometheusHandle` on AppState; `/metrics` route renders on each request. Today records: `voice_stt_ms`, `voice_smart_turn_inference_ms`, `voice_e2e_ms`. Counters: turn_complete, turn_error, barge_in_success, barge_in_false, filler_trigger, segment_drop, cancel_phrase, ask_user_blocked. Lean pass dropped 4 unrecorded histogram describes. |
| D2 backpressure metric | `5aa86fb` | `VoiceSession.segments_dropped: u64` + `CTR_SEGMENT_DROP`. `warn!` on each drop with running count. |
| D3 PlaybackAck-tightened truncation | `30e7264` | `VoiceSession.last_acked_played_ms` monotonic; `handle_barge_in` uses `max(client_played_ms, last_acked_played_ms)` for `Truncate.keep_ms`. |
| D4 Opus encoder reuse | `5aa86fb` | One encoder per kokoro task; helper `voice/opus.rs::encode_pcm_with(&mut encoder, …)`. Filler watchdog keeps its own (one-shot). |
| **D5 sample-accurate truncate** | **pending** | See §5. |

### New since the audit
| Task | Commit | Notes |
|------|--------|-------|
| Voice install service | `9e31552` | `services/voice_install_service.rs`. Mirrors `OllamaInstallService` shape. Mutex-gated single-flight; `watch::Sender<VoiceInstallSnapshot>` progress; RAII cancel. 4 artifacts hard-coded. Tarball extraction renames encoder/decoder/joiner ONNX to canonical names. `bzip2 = "0.4"` added (sherpa-onnx releases ship as .tar.bz2). |
| /voice/install/{status,start,cancel} | `9e31552` | `handlers/voice_install.rs`. Status response includes per-model `installed: PresenceSnapshot` (on-disk presence) alongside the per-model progress + aggregate status. |
| Welcome wizard VoiceSetupStepView | `c22d092` | New step 4 of 6. Phases: checking / idle / installing (500ms poll) / complete / failed / skipped. `DaemonClient.voiceInstallStatus`/`startVoiceInstall`/`cancelVoiceInstall` methods. Swift `VoiceInstall*` Codable types mirror Rust response. |
| `scripts/install-voice-models.sh` | `9e31552` | **DELETED**. Voice setup is no longer bash. |

### Lean re-architecture pass (commit `26fcda0`)
| Item | Effect |
|------|--------|
| Drop dead `voice_pack` field | Removed from `ClientMessage::Hello` + protocol tests + `SessionVoiceConfig` + Swift `ClientVoiceMessage.hello` + CodingKeys + VoicePipeline call site. |
| Drop 4 unrecorded histogram describes | `HIST_LLM_TTFT_MS`, `HIST_TTS_TTFB_MS`, `HIST_SENTENCE_EMIT_MS`, `HIST_INTER_TOKEN_GAP_MS` removed from `telemetry.rs`. Prometheus exposition no longer publishes empty series. |
| Remove `#[allow(dead_code)]` on `FillerLibrary::sample_rate()` | Real caller in `voice/sinks.rs::emit_filler`. |
| Compound filler debounce: SystemTime → Instant | Eliminates NTP-jump false-positive window. `Mutex<Option<Instant>>` instead of `AtomicU64<SystemTime ms>`. |
| Consolidate abort paths | `abort_active_turn(s, engines, out_tx, keep_ms, reason)` is the single body; `handle_barge_in` and cancel-phrase inline both call it. Previously the cancel-phrase path duplicated and slightly diverged (it included `engines.stt.reset()` that handle_barge_in lacked — now uniform). |
| `last_effort` cache on chat worker | `Mutex<Option<&'static str>>`. Skips redundant `set_model` RPC on back-to-back same-mode sends. |

---

## 4. What's in-flight (uncommitted)

The engine hot-reload refactor (item #1 in the next-steps list) is **partially landed** in the working tree. The change pattern:

**Goal**: Replace AppState's 5 voice engine fields with one `Arc<ArcSwap<VoiceEnginesSnapshot>>` so `VoiceInstallService::on_complete` can re-run `build_voice_engines_snapshot()` and atomically swap — wizard "Continue" works without a daemon restart.

**Files modified (uncommitted, do not compile yet)**:
- `BoBeService/src/voice/engines.rs` (new) — `VoiceEnginesSnapshot` struct with the 5 voice engine Options.
- `BoBeService/src/voice/mod.rs` — declares `engines` submodule.
- `BoBeService/src/app_state.rs` — replaced 5 voice engine fields with `voice_engines: Arc<ArcSwap<VoiceEnginesSnapshot>>`.
- `BoBeService/src/bootstrap/mod.rs`:
  - New `build_voice_engines_snapshot() -> VoiceEnginesSnapshot` async helper (existing `load_voice_engines` renamed to `load_engine_files` and wrapped).
  - Constructs `voice_engines: Arc<ArcSwap<…>>` from initial snapshot.
  - `VoiceInstallService::new` now takes `on_complete: Arc<dyn Fn() -> BoxFuture + Send + Sync>` that reloads via `engines.store(Arc::new(build_voice_engines_snapshot().await))`.
- `BoBeService/src/services/voice_install_service.rs`:
  - Field `on_complete: OnCompleteCallback`.
  - On `InstallStatus::Complete`, calls `on_complete().await` BEFORE flipping the snapshot status (so wizards polling don't see Complete until the AppState ArcSwap is updated).
- `BoBeService/src/copilot/registry.rs` — partially rewired: struct field `voice_filler_library` renamed to `voice_engines: Arc<ArcSwap<VoiceEnginesSnapshot>>`. Constructor signature updated. **`create_or_resume` BobeHooks::new call site still references the old `voice_filler_library` field** — this is the broken edit that the classifier kept rejecting.

**Files still needing edits (~80 LOC mechanical)**:
- `BoBeService/src/copilot/registry.rs::create_or_resume` — one Edit: swap `self.voice_filler_library.as_ref().map(Arc::clone)` for `Arc::clone(&self.voice_engines)` in the `BobeHooks::new(...)` call.
- `BoBeService/src/copilot/hooks.rs`:
  - Replace `voice_filler_library: Option<Arc<FillerLibrary>>` field with `voice_engines: Arc<ArcSwap<VoiceEnginesSnapshot>>`.
  - In PreToolUse + PostToolUse: read library via `self.voice_engines.load().filler_library.as_ref()` per fire.
  - Constructor accepts the new type.
- `BoBeService/src/api/handlers/voice.rs::VoiceEngines::from_state`:
  - Replace 5 `state.voice_*.as_ref()?` Arc-clones with a single load + extraction:
    ```rust
    let snap = state.voice_engines.load();
    Some(Self {
        stt: Arc::clone(snap.stt.as_ref()?),
        tts: Arc::clone(snap.tts.as_ref()?),
        vad: Arc::clone(snap.vad.as_ref()?),
        smart_turn: Arc::clone(snap.smart_turn.as_ref()?),
        fillers: snap.filler_library.as_ref().map(Arc::clone),
    })
    ```

**Recovery options for the working tree**:
1. *Resume the refactor*: complete the 3 file edits above, run `cargo build` to verify, commit.
2. *Revert*: `git -C /Users/john/Repos/bobrust/.claude/worktrees/voice-on-pivot checkout -- BoBeService/src/app_state.rs BoBeService/src/bootstrap/mod.rs BoBeService/src/services/voice_install_service.rs BoBeService/src/copilot/registry.rs BoBeService/src/voice/mod.rs && rm BoBeService/src/voice/engines.rs` → back to `c22d092` clean state.

---

## 5. What's remaining

### #1 Engine hot-reload after install (in-flight, ~80 LOC to finish)
Already covered in §4. Once the 3 file edits land:
- Verify with: `cargo test` (115 expected) + manual smoke with wizard → install → hot-reload → /voice/stream connects without restart.
- Edge case: WS connections established BEFORE the swap continue to use their captured snapshot until they disconnect. Subsequent connections pick up the new snapshot. This is correct (no yanking engines mid-turn).
- An alternative if you want stricter mid-session reload: add a `tokio::sync::watch::Sender<()>` "engines updated" signal that voice.rs's main loop reads on each iteration — but rare, probably not worth the complexity.

### #2 Model URL verification (5 min curl pass)
URLs hard-coded in `services/voice_install_service.rs::ARTIFACTS`:
- Zipformer: `https://github.com/k2-fsa/sherpa-onnx/releases/download/asr-models/sherpa-onnx-streaming-zipformer-en-2023-06-26.tar.bz2` — guess, may need a more recent release tag.
- Kokoro: `https://github.com/k2-fsa/sherpa-onnx/releases/download/tts-models/kokoro-multi-lang-v1_0.tar.bz2` — from existing prior install script, likely correct.
- Silero: `https://github.com/snakers4/silero-vad/raw/v6.2.1/src/silero_vad/data/silero_vad.onnx` — from existing prior install script, likely correct.
- Smart-turn: `https://huggingface.co/pipecat-ai/smart-turn-v3/resolve/main/smart_turn_v3.0.int8.onnx` — guess. Daemon looks for `smart-turn-v3.2.int8.onnx`; the installer renames to that target.

**Verification**: `curl -I` each URL. Replace any 404s with the correct upstream artifact. Update `target_subpath` constants to match if you change versions.

### #3 Settings → Voice pane (C6, #85, ~400 LOC cross-stack)
The biggest user-facing remaining gap. Lets users:
- See voice status (models installed? all/partial/none)
- Reinstall / repair (POST /voice/install/start again)
- Change `voice_persona` (Kokoro voice slot — 53 options, default `af_bella`)
- Change `voice_speed` (0.5–2.0)
- Per-soul `voice_id` override
- Toggle `voice_enabled`

**Implementation**:
1. **Rust DTOs** (`config/mod.rs` + `api/handlers/settings.rs`): add `voice_enabled: bool`, `voice_persona: String`, `voice_speed: f32`, `voice_max_turn_seconds: u32` to `DaemonSettings`. Default reasonable values.
2. **ConfigManager**: classify these as `applied` (no restart needed). The kokoro task reads `SessionVoiceConfig` (per-WS) which already falls through to defaults; if you want server-side defaults to apply, plumb a snapshot of these settings into `SessionVoiceConfig::new` when Hello omits them.
3. **AppState**: add `voice_config: Arc<ArcSwap<VoiceConfig>>` parallel to existing `config: Arc<ArcSwap<Config>>`. Bootstrap initializes from Config.
4. **SQL migration**: add `voice_pack TEXT` and `voice_id TEXT` columns to `souls`. Update `SoulRepository` insert/update/select.
5. **Swift Models** (`Models/DaemonSettings.swift`): mirror the new fields. `SettingsUpdateRequest` gets `voice*` fields.
6. **Swift Settings/VoicePanel.swift** (new file, ~250 LOC): SwiftUI panel with toggle, voice picker, speed slider, status section (calls `voiceInstallStatus`), reinstall button.
7. **SettingsWindow.swift**: register `.voice` category between `.appearance` and `.privacy`.
8. **Soul row UI**: extend SoulsEditor with `voice_id` picker (or defer the per-soul override to follow-up).

### #4 Overlay voice install status (~100 LOC)
The overlay's `MicButton` currently assumes voice is wired. If user skipped wizard, tapping fails.

**Implementation**:
- `BobeStore` polls `voiceInstallStatus()` every ~30s OR on `MicButton` tap.
- New `VoicePipeline.State` extension: `.needsSetup` when `installed.allPresent == false`.
- `MicButton` shows wrench icon + "Voice not installed" when in `.needsSetup`; tap opens Settings → Voice (or relaunches wizard).

### #5 Sample-accurate `truncatePlayback` (D5, #96, ~80 LOC Swift)
Current `truncatePlayback(keepMs _:)` in `VoicePipeline.swift:519` does `playerNode.stop() + reset() + play()` — drops everything immediately.

**Implementation**:
- Track each `scheduleBuffer` call with `(chunk_id, sample_offset, sample_count)` in a per-WS array.
- On `truncate{keep_ms}`: compute `target_sample = keepMs * playbackSampleRate / 1000`.
- Find which buffer's sample range straddles `target_sample`.
- `stop()` → drop tail buffers → re-schedule a PARTIAL of the straddling buffer (slice the AVAudioPCMBuffer at the boundary).
- v1 "stops everything" is acceptable UX; this is polish.

### #6 Localization for new strings (~9 locales × ~12 keys)
The wizard's `VoiceSetupStepView` and the future Settings → Voice pane use hardcoded English strings. The rest of the wizard uses `L10n.tr("setup.…")`.

**Implementation**:
- Add keys to `BoBeMacUI/BoBe/Resources/i18n/en.lproj/UI.strings`: `setup.voice.title`, `setup.voice.body`, `setup.voice.install_button`, `setup.voice.skip`, `setup.voice.cancel`, `setup.voice.complete`, `setup.voice.failed`, `setup.voice.continue`, `setup.voice.skipped_note`, `settings.voice.title`, `settings.voice.section.engine`, `settings.voice.section.models`.
- Replace inline strings in the Swift sources with `L10n.tr("…")`.
- For non-en locales: machine-translate or leave English placeholders (existing convention in the project).

### #7 Wake-word "Hey BoBe" (Wave E, ~560 LOC + ML training)

The biggest standalone UX upgrade after voice itself. Drops click-to-talk. **Genuinely multi-session** because E2 needs a training pipeline run.

**E1 livekit-wakeword Swift tap (~200 LOC)**:
- `BoBeMacUI/BoBe/Voice/WakeWord.swift` (new): pure Swift via CoreML or `ort-swift`.
- AVAudioEngine tap on inputNode separate from VoicePipeline; runs always-on while overlay is loaded.
- 80ms hop inference; ~2.5% CPU continuous on M4 Pro.
- Emit `ClientMessage::Wake { phrase, score, ts_ms }` to daemon WS on detection.
- Library: `livekit-wakeword` — May 2026 release, conv-attention head, 100× fewer FPPH than openWakeWord; backward-compatible with openWakeWord model format.

**E2 custom-train "Hey BoBe" model (~150 LOC training script + multi-hour runs)**:
- Default wake-word models are CC-BY-NC-SA; must custom-train for permissive license.
- `scripts/train-wakeword.sh`: Piper-synth positive examples (1000+ utterances across voices/accents); LibriSpeech as negatives.
- Output: `~/.bobe/models/wake-hey-bobe.onnx` (~6MB).
- Hosted somewhere downloadable so the VoiceInstallService can add it to the manifest.

**E3 stage-2 sherpa-onnx KWS verifier (~100 LOC Swift)**:
- Single-stage FPPH 0.5–1.0/h is too noisy for production.
- AVAudioEngine ring buffer of last 2s; on E1 wake candidate, run sherpa-onnx KWS on the buffer; reject if score < threshold.
- Target post-stage-2 FPPH < 0.1/h.

**E4 macOS sleep/wake handling (~50 LOC Swift)**:
- `IORegisterForSystemPower` callback in `BoBeMacUI/BoBe/Voice/SystemPower.swift` (new).
- Pauses AVAudioEngine on system sleep; resumes on wake.
- Without this, the tap thread accumulates buffers and OOMs on long sleeps.

**E5 wake → mic open hookup (~60 LOC Rust + Swift)**:
- Daemon Wake handler at `voice.rs:606` (currently logs only): if no current_turn and not muted → `state(Listening)`, allow streaming.
- Client: `MicButton` shows "wake-listening" state with subtle glow.

### #8 M5.2 hammering pushback proactive routing (~200 LOC)
C8 landed the proactive sink scaffold. M5.2 adds the **policy** layer:
- Detect repeated near-identical user requests (Levenshtein < 0.3) within a window.
- 4-level escalation cadence with per-soul `voice_pushback_pack`.
- `RuntimeSession::try_begin_proactive_message()` parallel to `try_begin_user_message` (same UserMessageGuard).
- `AppState.voice_proactive: Arc<dyn Fn(turn_id, text) -> BoxFuture>` — synthesizes via Kokoro and pushes through `voice_sink`.

**Implementation outline**:
- `voice/proactive.rs` (new): `route_to_voice_sink(turn_id, text)` helper that uses the existing kokoro task pattern.
- `runtime/session.rs::try_begin_proactive_message`: same AtomicBool single-flight, different return type for distinction.
- Repeat-detection in conversation_service or a new `services/hammering_detector.rs`.
- Per-soul voice_pushback_pack column on souls + UI editor.

### #9 Tier-3 corpora tests (Wave F partial, ~300 LOC + corpus download)
**F1 tier-1 unit expansion (~200 LOC)**: Already partially covered (115 tests). More edges: streaming_stt partial accumulation, smart-turn boundary cases, sentence_buffer multi-language, markdown_strip nested fences, opus encode/decode round-trip.

**F2 tier-2 integration with fakes**: **DELETED** per user feedback (no fakes/mocks). The slot is now "integration tests use real engines + real Copilot CLI" — i.e., live integration tests gated `#[ignore]` for nightly.

**F3 tier-3 E2E with corpora (~300 LOC + downloads)**:
- LibriSpeech-test-clean: feed corpus through real Zipformer; assert WER ≤ 10%.
- UTMOS/NISQA TTS quality: synthesize 20 reference utterances via Kokoro; assert UTMOS ≥ 3.5.
- Smart-turn-data-v3.1-test corpus accuracy gate; assert ≥ 85%.
- Slow (3–5 min); `#[ignore]` gated; nightly CI only.

**F4 E2E latency SLA test (~150 LOC)**: Synthetic audio replay through full pipeline (real Zipformer + real Kokoro + REAL Copilot CLI — no fakes); assert /metrics histograms meet SLOs (e2e p50 <800ms, p95 <1500ms, barge-in success ≥95%, false ≤5%).

**F5 nightly synthetic replay CI (~50 LOC)**: GitHub Actions / Forgejo nightly job runs F3 + F4; pass/fail summary; 30-day history; regression alerts on tier-3 gate breaks.

---

## 6. Research findings

### Copilot SDK (rust-v0.1.0 / monorepo beta.3) — what's there, what's not
**Confirmed available**:
- `DeliveryMode::Immediate` (types.rs:2407) — single-RPC interrupt + run. Used by A1.
- `MessageOptions::with_mode(DeliveryMode)` (types.rs:2494).
- `Session::set_model(model, Option<SetModelOptions>)` (session.rs:415) + `SetModelOptions::with_reasoning_effort` (types.rs:2168). Used by A5.
- `Session::cancellation_token()` (session.rs:169) — child token to bind futures to session lifetime.
- `Session::id() -> &SessionId` (session.rs:116).
- Hooks: `SessionStart`, `UserPromptSubmitted`, `PreToolUse`, `PostToolUse`, `SessionEnd`, `ErrorOccurred`.
- `PreToolUseOutput.permission_decision: Option<String>` ("allow"/"deny") + `permission_decision_reason: Option<String>`.
- `PostToolUseOutput.modified_result: Option<Value>` — replace tool result before LLM re-prompt.
- `UserPromptSubmittedOutput.additional_context: Option<String>` — inject context per-turn.
- Tool events: `tool.user_requested`, `tool.execution_start`, `tool.execution_partial_result`, `tool.execution_progress`, `tool.execution_complete`. `AssistantMessageData.tool_requests: Vec<…>` carries parallel calls.

**Confirmed NOT available**:
- No `MessageOptions.metadata` field — voice-mode signaling falls back to AppState shared state (`voice_turn_active` AtomicBool + `voice_sink` registry).
- No `SystemMessageTransform` per-turn hook (only fires at session.create/resume).
- No audio/voice surface in the SDK at all (despite the github-next "Copilot Voice" sunset). Our voice loop is entirely BoBe-side.
- No mid-stream interrupt token beyond `session.abort()` + `cancellation_token()`.
- No token-budget cap on MessageOptions.

### Voice stack picks (May 2026)
| Component | Pick | Rationale |
|-----------|------|-----------|
| STT | sherpa-onnx 1.13.1 streaming Zipformer EN | Live partials; ~6.5% WER EN; unlocks MinWords + cancel phrases + (future) speculative LLM. Moonshine (offline-on-segment) deleted. |
| TTS | Kokoro v1.0 multilingual (sherpa-onnx) | 24kHz mono; 53 voice slots; Apache 2.0; <300ms first-audio on M4 Pro. Orpheus 3B better-sounding but no clean ONNX. F5-TTS CC-BY-NC. |
| Acoustic VAD | Silero v6.2.1 ONNX (sherpa-onnx) | ~189μs/chunk "ifless" build; ~2MB; MIT. v7 not released. |
| Semantic VAD | smart-turn v3.0 via tract-onnx 0.21 | Pipecat smart-turn-v3 8MB int8; ~12-50ms CPU; Whisper-tiny feature extractor [1,80,800] f32; MIT. v3.2 is Daily.co's tag for short-utterance improvements — model name preserved as `smart-turn-v3.2.int8.onnx` for forward-compat when v3.2 ONNX publishes. |
| Wake-word (M5.1) | livekit-wakeword (custom-trained "Hey BoBe") | May 2026 release; conv-attention head; 100× lower FPPH than openWakeWord; backward-compatible model format. Defaults are CC-BY-NC-SA → custom-train via Piper. |
| Codec | Opus 20ms VOIP @24kbps | 16kHz capture, 24kHz playback. 320-sample frames at 16kHz. |
| AEC (macOS) | AVAudioEngine VPIO on BOTH input AND output | Required for full-duplex AEC during TTS playback. `channelMap=[0]` for multichannel VPIO. |
| AEC (Linux, future) | aec-rs (SpeexDSP) + DeepFilterNet3 NS | Not yet built; defer until remote-daemon mode. |
| Mel preprocessing | mel_spec 0.3 | Whisper-compatible. fft=400, hop=160, 80 mel bins. |
| ONNX runtime | tract-onnx 0.21 | Pure-Rust; ~3× slower than `ort` but irrelevant at 8MB int8 / <50ms inference. Avoided `ort 2.0.0-rc.12` due to VitisAI EP API mismatch with bundled ORT in sherpa-onnx. |

### Architectural patterns stolen
- **Pipecat MinWordsUserTurnStartStrategy** → C3 MinWords gate (3 words during TTS, 1 idle).
- **GetStream speculative tool calling** → C1 PreToolUse fillers via hook-side sink emit.
- **LiveKit "Prompting Voice Agents to Sound More Realistic"** template → A4 VOICE_TONE_HINT constant.
- **Anthropic cookbook PostToolUse summary** → C5 truncate >800-char results.
- **Pipecat parallel-tool compound filler** → C4 (debounced 100ms via Instant).
- **OllamaInstallService pattern (BoBe-internal)** → VoiceInstallService shape.

### Tasks rejected after research
- `SystemMessageTransform` for per-turn voice tone: only fires at session.create. → Used `UserPromptSubmitted` hook with conditional additional_context.
- F2 Tier-2 with FakeStreamingSttEngine/MockCopilotSession: per `feedback_no_fakes_no_mocks.md`, NO test doubles ever. Replaced with real-engine integration tests gated `#[ignore]`.
- Phoneme-boundary TTS interrupt: not a 2026 pattern. Sentence-cut is what production agents do.
- YAMNet voice classification: 10MB overhead for marginal "ignore TV" UX.
- Speech as separate cargo crate: coupling clean via trait-DI; no real consumer.

---

## 7. Architectural decisions (D1–D13)

Documented in the plan file. Quick reference:

- **D1**: Single STT path — streaming Zipformer replaces Moonshine entirely.
- **D2**: Smart-turn v3.2 ONNX is required; `StubSmartTurn` deleted.
- **D3**: Hook → voice sink wiring via single-slot `VoiceSink` registry (simplified from session-keyed because single-flight makes it sufficient).
- **D4**: Voice-mode signal: per-call via shared `AppState.voice_turn_active: Arc<AtomicBool>` (Copilot SDK 0.1 doesn't expose MessageOptions metadata).
- **D5**: Voice config hot-reload via `Arc<ArcSwap<VoiceConfig>>` (NOT YET implemented for VoiceConfig per se; engines snapshot in-flight).
- **D6**: `ask_user` tool hard-disabled in voice mode (matches autopilot pattern).
- **D7**: Single voice session at a time via existing `UserMessageGuard`.
- **D8**: Hook ordering at `UserPromptSubmitted`: memory injection FIRST, voice-tone hint SECOND.
- **D9**: Daemon fails loud on missing models — **NOT enforced**. Currently warns + returns Option::None; /voice/stream 503s. Intentional softer behavior because welcome wizard owns install.
- **D10**: Latency budgets (p95): mic→daemon 30ms, VAD 5ms, STT first partial 150ms, smart-turn 50ms, set_model+send 50ms, LLM TTFT 600ms ("low"), Kokoro first sentence 200ms, opus+ws 75ms. **End-to-end target: p50 <800ms, p95 <1500ms.**
- **D11**: Per-client voice config via Hello handshake (`voice_id`, `speed`); falls through to DaemonSettings defaults.
- **D12**: Cancel phrases bypass the LLM (regex on streaming-STT partials).
- **D13**: Wake-word is client-side; daemon receives `Wake` event only.

---

## 8. Known issues & risks

### Bugs
1. **Engine hot-reload refactor uncommitted** (§4). Working tree won't compile.
2. **Model URLs guessed** (§5 #2). First install attempt will reveal 404s.
3. **Smart-turn-v3.2 ONNX may need rename** — daemon looks for `smart-turn-v3.2.int8.onnx`; installer downloads `smart_turn_v3.0.int8.onnx` and renames at target path. If upstream changes filename, install will silently succeed but daemon won't find the model. Mitigate: log the target path explicitly on load failure.
4. **Wizard "Continue" does not yet wait for engine reload to confirm**: currently it shows "Complete" status from /voice/install/status, and that snapshot is bumped by the install service. With hot-reload uncommitted, engines are still None — first /voice/stream after wizard 503s. ONCE hot-reload lands this resolves.

### Risks
1. **OnlineStream Mutex over-cautious**: `LocalZipformerStt.stream: Mutex<OnlineStream>` was speced for cross-task concurrent access; current callers all run on the same main rx loop. Could drop the Mutex once we verify no future caller needs it. Defer until B2 has more callers.
2. **Filler library bootstrap blocks ~2–5s** even after parallelization: 7 phrases × ~300–700ms Kokoro synthesis. Acceptable but noticeable on first daemon launch. Future: move to background `tokio::spawn` + ArcSwap and accept a brief no-filler window post-boot.
3. **`set_model` doesn't actually validate the model name**: a typo in DaemonSettings would silently warn + continue. Production: add a one-time validation at session-create.
4. **AppState arms race**: bootstrap reorder (load voice engines BEFORE workers registry, per C1 commit) means daemon startup blocks on ~2–5s of filler synthesis. Tolerable but matters for cold starts.
5. **`voice_turn_active` global atomic**: works because single-flight. Multi-turn (M5.2 proactive) would break the assumption — `voice_turn_active` would need to be per-session-id or the proactive path would need its own flag.

### Open uncertainties
1. **SDK metadata-on-MessageOptions** verified absent at audit time; if a future SDK release adds it, swap from `voice_turn_active` AtomicBool to per-message metadata for cleaner per-turn isolation.
2. **smart-turn v3.2** is Daily.co's tag, not necessarily a HuggingFace artifact yet. Currently we download v3.0 and rename to v3.2 — when v3.2 ONNX publishes, just update the URL in `ARTIFACTS`.

---

## 9. References

### Project files
- **Plan of record**: `/Users/john/.claude/plans/iridescent-percolating-octopus.md` (30-task plan across 6 waves).
- **Voice design doc**: `docs/voice-plan.md` (definitive May-2026 plan with stack picks).
- **Memory pointers**: `/Users/john/.claude/projects/-Users-john-Repos-bobrust/memory/MEMORY.md`:
  - `feedback_simplification_preferences.md` — no backcompat, no split-brain
  - `feedback_no_fakes_no_mocks.md` — tests use real engines + real corpora
  - `feedback_follow_the_plan.md` — once approved, just execute; no procedural questions
  - `reference_voice_implementation_findings.md` — 12-agent research
  - `reference_voice_production_patterns.md` — Pipecat/LiveKit/Daily patterns
  - `reference_copilot_sdk_api.md` — SDK cheat sheet

### External
- github-copilot-sdk: `https://github.com/github/copilot-sdk/tree/main/rust/src/`
- sherpa-onnx Moonshine docs: `https://k2-fsa.github.io/sherpa/onnx/moonshine/index.html`
- sherpa-onnx releases (model bundles): `https://github.com/k2-fsa/sherpa-onnx/releases`
- Silero VAD: `https://github.com/snakers4/silero-vad/releases`
- Pipecat smart-turn v3: `https://huggingface.co/pipecat-ai/smart-turn-v3`
- Daily.co smart-turn v3.2: `https://www.daily.co/blog/smart-turn-v3-2-handling-noisy-environments-and-short-responses/`
- livekit-wakeword: `https://livekit.com/blog/livekit-wakeword`
- LiveKit "Prompting Voice Agents to Sound More Realistic": `https://livekit.com/blog/prompting-voice-agents-to-sound-more-realistic`
- Pipecat MinWordsUserTurnStartStrategy: `https://docs.pipecat.ai/api-reference/server/utilities/turn-management/user-turn-strategies`
- mel_spec crate: `https://github.com/wavey-ai/mel-spec`
- tract-onnx: `https://github.com/sonos/tract`

### Commit log (this session, in order)
```
c22d092 voice(wizard): VoiceSetup step in WelcomeWizard, drives /voice/install
9e31552 voice(install): daemon-owned voice model installer, delete shell script
26fcda0 voice(lean): purge dead code, consolidate abort path, monotonic compound
c1e2a83 voice(test): hook-level tests for voice tone gate
7dbba9a voice(review): close race + parallelize filler render + richer postool ctx
4cb5a0f voice(C8 partial/D11): per-WS voice_id/speed/voice_pack via Hello handshake
3e338db voice(B2/R11): streaming Zipformer STT, delete Moonshine + SttEngine trait
1c27342 voice(C3/R12): MinWords barge-in gate drops "uh-huh" backchannel
8afcb99 voice(C7): cancel-phrase regex bypass LLM during TTS
fa28dae voice(B1/M4.5.2): smart-turn v3.2 ONNX gate, delete StubSmartTurn
26acad4 voice(D1/R16): /metrics endpoint + per-turn histograms + counters
87a2cf8 voice(C4/R13): compound filler for parallel tools within 100ms
7420714 voice(C1/R8): single-slot VoiceSink + PreToolUse fillers + ask_user gate
83d54c2 voice(C2/R14): typed FillerLibrary replaces single voice_filler_pcm
30e7264 voice(D3/R3): tighter barge-in truncate via PlaybackAck
9ef301b voice(C5/R15): PostToolUse hook truncates long tool results in voice mode
5aa86fb voice(D2+D4): segment-drop warn counter + per-turn Opus encoder reuse
08b5a1d voice(A4/R7): UserPromptSubmitted hook injects voice-tone hint
b2e6c7b voice(A5/R9): set_model reasoning_effort=low for voice, medium for text
57739b2 voice(A3/R1): wire Mute/Unmute/Reset control actions
a74fe7d voice(A2/R5): WebSocket keepalive ping + 60s recv timeout
7505ff4 voice(A1/R10): route voice turns via DeliveryMode::Immediate
```
