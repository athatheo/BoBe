# Voice Production Plan for BoBe (pivot-aware reset)

> **Status**: 2026-05-11. This document supersedes earlier voice planning. The voice spike (M1–M4.5.1) was implemented on a branch forked from `main` BEFORE the `feat/copilot-sdk-pivot` work landed. The pivot has 74 commits we don't have and a fundamentally different architecture (Copilot SDK sessions, single-doc memory.md, Engine pane, welcome wizard, MCP runtime state). **The spike will NOT be rebased**. Instead, treat the spike as a working reference for features + design picks, and rebuild those capabilities on the pivot's architecture.

> **Mentality**: "What features did we add? How do those features land on the pivot?" — not "replay the 12 commits".

---

## 1. Branch context + recovery path

### What happened
- `worktree-voice-spike-phase1` (this branch) was forked from `main` at `5479298` (CI sparkle fix). Twelve commits of voice work were built on top.
- `feat/copilot-sdk-pivot` is the actual evolving main (74 commits ahead). It has Copilot SDK 0.1.0 in Cargo.toml, has restructured the message/memory/settings layers, and has shipped a welcome wizard.
- The spike's `LlmProvider::stream()` calls, SQLite memory assumptions, and Settings panel layout do not exist on the pivot.

### Recovery sequence
1. **Save this doc** (it lives at `docs/voice-production-plan.md` on this branch — commit it before nuking).
2. **Memory files** at `~/.claude/projects/-Users-john-Repos-bobrust/memory/reference_voice_*.md` already capture the 12+4 agent research. They persist across worktree changes.
3. **Nuke `worktree-voice-spike-phase1` worktree + branch** (after this doc is committed and copied to a safe place).
4. **Create new worktree from `feat/copilot-sdk-pivot`** (e.g. `worktree-voice-on-pivot`).
5. **Phase -1 (mapping)**: read the pivot's architecture before writing code. Checklist in §8 below.
6. **Phase 0 (re-build M1–M4.5.1 features on the pivot)**: capabilities listed in §2, adapted to the pivot's shape per §4.
7. **Phase 1+ (M4.5.0–M5.2)**: as plan in §6.

### Tasks to clean up in the task DB on session restart
Existing M4.5.0a–M5.2 task IDs (#80–#89) are written against this branch's assumed architecture. When restarting on the pivot, re-create them with the corrected scope from this doc. Existing M4.x completion records (#69–#79) describe spike commits that don't exist on the pivot — keep as historical, but the work needs re-doing.

---

## 2. What the spike proved (features to add to the pivot)

These are the **capabilities** the spike validated, in dependency order. The PIVOT REWRITE of each is what gets implemented; the spike code is the reference, not the source.

| # | Capability | Spike commit | Files in spike (reference) | Pivot rewrite needed |
|---|---|---|---|---|
| 1 | Daemon speech module: sherpa-rs Moonshine STT (16kHz f32 → text) + Kokoro v1.0 TTS (text + voice slot → 24kHz Int16 PCM) loaded once at boot, gated by feature flags + env. | M4.1 (`1c7b9b3`) | `BoBeService/src/speech/{local_sherpa,local_kokoro,protocol,mod}.rs`, `BoBeService/src/bootstrap/mod.rs:114-188` | New `src/speech/` directory on pivot. Bootstrap calls likely move under pivot's startup pattern. |
| 2 | WS endpoint `/voice/stream`: per-connection Opus decoder, PCM accumulator, JSON control messages (Start/Commit/Stop/Ack/Transcript/SpeakingStarted/SpeakingEnded/Error). | M4.1 | `BoBeService/src/api/handlers/voice.rs` | New handler on pivot under whatever its API router pattern is. |
| 3 | Swift voice client: AVAudioEngine VPIO 16kHz mono Float32 input + AVAudioPlayerNode 24kHz Int16 output, Opus encode/decode via swift-opus, WS transport, @Sendable tap + DispatchQueue.main.async hop, prewarm() so AGC settles, channelMap=[0] for multichannel mics. | M4.2 (`e6b15c6`) | `BoBeMacUI/BoBe/Voice/{VoicePipeline,MicButton}.swift` | Same files on pivot; integration with pivot's overlay UI shape may differ. |
| 4 | LLM wire-in: voice transcript → chat pipeline → reply streamed back → markdown-stripped → Kokoro TTS → Opus → WS. Voice transcript persists to conversation history same as text. Single-flight guard `try_begin_user_message` held across STT + LLM + TTS. | M4.3–4.4 + review (`8078335`, `b997788`, `72ab8f5`) | `BoBeService/src/api/handlers/voice.rs:78-204`, `BoBeService/src/runtime/{session,message_handler,response_streamer}.rs` | **THIS IS THE BIG REWRITE**: pivot uses Copilot SDK sessions, not LlmProvider. Voice handler talks to SDK directly (or via pivot's chat pipeline wrapper). The `respond_to_message` shape and the observer pattern (`proactive_generator.rs:200-228` on this branch) won't exist on the pivot in the same form. §4 details. |
| 5 | VAD-gated streaming: drop push-to-talk, client-side RMS hysteresis VAD (-45 dBFS speech / -50 dBFS silence), 60ms speech-start / 700ms silence-end thresholds, auto-commit on silence. | M5.1 (`30c3dd0`) + M4.5.1 (`8f23fa2`) | `BoBeMacUI/BoBe/Voice/VoicePipeline.swift` | Same — client-side, no pivot collision. |
| 6 | LiveKit-canonical state machine: 9 states (idle/connecting/ready/recording/committing/thinking/speaking/stalled/failed) with hysteresis between speech (-45) and silence (-50) thresholds; VAD runs in committing/thinking/speaking states too (no transmission) to set up barge-in. | M4.5.1 | `BoBeMacUI/BoBe/Voice/VoicePipeline.swift`, `MicButton.swift` | Same — client-side. |

**Critical fixes from spike reviews** that must NOT be lost in the pivot rewrite:
- Acquire `try_begin_user_message` guard BEFORE sending Transcript JSON to client (M4 review) — prevents orphan transcript bubble on conflict.
- Hold the guard across STT + LLM + TTS (M4 review CRITICAL) — naive scoping drops it before TTS spawn_blocking runs and lets concurrent text-chat steal the slot.
- Strip markdown from reply text BEFORE Kokoro synthesis (M4 review) — otherwise Kokoro speaks asterisks literally. LazyLock-cached regex set.
- Empty transcript → silent drop + Transcript("") + Error("I didn't catch that"), not a canned reply. Don't orphan chat history.
- TTS failure must send `SpeakingEnded` before erroring, or client wedges in `.speaking` state forever.
- Synthesize PCM FIRST, then send SpeakingStarted, then stream Opus, then SpeakingEnded — pairing invariant.
- Disconnect race: WS `receiveLoop` snapshots `currentTask`, discards stale callbacks; `disconnect()` sends "stop" + cancels old task in same Task — prevents stale state stomps on reconnect.
- VPIO must be enabled on BOTH `inputNode` AND `outputNode` (spike only had input — flag for pivot fix).
- `scheduleBuffer(buf, completionHandler: nil)` (fire-and-forget queue version) — the async overload deadlocks because it returns when buffer FINISHES playing.
- xcodegen regenerated Info.plist/entitlements and blew away NSMicrophoneUsageDescription + audio-input → moved keys into `project.yml`'s `info.properties` + `entitlements.properties`.
- swift-opus pinned to revision `6f3cb6bd3ffed1fe5f06d00a962d5c191a50daf8` in both Package.swift and project.yml for reproducibility.
- CMake compat for libopus: `CMAKE_POLICY_VERSION_MINIMUM=3.5` env var.
- Kokoro voice table: 53 slots (0..=52) — canonical from `generate_voices_bin.py`; spike had off-by-one error (non-existent "em_santa") fixed in M4.1 review.

---

## 3. Design picks (architecture-independent, survives the pivot)

| Need | Pick | Rationale |
|---|---|---|
| STT | sherpa-rs Moonshine base int8 (English) | Offline, fast, CoreML EP on Mac, CPU on Linux daemon. Confidence not exposed but not needed for v1. |
| TTS | sherpa-rs Kokoro v1.0 multilingual (24kHz Int16) | Local, ~85MB, voice via slot id (af_bella default). espeak-ng phonemization. |
| Audio codec | Opus 24kbps VOIP, 20ms frames | Apple-native via swift-opus (alta/swift-opus pinned), Rust via `opus` 0.3 crate. |
| WS protocol | Deepgram-style: binary=audio, JSON=control | Industry norm. Single writer to SplitSink. |
| AEC + AGC + NS | AVAudioEngine VPIO on both nodes | Apple's full-duplex stack; matches FaceTime quality. `mDuckingLevel = .min` for soft barge-in. |
| Acoustic VAD | RMS hysteresis client-side, 700ms silence floor | LiveKit/VideoSDK production. 500ms was too aggressive (M4.5.1 corrected). |
| Semantic VAD | Pipecat smart-turn-v3.0 ONNX via `ort` (8MB, ~12ms M-class) | Only local-runnable option. HF publishes v3.0 + v3.2-cpu only (no "v3.1" file). |
| Sentence splitter | Port LiveKit `_basic_sent.split_sentences` (~50 LOC Rust) | Handles Mr./Dr./U.S.A./3.14/.com/Ph.D. Don't pull NLP models. |
| Filler audio | 20-phrase intent-keyed library, pre-built to repo, Opus on disk + PCM in memory | Production norm (Vapi/Retell/ElevenLabs). One filler per turn cap. 80ms crossfade. |
| Barge-in | 3-event OpenAI Realtime canonical: cancel/truncate/clear + Pipecat 3-word minimum | Industry standard. Client orchestrates serial-fire-and-forget. |
| Wake-word (M5) | openWakeWord custom-trained ONNX via `ort` | Default oWW pretrained models are CC-BY-NC-SA (unusable commercially) → custom train via Pipecat Colab notebook. Porcupine rejected ($6k/yr commercial). |
| Stall watchdog | Tokio `select!` + `Sleep::reset`, 800ms inter-token, 15s total soft, 30s hard | Reuses filler cache infra. Separate stall pool. |
| State machine | LiveKit-canonical 9 states + `false_interruption_pending` + `interrupted_draining` | Industry standard. |

---

## 4. What WILL change on the pivot (architecture deltas)

This is the load-bearing section. Every code path the spike touched needs re-mapping on the pivot.

| Spike assumption | Pivot reality (confirmed) | Implementation impact |
|---|---|---|
| `LlmProvider::stream()` returns `Stream<Item=Result<StreamChunk>>` | `github-copilot-sdk = "0.1"` with `Session::send` + `Session::subscribe` event stream | Voice handler talks to SDK directly. Event types: `assistant.message_delta` (field is `deltaContent`, NOT `delta`), `tool.execution_start/complete`, `session.idle`. |
| No abort primitive — need HTTP/2 RST_STREAM hack | `session.abort()` is a real call | Barge-in M4.5.5 is simpler: no token-waste workaround. Abort fires after current tool call (atomic-file-write guarantee). |
| No mid-turn injection | `DeliveryMode::{Immediate, Enqueue}` exist | Hammering pushback M5.2: `Enqueue` for queue, `abort()`+`send()` for L4 escalation, `Immediate` for steering. |
| `respond_to_message` in `runtime/message_handler.rs` is private + accumulates stream + persists | **Unknown shape on pivot** — needs exploration | The pivot may have a streaming-text observer pattern, or may stream events directly from the SDK to SSE without a wrapper. The spike's "fork respond_to_message with observer" plan assumes spike shape. Map the pivot's chat path first. |
| `proactive_generator.rs:200-228` is the in-tree observer precedent | **May or may not exist on pivot in the same form** | If proactive on the pivot uses SDK session.send too, the observer pattern is just `subscribe()` — different shape from spike. |
| SQLite `MemoryRepository`, multi-record memories distilled by learning loop | Single-doc memory.md, GET/PUT /memory endpoint, daemon prunes nightly at 03:00 local | Voice transcripts feed memory differently — via the pivot's memory-write path, not via observation→consolidation. Mid-reply barge-in truncation may need to truncate the memory.md write too. |
| `aiModel` Settings pane | `Engine` pane (replaced) | Voice settings pane goes elsewhere in the PREFERENCES group. SettingsCategory enum is different. |
| No welcome wizard | Welcome wizard with Copilot CLI presence check, permissions step | Mic permission step belongs in the wizard. May need to add a "voice setup" step (model download) once M4.5.0a lands. |
| `binary_manager` is Ollama-only on this branch | `binary_manager` already resurrected for Ollama runtime download on pivot (`engine(phase-5a): resurrect binary_manager for Ollama runtime download`, `8fc1bad`) | M4.5.0a extends the pivot's `binary_manager` for voice models; doesn't refactor from scratch. |
| BobeStore appends voice messages to chat history via `appendUserVoiceMessage` | Pivot's chat/overlay shape may be different (swiftui catch-up: `swiftui(memory)`, `swiftui(privacy)`, `swiftui(settings)`, `swiftui(welcome)`) | Mirror logic to pivot's overlay history append point. |
| Conversation auto-close after `conversation_auto_close_minutes` triggers memory distillation | Memory model is different (single-doc) — auto-close still exists but doesn't drive memory consolidation the same way | Voice transcript persistence semantics likely unchanged at the conversation_repo layer; memory side differs. |

**Unknown on pivot (must explore before coding):**
- Does the pivot have a chat WS endpoint or only HTTP `/message` + SSE `/events`?
- Where is the equivalent of `runtime/session.rs::handle_user_message`?
- Does the pivot have a `try_begin_user_message` single-flight guard? (Probably yes — text+voice need it.)
- What's the conversation persistence shape — per-turn record, streaming-turn (begin/push/finalize), or something else?
- Where do tool events surface on the pivot — direct from SDK subscription, or re-emitted via the EventQueue/SSE pattern?
- What's the MicButton overlay integration spot on the pivot post-swiftui-catch-up?

---

## 5. Research findings (12+4 agent synthesis)

Full per-topic detail lives in two memory files:
- `~/.claude/projects/-Users-john-Repos-bobrust/memory/reference_voice_production_patterns.md` — industry-canonical state machine, library picks, thresholds, latency tricks
- `~/.claude/projects/-Users-john-Repos-bobrust/memory/reference_voice_implementation_findings.md` — concrete APIs (SDK wire-level fields, ONNX schemas), BoBe-specific code paths (will need re-mapping on pivot), VPIO config, etc.

Condensed picks per capability:

| Capability | Pick | Threshold/Key Detail |
|---|---|---|
| Acoustic VAD | RMS hysteresis | 700ms silence floor |
| Semantic VAD | smart-turn-v3.0 ONNX | threshold 0.6-0.7 (favor wait); 8s window, zero-pad at START |
| Sentence streaming | LiveKit splitter port | min_ctx_len=10, min_sent_len=20, emit only when ≥2 sentences buffered |
| Filler @ TTFT | 20 phrases, intent-keyed | 800ms trigger; one per turn cap; 80ms crossfade |
| Barge-in | 3-event canonical | cancel/truncate/clear; 3-word minimum; 100ms playback-progress reports; 30ms fade-to-zero on stop |
| Stall watchdog | tokio `Sleep::reset` | 800ms inter-token + 15s total soft + 30s hard; 3-level escalation 800→2400→6000ms |
| Hammering | commit-count during thinking | 4 levels; Levenshtein <0.3 = rephrase dedup; cancel-phrase regex escalates to barge-in |
| Wake-word | openWakeWord custom + sherpa-onnx KWS verifier | ~2.5% CPU continuous on M4 Pro; FPPH 0.5-1.0/h → stage-2 mandatory |
| Audio queue | strict no-overlap | Priority: Reply > Status > Filler > Proactive; .interrupts + reset() between sources |
| AEC | VPIO on BOTH nodes, one engine, mainMixer crossroad | `voiceProcessingOtherAudioDuckingConfiguration` with `.min` for barge-in robustness |
| Testing | 3-tier pyramid | LiveKit fake_* providers; smart-turn-data-v3.1-test on HF; UTMOS + NISQA + WER round-trip |
| Observability | tracing + OSSignposter, turn_id propagation | Rotating JSONL; ring-buffer incident dump opt-in; HUD latency pill; zero outbound network |
| Model distribution | hf-hub + manifest + atomic swap | SHA256 + revision SHA pin; .complete marker; `current →` symlink; NSURLIsExcludedFromBackupKey |
| Adaptive endpointing | **skip** | Industry obviated by smart-turn; ship a manual "pause sensitivity" slider instead (~1hr work) |

---

## 6. Master implementation plan (M4.5.X on pivot)

### Phase -1: Mapping (no code; ~1 day)
Explore pivot architecture before writing anything. See §8.

### Phase 0: Re-build spike capabilities on pivot (~1 week)
Each item below is a SEPARATE COMMIT/PR on the pivot. NOT a rebase.

- **0.a**: New `BoBeService/src/speech/{local_sherpa,local_kokoro,protocol,mod}.rs` — sherpa-rs STT + Kokoro TTS engines. Mostly file copy from spike (these don't depend on pivot architecture).
- **0.b**: New `BoBeService/src/api/handlers/voice.rs` — `/voice/stream` WS handler. SHELL ONLY at this phase: accept WS, decode Opus, dump to a debug PCM buffer. No LLM yet.
- **0.c**: New `BoBeMacUI/BoBe/Voice/` (VoicePipeline + MicButton). Mostly file copy from spike. Integrate MicButton with pivot's overlay UI (which may have moved).
- **0.d**: VAD + state machine in VoicePipeline (M5.1 + M4.5.1 from spike). Client-side only, no pivot collision.
- **0.e**: Wire voice handler to pivot's chat pipeline — the ARCHITECTURE-COUPLED part. Three sub-tasks based on Phase -1 findings:
  - 0.e.i: Discover pivot's chat path (Copilot SDK session creation, send call, event subscription).
  - 0.e.ii: Adapt voice handler to call SDK directly OR through pivot's wrapper.
  - 0.e.iii: Preserve all M4 review fixes (guard scope, markdown strip, empty-drop, pairing invariant).
- **0.f**: Conversation persistence for voice replies. May come for free from pivot's chat path; verify.
- **0.g**: Welcome wizard mic-permission step.

**Gate for Phase 0 done**: end-to-end voice turn works on pivot (mic → STT → SDK → TTS → speaker), voice transcripts appear in overlay history, single-flight guard prevents overlap with text chat. Equivalent to spike's M4.4 + M4.5.1 quality.

### Phase 1: Foundations (parallel; week after Phase 0)

- **M4.5.0a**: Extend pivot's `binary_manager` for voice models. Add `ModelSource` trait with HuggingFaceSource (`hf-hub` crate), manifest with SHA256 + revision SHA pinning, atomic install via `models/<id>/<version>/` + `current →` symlink + `.complete` marker. Move models to `~/Library/Application Support/com.bobe.daemon/models/` (macOS) / `$XDG_DATA_HOME/bobe/` (Linux). `NSURLIsExcludedFromBackupKey`.
- **M4.5.0b**: Tracing + turn_id propagation. Rust `tracing` JSON layer → rotating JSONL at `~/Library/Logs/BoBe/voice-trace.jsonl`. Swift `OSSignposter` + `MetricKit`. Turn_id propagated over WS. Metrics: vad.inference_ms, stt.confidence, eou.delay_ms, llm.ttft_ms, llm.inter_token_gap_ms, tts.ttfb_ms, e2e.latency_ms, barge_in.events, filler.trigger_count. Ring buffer + opt-in incident dump on errors. HUD latency pill. Zero outbound network.
- **M4.5.0c**: Voice settings pane (location depends on pivot Settings layout — likely PREFERENCES group alongside Engine). Soul model: add `voice_pack: Option<String>`, `voice_id: Option<String>`, `voice_speed: Option<f32>`. Active-soul rule: first-enabled-wins for voice. Settings fields: voiceEnabled, wakeWordEnabled, bargeInEnabled, smartTurnEnabled, vadSensitivityDbfs (default -50), vadSilenceMs (default 700), voiceVolume, voicePersona.

### Phase 2: Latency masking + semantic gating (parallel; week 3)

- **M4.5.2** (depends on M4.5.0a): smart-turn-v3.0 ONNX semantic VAD. Daemon-side via `ort` crate. Whisper feature extractor in Rust (n_fft=400, hop=160, 80 mel, do_normalize). Input `[1,80,800]` f32, output `[1,1]` sigmoid prob. Threshold 0.6-0.7. 8s window, zero-pad at START. If P<threshold, new `ContinueListening{reason:"incomplete"}` protocol msg keeps client in `.recording`. Warmup at boot. `max_duration_secs=8` escape hatch. `USE_ONLY_LAST_VAD_SEGMENT=True` Pipecat quirk — implement own buffer retention.
- **M4.5.3 — THE KEYSTONE** (depends on Phase 0 chat-path mapping): sentence-level TTS streaming. Hook into pivot's text-delta stream (form depends on pivot architecture — observer pattern, subscription wrapper, or direct SDK event consumption). Port LiveKit `_basic_sent.split_sentences` (~50 LOC). Sentence buffer state machine (emit only when ≥2 sentences buffered). Markdown stripping via running buffer re-strip + diff (suppress when unclosed fence). Kokoro per-sentence `spawn_blocking`, mpsc(depth=4), single WS writer drains in order. New protocol: `SentenceStarted{idx,msg_id}`, `SentenceEnded{idx,bytes_emitted}`. Adopt pivot's streaming-turn persistence pattern so barge-in saves partial replies.
- **M4.5.4** (depends on M4.5.3): cached fillers. 20 phrases (5 ack-short, 5 generic-think, 7 tool-specific, 3 long-wait). Pre-build to repo at `BoBeService/assets/fillers/`. Decode-once at boot. T1=800ms TTFT, T2=2500ms escalation. Intent-keyed from `tool.execution_start` (ground truth, no prediction). 80ms crossfade. One per turn cap. Rotation no-repeat-last-2.

### Phase 3: Interaction polish (week 4)

- **M4.5.5** (depends on M4.5.3): 3-event barge-in. New client messages: CancelResponse, TruncateAssistant{msg_id, played_ms}, ClearAudioBuffer. Server: Interrupted. Client reports PlaybackProgress every 100ms in `.speaking`. Truncate = min(played_ms_client, samples_emitted_daemon). Backchannel: Pipecat 3-word min; keep audio playing in `.false_interruption_pending`; 2s timeout → resume. **Uses pivot's `session.abort()` natively** — no HTTP/2 hack needed. Plumb cancellation token from voice.rs through pivot's chat path. Kokoro spawn_blocking uninterruptible → sentence-streaming is hard prereq (cancel between sentences). VPIO on BOTH input AND output nodes.
- **M4.5.6** (depends on M4.5.3 + M4.5.4): stall watchdog. Tokio `select!` + `Sleep::reset` on the SDK event stream. 800ms inter-token, 15s total soft, 30s hard. Gate: only fire if TTS queue empty + not playing + 200ms since last audio. 3-level escalation 800→2400→6000ms. Separate stall-filler pool from M4.5.4 (different cadence). Tool-call-aware: suppress inter-token while `tool.execution_active`, use 4s tool-budget timer.

### Phase 4: Voice testing harness (parallel from week 2; ongoing)

- **M4.5.T**: 3-tier test pyramid. Tier 1 (unit, every commit, <30s): fake_stt/fake_tts/fake_vad trait impls; state-machine tests with `tokio::time::pause()`; frame-ordering tests. Tier 2 (per-PR, ~10min): WER on LibriSpeech 50 + 25 conversational (jiwer); UTMOS + NISQA-TTS + Whisper round-trip for Kokoro (block on -0.15 UTMOS or +2% WER); smart-turn-v3.1-test HF dataset accuracy (>85% English); barge-in halt-latency. Tier 3 (nightly, ~30min): synthetic conversation replay; E2E latency SLA gate (P95 EOU+TTFT+TTFB <1.2s); cold-start memory profile.

### Phase 5: Wake-word + proactive voice (M5)

- **M5.1** (depends on M4.5.0a): "Hey BoBe" wake-word. CUSTOM-TRAIN ONNX via Pipecat Colab notebook + piper-sample-generator (mirror archived fork to BoBe-controlled repo first). Stage-2 verifier (sherpa-onnx KWS) MANDATORY because FPPH 0.5-1.0/h. Client-side AVAudioEngine tap → ring buffer → `ort` inference at 80ms hops. Battery: on while charging, off on battery. macOS sleep handling via `IORegisterForSystemPower`. Visible mic indicator + chirp on trigger.
- **M5.2** (depends on M4.5.3 + M4.5.5): hammering pushback + proactive voice routing. AppState gets `voice_session: ArcSwap<Option<VoiceSessionHandle>>`. Voice WS registers/deregisters. Proactive observer also feeds voice TTS sink when present. New `try_begin_proactive_message` guard parallel to `try_begin_user_message`. Hammering: 4-level escalation (silent enqueue → soft → firm → persona+abort+merge). Uses `session.send(Enqueue)` for queue, `session.abort()` for L4. Edit-distance rephrase dedup (Levenshtein <0.3). Cancel-phrase regex escalates to barge-in. Soul integration: `voice_pushback_pack` per persona.

---

## 7. Per-milestone gates

| # | Test gate | Manual check |
|---|---|---|
| Phase 0 | End-to-end voice turn works on pivot; voice transcripts in overlay history | Click mic, speak, hear reply; check chat history shows turn |
| M4.5.0a | Round-trip download/verify/swap for one model; SHA mismatch surfaces | Pull plug mid-download, relaunch, verify resume |
| M4.5.0b | Every WS turn has turn_id in client + daemon logs | Latency pill renders; export bundle produces valid zip |
| M4.5.0c | Settings PATCH round-trips; voice uses chosen voice_id | Switch voice, hear different speaker |
| M4.5.2 | ≥85% accuracy on smart-turn-data-v3.1-test English; first inference <100ms warmed | "…and then I…" doesn't get cut |
| M4.5.3 | TTFA <500ms p95 on 20-utterance suite; sentences in order | Mid-sentence boundary not mangled; abbreviations not split |
| M4.5.4 | Filler fires 800±50ms when LLM delayed | No filler when LLM is fast; crossfade not clicky |
| M4.5.5 | Halt <200ms p95; truncated transcript matches played_ms ±100ms | Coughing/uh-huh doesn't interrupt; "wait" interrupts cleanly |
| M4.5.6 | Stall filler fires at 800ms gap | No filler when sentence still playing |
| M5.1 | FPPH <0.2/h on 1h dogfood recording | Triggers from across room; doesn't fire on "OK Buddy" |
| M5.2 | Hammering escalation at 2/3/4 commits visible in logs | BoBe doesn't talk over user; pushback feels in-character per soul |

---

## 8. Phase -1 mapping checklist (DO THIS FIRST in new worktree)

Before writing any voice code on the pivot, answer these by reading the pivot's codebase:

### Backend (`feat/copilot-sdk-pivot` worktree at `/Users/john/Repos/bobrust/`)

1. **Chat path entry point**: Where does a user message enter the system today? Find the equivalent of `runtime/session.rs::handle_user_message`. (Likely `BoBeService/src/api/handlers/message.rs` or similar, since the pivot exposes `POST /message`.)
2. **SDK session usage**: Where is `github_copilot_sdk::Session` created? How is it stored (per-conversation? singleton?)? Find the call to `session.send(...)`.
3. **Event handling**: Where does the pivot subscribe to `session.subscribe()`? How does it route `assistant.message_delta` events to SSE clients? This IS the equivalent of `response_streamer.rs` on this branch.
4. **Single-flight guard**: Search for `try_begin` or `in_flight` or `UserMessageGuard`. The pivot must have something — text-only would still need it for conversation-state coherence. Find it.
5. **Conversation persistence**: Where are turns persisted? Same `conversation_service.rs` shape, or new? Streaming-turn pattern (begin/push/finalize) exists?
6. **Memory write path**: How is memory.md written today? Find `POST /memory` or `PUT /memory` handler. Understand cadence (per-turn? on-close? nightly only?).
7. **`binary_manager` shape on pivot**: Read `BoBeService/src/binary_manager/{mod,download,extract}.rs`. Understand what it manages today (Ollama runtime). Map out the extension surface for voice models.
8. **Bootstrap pattern**: Read `BoBeService/src/bootstrap/mod.rs` (or wherever startup lives). Find where Ollama is loaded; that's the precedent for loading speech engines.
9. **Tool events**: How are `tool.execution_start/complete` from SDK surfaced to clients? Direct SSE? Wrapped EventQueue? Need to know for filler intent-keying.

### Frontend (`feat/copilot-sdk-pivot` worktree)

10. **Overlay UI**: Where does the chat history render? `BoBeMacUI/BoBe/Views/Overlay/` shape post-swiftui-catch-up.
11. **Settings categories**: `SettingsWindow.swift` enum + groups. Where does Voice slot in (likely PREFERENCES alongside Engine)?
12. **Welcome wizard**: `BoBeMacUI/BoBe/Views/Setup/WelcomeWizard.swift`. Current steps. Where to inject a mic-permission step.
13. **Settings store**: How are settings round-tripped to the daemon today? PATCH `/settings`? DTO shape in `Models/SettingsTypes.swift`.
14. **BobeStore (or its replacement)**: How does the overlay append user/assistant messages? Voice's `appendUserVoiceMessage` analogue.
15. **Mic permission**: Has the pivot's welcome wizard already added a screen-recording permission step? Likely yes. Add mic alongside.

### Output of Phase -1
A short doc (`docs/voice-pivot-mapping.md`) with:
- "Pivot chat path: A → B → C → D" diagram
- File paths for each integration point
- Notes on shape differences vs spike
- Updated tasks #80–#89 descriptions with correct pivot file paths

---

## 9. Top risks

1. **Pivot is moving (74 commits ahead and growing)**: voice work could collide with future pivot churn. Mitigation: do Phase 0 fast (~1 week) to land voice on pivot, then voice + pivot move forward together.
2. **Copilot SDK 0.1.0 is pre-1.0**: breaking changes possible. Mitigation: pin version; watch changelog; abstract behind a thin BoBe-side trait if hot-spots emerge.
3. **`respond_to_message` observer pattern may not exist on pivot**: the spike found a clean observer hook at `proactive_generator.rs:200-228`. The pivot's SDK-native architecture may stream events differently. M4.5.3 (sentence streaming) is the keystone — if the pivot's shape makes observer hooks hard, M4.5.3 takes longer. Mitigation: Phase -1 maps this BEFORE coding M4.5.3.
4. **Memory model change**: spike persists voice transcripts via the pivot's conversation→observation→consolidation path. Pivot uses single-doc memory.md. Mid-reply barge-in may need to truncate the memory.md write to match what user heard. Mitigation: explicit Phase -1 question; treat memory persistence as part of M4.5.5 scope if it doesn't come free.
5. **Single-flight guard on pivot**: spike's `try_begin_user_message` is critical for text+voice non-overlap. The pivot must have an equivalent. If not, M5.2's `try_begin_proactive_message` becomes more work (introducing the guard concept fresh).
6. **openWakeWord licensing**: default pretrained models CC-BY-NC-SA — unusable. Custom training mandatory. Mirror the archived `piper-sample-generator` fork to BoBe-controlled repo before M5.1.
7. **Smart-turn artifact naming**: HF publishes v3.0 + v3.2-cpu only (no v3.1 despite Pipecat docs). Verify before download.
8. **VPIO on output node**: spike only enabled VPIO on input. Pivot rewrite must enable on BOTH for AEC reference signal during barge-in. Easy to forget.

---

## 10. References + memory pointers

### Memory files (persist across sessions)
- `~/.claude/projects/-Users-john-Repos-bobrust/memory/MEMORY.md` — index
- `~/.claude/projects/-Users-john-Repos-bobrust/memory/reference_voice_production_patterns.md` — industry-canonical patterns, library picks
- `~/.claude/projects/-Users-john-Repos-bobrust/memory/reference_voice_implementation_findings.md` — concrete APIs (SDK fields, ONNX schemas, VPIO config); NOTE: BoBe-specific code paths in this file are from THIS branch, will need re-mapping on pivot
- `~/.claude/projects/-Users-john-Repos-bobrust/memory/project_voice_stack_design.md` — pre-spike design doc

### Existing repo docs
- `docs/voice-stack-plan.md` — older pre-research design plan, partially superseded by this doc
- `docs/RUST_GUIDELINES.md` — Rust style guide
- `docs/CODE_SIGNING.md` — Mac signing

### Web-research sources (full lists in agent transcripts in memory file)
Key URLs by topic:
- Smart-turn: https://huggingface.co/pipecat-ai/smart-turn-v3, https://github.com/pipecat-ai/smart-turn
- Copilot SDK: https://docs.rs/github-copilot-sdk/, https://docs.github.com/en/copilot/how-tos/copilot-sdk/
- LiveKit patterns: https://docs.livekit.io/agents/, https://github.com/livekit/agents
- Pipecat: https://docs.pipecat.ai/, https://github.com/pipecat-ai/pipecat
- OpenAI Realtime barge-in: https://platform.openai.com/docs/guides/realtime-conversations
- openWakeWord: https://github.com/dscripka/openWakeWord
- AVAudioEngine VPIO: https://developer.apple.com/documentation/avfaudio/audio_engine/audio_units/using_voice_processing

### Spike commit references (for code archaeology, NOT for rebase)
Voice commits on `worktree-voice-spike-phase1` (all between `5479298..8f23fa2`):
```
8f23fa2 voice(M4.5.1): VAD 700ms + LiveKit-canonical state machine
30c3dd0 voice(M5.1): drop push-to-talk → always-on VAD-gated streaming
72ab8f5 voice(M4 review): conflict-order, empty drop, guard span, markdown strip
b997788 voice(M4.4): voice transcripts through full chat pipeline + UI history
8078335 voice(M4.3): wire LLM into voice handler + M4.2 review fixes
e6b15c6 voice(M4.2): Voice/ module in BoBe + mic button in overlay; M4.1 review fixes
1c7b9b3 voice(M4.1): port spike to BoBeService speech module + nuke spike code
743b535 voice(M3): WS server in voice-spike + Live mode in VoiceSpike app
04b752d voice(M2): scaffold Swift mic + Opus codec mini-app
e50abb1 voice(M1): fix StageTimer.mark() + expand Kokoro voice table
0553f96 voice(M1): wire Kokoro v1.0 lexicon paths + record results
5d6ca47 voice(M1): scaffold daemon-only spike binary with sherpa-rs
```

Use `git show <sha>` from any worktree to inspect spike implementation as reference. Even after `worktree-voice-spike-phase1` branch is deleted, the commits live in `.git` until garbage-collected — but **tag it** before delete to be safe: `git tag voice-spike-archive 8f23fa2`.

---

## 11. Action items, in order

1. ✅ Write this doc (this file).
2. ⏳ Commit this doc to `worktree-voice-spike-phase1` so it's recoverable from git.
3. ⏳ Copy this doc to `/Users/john/Repos/bobrust/docs/voice-production-plan.md` so it's immediately visible on the pivot worktree.
4. ⏳ **Tag the spike branch tip** before deletion: `git tag voice-spike-archive 8f23fa2 -m "voice spike M1-M4.5.1 reference"`.
5. ⏳ Remove the worktree + delete the branch (user confirms): `git worktree remove .claude/worktrees/voice-spike-phase1 && git branch -D worktree-voice-spike-phase1`.
6. ⏳ Create new worktree from pivot: `git worktree add .claude/worktrees/voice-on-pivot feat/copilot-sdk-pivot`.
7. ⏳ In new worktree: do Phase -1 mapping (§8). Write `docs/voice-pivot-mapping.md`.
8. ⏳ Update task DB with corrected scope (re-create #80-#89 with pivot file paths).
9. ⏳ Phase 0 implementation: rebuild M1-M4.5.1 capabilities on pivot, one PR per capability per §6.
10. ⏳ Phase 1+: implement M4.5.0a-M5.2 per the master plan.
