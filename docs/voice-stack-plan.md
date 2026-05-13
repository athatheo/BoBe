# BoBe voice stack — design plan

Status: pre-spike. Research synthesized 2026-05-11. No code written yet.

## 1. Scope

Add full-duplex-ish voice to BoBe: user speaks, BoBe listens, BoBe replies in voice, user can interrupt. Always-on wake-word listening with push-to-talk as alternate input.

Constraints baked into every decision below:

- Rust daemon must build clean on macOS arm64 AND Ubuntu x86_64. No Apple-framework lock-in in the daemon.
- Swift macOS app is the only frontend. It will eventually connect to a remote daemon over the network, so audio transport must be bandwidth-aware (Opus) and not assume localhost.
- BoBe's "brain" stays the Copilot SDK. Voice does not introduce a second LLM provider.
- Locally-hosted models are the default; cloud STT/TTS are swappable per-direction via Settings, mirroring the existing Copilot/Ollama engine pattern.

## 2. Architecture

```
┌─ Swift macOS client ────────────┐    Opus over WS    ┌─ Rust daemon (Mac OR Ubuntu) ─┐
│  AVAudioEngine mic capture      │  ───audio frames──→ │  WS audio endpoint            │
│  AEC (AVAudioEngine voice proc) │                    │  STT engine (local | cloud)   │
│  TEN VAD (sherpa-onnx Swift)    │  ←──TTS audio───── │  TTS engine (local | cloud)   │
│  openWakeWord (TFLite)          │  ──text control──→ │  Voice session state machine  │
│  Opus codec (alta/swift-opus)   │   JSON on same WS  │  Filler cache                 │
│  AVAudioEngine playback         │                    │                               │
│  Client state machine + UI      │                    │  Trait-shaped engines,        │
│                                 │                    │  hot-swappable via ConfigMgr  │
└─────────────────────────────────┘                    └───────────────────────────────┘
```

Two layers of VAD, intentionally:

- **Client coarse VAD** (TEN VAD) = mic gate. Don't ship silence over the wire, don't ship ambient audio when nobody's talking. ~30ms decisions. Cheap.
- **Server-side endpointing** = "is this utterance complete?". `sherpa-rs` streaming STT already exposes `is_endpoint()` — linguistic, model-aware, far better than naive silence-based endpointing. Free.

Wake-word is client-only (privacy + bandwidth — never stream ambient audio to a daemon).

AEC is client-only (the daemon may live on a server with no access to the speaker reference signal).

## 3. State machine

Adopt LiveKit's canonical state vocabulary verbatim — it's the de-facto standard and maps cleanly to the existing overlay's avatar animation.

```
                    ┌──────────────────────────────────┐
                    │ Idle (mic open, wake-word listen)│◄──────────────┐
                    └────────────┬─────────────────────┘               │
                                 │ wake-word fires OR push-to-talk     │
                                 ▼                                     │
                    ┌──────────────────────────────────┐               │
                    │ Capturing (audio→Opus→WS→STT)    │               │
                    │ STT emits partials over SSE      │               │
                    └────────────┬─────────────────────┘               │
                                 │ is_endpoint() = true                │
                                 ▼                                     │
                    ┌──────────────────────────────────┐               │
                    │ Thinking (Copilot session.chat)  │◄─┐            │
                    │   filler timer = 800ms           │  │            │
                    │     ↳ stream cached filler clip  │  │            │
                    │     ↳ NEVER call TTS to make it  │  │            │
                    └────────────┬─────────────────────┘  │            │
                                 │ first text_delta       │            │
                                 ▼                        │            │
                    ┌──────────────────────────────────┐  │            │
                    │ Speaking (Kokoro→Opus→WS→play)   │  │ BTW: user  │
                    │   barge-in detector running      │  │ kept       │
                    │   wake-word kept armed (AEC'd)   │  │ talking    │
                    └────────────┬─────────────────────┘  │            │
                                 │                        │            │
              ┌──────────────────┼────────────────────────┘            │
              │                  │                                     │
              │ barge-in         │ EOT clean                           │
              │ (≥2 words)       │                                     │
              ▼                  ▼                                     │
   ┌──────────────────┐  ┌──────────────────┐                          │
   │ Interrupted      │  │ 1.5s grace window│ ── continuation ───┐     │
   │ - halt TTS       │  │ - if user speaks │                    │     │
   │ - cancel chat    │  │   within 1.5s,   │                    │     │
   │ - truncate turn  │  │   merge as same  │                    │     │
   │   in mem to what │  │   turn (BTW)     │                    │     │
   │   user heard     │  │ - else new turn  │                    │     │
   └────────┬─────────┘  └────────┬─────────┘                    │     │
            │                     │                              │     │
            ▼                     ▼                              ▼     │
   (back to Capturing)    (back to Idle)             (back to Thinking)│
                                                                       │
                                                       ───────────────┘
```

States (publish over SSE as `agent.state.changed { state, ts }`):

| State | Meaning | UI hint |
|---|---|---|
| `idle` | Mic open, wake-word listening | Dim avatar |
| `listening` | Wake-word fired, opening capture | Avatar pulse |
| `capturing` | User speaking, audio streaming up | Waveform |
| `thinking` | LLM working | Spinner / "thinking" |
| `speaking` | TTS playing back | Mouth animation |
| `tool_calling` | Copilot SDK tool dispatch | Tool icon |

Additional event types (LiveKit vocabulary):
`user_speech_started`, `user_stopped_speaking`, `agent_started_speaking`, `agent_stopped_speaking`, `agent_speech_interrupted`, `function_calls_collected`, `function_calls_finished`.

## 4. Behavior specs

### 4.1 Filler when LLM is slow

- Cache 6–10 pre-synthesized filler audio clips at install (Kokoro TTS → Opus → `~/.bobe/cache/fillers/*.opus`). Examples: "Hmm…", "Let me think for a second…", "One moment…", "Working on that…".
- Daemon timer: at **800ms** since STT-final with no `text_delta` yet, pick a random unused filler and stream it on the audio channel. Mark it played so we don't repeat within the session.
- The filler exists to mask the LLM's TTFT. **Never** generate the filler with TTS on the fly — that's the same round-trip cost we're trying to mask. Pre-cache is non-negotiable.
- Source: LiveKit recommends pre-synthesized cached audio for fixed phrases ([LiveKit docs](https://docs.livekit.io/agents/multimodality/audio/)). Filler emission threshold derived from Forasoft's tiered latency framework: below 300ms feels human, 300–600ms acceptable, >600ms noticed, >1.5s disengaged. We pick the midpoint.

### 4.2 Barge-in (user interrupts agent)

Two-layer detection:

1. **Client-side VAD** posts `user_speech_started` within ~50ms. Fast, but high false-positive on backchannels ("mm-hmm", coughs).
2. **Server-side word-count gate**: only act on the barge-in once STT partial has 2+ words. Discards backchannels. (Pipecat's `MinWordsInterruptionStrategy` pattern.)

On confirmed barge-in:

- Daemon aborts the in-flight Copilot SDK `chat()` future immediately.
- Daemon stops the Kokoro TTS stream and the WS audio channel.
- Daemon emits `agent_speech_interrupted { played_bytes }` so the client knows how much audio actually reached the speaker.
- **Critical**: truncate the assistant turn in the conversation memory (memory.md / chat history) to match what the user actually heard. Otherwise the next LLM call references content the user never received → coherence breaks.

Target: <200ms from user-speech-start to TTS-halt. (Retell 2025 production benchmark.)

### 4.3 Queuing / BTW injection

Three sub-cases based on where the user starts talking:

1. **During `Speaking`** = barge-in. Halt + truncate + re-capture. (§4.2.)
2. **Within 1.5s after agent EOT** = continuation. Merge with previous turn — concat the new utterance onto the same Copilot SDK session as a follow-up `chat()`. (1.5s threshold derived from ElevenLabs auto-merge behavior.)
3. **After 1.5s** = new turn. Just a fresh `chat()` on the same session.

If the agent has finished text generation but is still tool-calling when the user speaks: cancel in-flight tool, resubmit a combined prompt. (Pipecat issue #2791 documents this pain point — agent monologues + topic-change interrupts.)

The Copilot SDK supports queueing multiple `chat()` calls to the same session, which lets us implement this without architectural change.

### 4.4 Proactive speaking (BoBe-specific)

BoBe already proactively generates content via `ProactiveGenerator` (check-ins, goal triggers). When voice is enabled, those proactive turns should be voice-able too — BoBe speaks unsolicited.

Implementation: `ProactiveGenerator` → produces text → if voice mode active, route through TTS → state transitions `idle → speaking`. Wake-word stays armed (AEC'd) so user can interrupt with "wait" or similar.

This means the state machine `speaking` state is entered from two sources: user-initiated (after `thinking`) AND proactive-initiated (from `idle`).

## 5. Latency budget

Per-stage targets (Telnyx voice-AI budget framework):

| Stage | Budget | Where |
|---|---|---|
| Mic → VAD decision | ≤50ms | Client |
| Endpoint silence threshold | 400ms (adaptive EMA) | Daemon |
| STT first partial | ≤300ms | Daemon |
| LLM TTFT (Copilot SDK) | ≤600ms | Daemon |
| Filler trigger | 800ms | Daemon |
| TTS first audio chunk | ≤250ms | Daemon |
| Network leg | ≤150ms | Both |
| **End-to-end p90 target** | **<3.5s** | — |
| **Barge-in halt** | **<200ms** | — |

Wake-word multi-phrase confidence (openWakeWord):
- `"hey bobe"` (3 syl) threshold 0.5 default
- `"bobe" / "bo-be"` (2 syl) threshold 0.7 (stricter — short triggers FAR higher)
- 200ms hangover window post-wake to catch trailing audio

## 6. Library + model picks

| Stage | Where | Pick | License | Notes |
|---|---|---|---|---|
| Mic capture | Swift | `AVAudioEngine` | Apple | Built-in |
| AEC | Swift | `setVoiceProcessingEnabled(true)` | Apple | Built-in. Linux desktop variant (future): [`tonarino/webrtc-audio-processing`](https://github.com/tonarino/webrtc-audio-processing) |
| VAD | Swift | TEN VAD via [`sherpa-onnx` Swift](https://github.com/k2-fsa/sherpa-onnx) | Apache 2.0 | <50ms end-of-speech |
| Wake-word | Swift | [openWakeWord](https://github.com/dscripka/openWakeWord) TFLite | Apache 2.0 | Train via [atlas-voice-training](https://github.com/briankelley/atlas-voice-training) Docker. Single model on all 3 phrase variants → lower aggregate FAR |
| Opus codec | Swift | [`alta/swift-opus`](https://github.com/alta/swift-opus) | BSD | 20ms / 16kHz / 16–24kbps VBR / `OPUS_APPLICATION_VOIP` |
| WS transport | both | `tokio-tungstenite` / `URLSessionWebSocketTask` | — | Binary frames for Opus, text JSON for control |
| STT local | Rust | [`sherpa-rs`](https://docs.rs/sherpa-rs/) + Moonshine-base-en or streaming-zipformer-en | MIT | Native streaming-partial API; CoreML EP on Mac, CUDA EP on Linux |
| STT cloud | Rust | Azure Speech (raw WS) / Deepgram Nova-3 | — | Azure: free 5h/mo + 0.5M chars; user has access; WEU sub-10ms RTT |
| TTS local | Rust | **`sherpa-rs` (`tts` feature) + Kokoro v1.0** | Apache 2.0 | sherpa-rs wraps both Kokoro inference AND espeak-ng phonemization in one crate — simpler than `ort` + DIY phonemizer. Confirmed working in M1 scaffold (cargo check ✓). Fallback: [`ort`](https://github.com/pykeio/ort) 2.0 + [Kokoro-82M ONNX](https://huggingface.co/onnx-community/Kokoro-82M-v1.0-ONNX) direct if sherpa-rs Kokoro proves inflexible |
| TTS cloud | Rust | Azure Neural / ElevenLabs Flash v2.5 | — | Flash v2.5 = 75ms TTFB if voice cloning matters |

Model assets to install (mirror existing Ollama install UX):
- Moonshine-base-en (~70MB int8)
- Kokoro-82M v1.0 (~85MB int8)
- TEN VAD weights (~30MB)
- openWakeWord "Hey BoBe" + variants (~600KB total)
- 6–10 pre-cached filler Opus clips (~100KB total)

Total ~290MB. Acceptable. Reuse `BinaryManager` + the Ollama install service pattern for resumable downloads + progress UX.

## 7. Daemon code shape

New modules:

```
BoBeService/src/
  speech/
    mod.rs              # SpeechProvider traits, error types, events
    stt.rs              # SttProvider trait, SttEvent enum
    tts.rs              # TtsProvider trait, TtsAudioChunk
    local_sherpa.rs     # sherpa-rs Moonshine impl
    local_kokoro.rs     # ort + Kokoro ONNX impl
    azure_stt.rs        # Azure Speech STT (raw WS)
    azure_tts.rs        # Azure Speech TTS (raw WS)
    deepgram_stt.rs     # Deepgram Nova-3 (STT only)
    elevenlabs_tts.rs   # ElevenLabs Flash v2.5 (TTS only)
    filler_cache.rs     # Pre-synthesized filler management
  runtime/
    voice_session.rs    # Per-WS-connection voice state machine
  api/handlers/
    voice.rs            # WS endpoint + control routes
```

Trait sketch:

```rust
pub(crate) enum SttEvent {
    Partial { text: String, confidence: f32 },
    Final { text: String },
    EndOfUtterance,
    Error(String),
}

#[async_trait]
pub(crate) trait SttProvider: Send + Sync {
    async fn stream(
        &self,
        audio: impl Stream<Item = Vec<u8>> + Send,
    ) -> Result<BoxStream<'static, SttEvent>, AppError>;
}

pub(crate) struct TtsAudioChunk {
    pub opus_bytes: Vec<u8>,
    pub is_final: bool,
}

#[async_trait]
pub(crate) trait TtsProvider: Send + Sync {
    async fn stream(
        &self,
        text: impl Stream<Item = String> + Send,
    ) -> Result<BoxStream<'static, TtsAudioChunk>, AppError>;
}
```

Config additions to `EngineConfig` (all hot-swappable via existing `ConfigManager`):

```toml
stt_engine = "local"              # local | azure | deepgram
stt_cloud_endpoint = ""
# stt_cloud_key — keychain only, NEVER config.toml

tts_engine = "local"              # local | azure | elevenlabs
tts_cloud_endpoint = ""
# tts_cloud_key — keychain only

wake_word_phrases = ["hey bobe"]
wake_word_short_aliases = false   # opt-in: "bobe" + "bo-be"
voice_input_mode = "wake_word"    # wake_word | push_to_talk
voice_output_enabled = true       # master toggle for TTS

filler_threshold_ms = 800
barge_in_min_words = 2
continuation_window_ms = 1500
endpoint_silence_ms = 400
```

WS endpoint: `GET /voice/stream` upgrades to WS. Per-connection: spawn a `VoiceSession` actor that owns:

- one `Arc<Session>` to the Copilot Chat worker (BoBe's brain)
- one `SttProvider` + one `TtsProvider` (selected from `EngineConfig`)
- state machine (initially `Idle`)
- filler timer + barge-in word-count gate
- continuation grace timer

Hot-swap path: `ConfigManager` listener fires on `stt_engine` / `tts_engine` change → registry rebuilds providers → existing voice sessions: option (a) tear down + reconnect (simplest), option (b) swap mid-session (preserves session continuity). Recommend (a) for v1.

## 8. Client code shape

```
BoBeMacUI/BoBe/
  Voice/
    VoicePipeline.swift          # State machine controller
    MicCapture.swift             # AVAudioEngine + voice processing
    VadGate.swift                # TEN VAD via sherpa-onnx Swift
    WakeWordDetector.swift       # openWakeWord (TFLite) wrapper
    OpusCodec.swift              # alta/swift-opus encode/decode
    VoiceWebSocket.swift         # WS audio + control framing
    TtsPlayback.swift            # AVAudioEngine output + barge-in halt
    VoiceModels.swift            # Manage ONNX/TFLite asset install
  Stores/
    VoiceStore.swift             # @Observable voice state for overlay
  Features/Settings/
    VoicePanel.swift             # STT/TTS engine pickers, wake-word phrases, cloud keys
  Resources/i18n/
    UI.strings additions: voice.* namespace
```

Swift state machine mirrors daemon's, plus client-only states for asset install:
`assetsMissing → installing → ready → idle → listening → capturing → thinking → speaking → interrupted`

The daemon is the source of truth for `thinking | speaking | tool_calling`; the client owns `idle | listening | capturing` and reconciles with the daemon's broadcasts.

Backend URL config in Swift (separate small task, but related to the future-remote constraint):
- `Models/SettingsTypes.swift` add `backendUrl: String` (default `http://127.0.0.1:8766`)
- `Services/BackendService.swift` reads from settings + validates + reconnects on change
- `Features/Settings/AdvancedPanel.swift` add field with URL validator

## 9. Spike scope (Mac-only first)

Three phases. Each is a separate worktree, each is throwaway code unless it works clean enough to merge.

### Phase 1: Daemon-only Rust spike

**Goal:** validate that sherpa-rs + ort+Kokoro hit latency budgets on M4 Pro before any client work.

**Build:** new workspace member `bobrust/voice-spike/`. CLI binary `voice-spike`:

```bash
$ voice-spike \
    --in clip.wav \
    --stt sherpa --stt-model moonshine-base-en \
    --tts kokoro --tts-voice af_bella \
    --out reply.wav
[VAD]   trimmed 5.2s → 3.1s of speech in 12ms
[STT]   "what's on my calendar today" — first partial 217ms, final 287ms
[TTS]   reply.wav 4.8s audio — first chunk 234ms, total 412ms
[total] 699ms
```

**Measures:**
- STT first-partial latency on a representative clip
- STT final latency (with `is_endpoint()`)
- TTS time-to-first-audio
- TTS total synthesis time vs audio duration (RTF)
- Resident memory at idle vs hot
- Model load times (cold start)

**Validates:** are the model picks viable on M4 Pro? Pass criterion: all stage budgets met in §5.

### Phase 2: Swift mic + Opus spike

**Goal:** validate Swift-side capture + VAD + Opus codec round-trip independently.

**Build:** Xcode standalone target `BoBeMacUI/VoiceSpike` (separate from the main BoBe target). Single-window app:

- Big "Start" button → AVAudioEngine mic capture → TEN VAD trim → alta/swift-opus encode → write `~/Desktop/voice-spike-out.opus`
- "Play" button → read Opus file → decode → AVAudioEngine playback
- Show real-time VAD speech-detect indicator
- Print capture-to-encode latency in console

**Measures:**
- VAD speech-detect → encoded-bytes latency
- AEC effectiveness (capture while playing back, validate no feedback)
- Opus codec round-trip quality (subjective)
- Bytes/sec at 16kbps and 24kbps

**Validates:** Swift audio pipeline works end-to-end without daemon involvement.

### Phase 3: Wire-up integration (only if 1+2 pass)

**Goal:** end-to-end voice loop.

**Build:** WS handler in spike daemon + Swift mini-client that uses Phase 2's pipeline pointed at the WS.

- Mic → VAD → Opus → WS → STT → text → TTS → Opus → WS → playback
- Implement state machine (basic — `idle / capturing / thinking / speaking`)
- Implement filler trigger at 800ms
- Implement barge-in (single-layer VAD only for spike)
- Implement continuation merge

**Measures:**
- End-to-end perceived latency
- Filler activation rate (how often LLM exceeds 800ms)
- Barge-in halt latency
- Subjective: does it feel like a conversation?

**Validates:** the design works in practice. Pass criterion: <3.5s p90 end-to-end, <200ms barge-in halt.

### Deferred (post-spike, in real planning)

- Wake-word integration (use push-to-talk in spike)
- openWakeWord model training for "hey bobe" + variants
- Linux validation (only after M4 Pro numbers prove out)
- Cloud STT/TTS implementations
- Proactive-speaking integration with existing `ProactiveGenerator`
- Backend URL config in Swift client (related but separate small task)
- Full i18n (en-only for spike)

## 10. Risk register

| Risk | Severity | Mitigation |
|---|---|---|
| Kokoro voice doesn't fit BoBe's character | Medium | Phase 1 explicitly samples 3+ voices; if all wrong, fall back to ElevenLabs Flash v2.5 (premium but it's the right voice) |
| `ort` 2.0 still rc with API churn | Medium | Pin specific rc, prepare fallback to `kokoroxide` or sherpa-rs TTS (Piper, quality tier down but stable) |
| AEC on macOS not strong enough during loud playback | Medium | Mute wake-word during `speaking` state as a guardrail; only re-arm post `agent_stopped_speaking` |
| Linux GPU acceleration limited to CUDA EP | Low | Spike doesn't touch Linux. Plan investigation post-Mac validation. CPU-only baseline is acceptable for low-volume use |
| `azure-speech` Rust crate is "early stage" | Low | Cloud impls are post-spike. If crate isn't stable, drop to raw WS against documented Azure endpoints |
| Opus over flaky network = no congestion control | Low | Not a localhost-daemon concern. Flag for remote-daemon work |
| Wake-word commercial licensing (Porcupine $6k/yr) | High if shipped wrong | openWakeWord by GA. Porcupine only allowed during personal-spike phase |
| Filler clips feel robotic / repetitive | Medium | Cache 10+ variants, pick random unused-this-session, vary voice slightly |
| Conversation truncation on barge-in breaks Copilot SDK context | High | Must implement turn-truncation in memory.md / chat history; validate Copilot SDK accepts it cleanly |
| Wake-word multi-phrase FAR compounds | Medium | Single openWakeWord model trained on all variants > separate models per phrase |
| Daemon's existing host-validation middleware blocks WS upgrade | Low | Add `/voice/stream` to the allow-list; validate origin same as REST endpoints |

## 11. Open decisions (need user input)

1. **Wake-word for spike**: Porcupine (fast DX, $0 personal) or jump straight to openWakeWord (slower but ship-safe)?
2. **Voice character for Kokoro**: which voice slot do we sample in Phase 1? (af_bella, af_sarah, am_michael — defaults to female warm-toned. BoBe is currently genderless.)
3. **Filler clip personality**: tone for the 10 cached fillers — neutral ("one moment"), characterful ("hmm, let me think"), or both?
4. **First spike target machine**: just M4 Pro, or also any Linux validation in scope for Phase 1?
5. **Should Phase 1 also test cloud STT/TTS** (Azure) as a side-by-side latency comparison? Quick to add given Azure access is already there.

## 12. Deferred smart tricks (post-architecture)

These are perception-latency improvements and conversational robustness features. They are explicitly **deferred until §13 milestone M3 is complete and the end-to-end voice loop works**. Captured here so we don't lose the ideas.

### 12.1 Adaptive filler tiers

A single filler at 800ms is the v1. Real conversations need escalation as the wait grows:

| Tier | Trigger | Content |
|---|---|---|
| T1 | 800ms TTFT | Generic short clip: "Hmm…", "One sec…" |
| T2 | 1500ms TTFT | Status-aware: "Still reading your screen…", "Checking your calendar…", "Looking at your notes…" |
| T3 | 3000ms TTFT | Explicit acknowledgment: "Hold on, I'm working on it.", "Give me a moment, this needs more thought." |

T2 requires knowing **what BoBe is doing** — i.e., which tool is currently dispatched. The Copilot SDK already emits `tool_call_start` events; we can map tool names to status phrases ("reading screen" for screen-capture tool, "checking calendar" for calendar tool, etc.).

### 12.2 "I'm gonna take a long time" predictor

Estimate slowness *before* the LLM starts producing tokens, so we can play a longer-form preamble:

- Heuristic signals: prompt length, presence of tool calls likely needed (file edits, web fetches, screen analysis), recent latency moving average per tool
- If predicted total > 3s: emit a tier-1 filler immediately at STT-final, not at 800ms
- If predicted total > 8s: switch tone — "This is a big one, give me a moment to think it through."

Implementation hook: add a lightweight classifier in `MessageHandler` between STT-final and LLM dispatch. Could be hardcoded heuristics initially; a small model later.

### 12.3 Hammering protection

User keeps voice-hammering while BoBe is still processing the first request. State: `Thinking` + new `user_speech_started` event.

Behaviors:
- First hammer (within 2s of original): treat as BTW, queue for merge into the in-flight Copilot SDK call
- Second hammer (within 4s): emit an interrupting filler — "I heard you, working on it, give me a sec." — and continue
- Third hammer (within 8s): full barge-in. Halt, gather all queued input, resubmit combined. Daemon ack: "OK, restarting with all that."

### 12.4 Midway status updates ("status streaming")

For long LLM responses (tool-call-heavy or extended generation), emit BoBe's internal narration as separate audio:

- "Reading your screen now…" → "Found the relevant section." → (TTS reply continues)
- Implementation: hook into `tool_call_start` / `tool_call_complete` events, generate one-line status, TTS it on a separate priority lane
- Critical: must NOT overlap with the main reply audio (see §12.5)

### 12.5 Audio stream stacking / no-overlap policy

Strict invariant: **exactly one audio stream plays at a time per session**. Filler, status, reply, and proactive audio all share one output queue.

- Each audio stream has a priority: `Reply > Status > Filler > Proactive`
- Higher-priority arrival cancels lower-priority in-flight (smooth fade-out ~80ms)
- Within same priority: FIFO, no preemption
- Filler that's already played past 50% finishes; one that just started is cut short
- Client-side: maintains a single `AVAudioPlayerNode` per voice session; daemon-side: a `VoiceAudioQueue` actor schedules and emits start/cancel events

This is **the** tricky part the user called out. It's the difference between "feels like an assistant" and "feels like a mess".

### 12.6 Slow-LLM detection (in-flight)

Once tokens start streaming, monitor inter-token gap:

- Normal gap: <200ms per token
- Stalled: >500ms with no new token → emit micro-filler ("…uh…") to bridge gap
- Failed: >5s gap → assume LLM failure, emit "I lost my train of thought, let me restart" + cancel + retry

### 12.7 Speech rate adaptation

Some replies should be read slower (technical content) or faster (casual acknowledgments). Kokoro supports `speed` parameter. Implementation: a lightweight classifier on reply text picks rate. Default 1.0×; up to 1.2× for "got it" / "okay" type ack; down to 0.85× for technical/numeric content.

### 12.8 Voice activity learning

Per-user: how long does *this* user typically pause mid-utterance vs end-of-turn? Adapt endpoint-silence threshold per user. Default 400ms, range 200–800ms. Learn from utterance pause patterns over time.

## 13. Execution roadmap

Phased so each milestone lands a green build and a testable artifact. Each milestone is a separate worktree off `main`, merged back via PR before the next starts.

### M0 — Plan finalization (this conversation)

- ✅ Research synthesized
- ✅ `docs/voice-stack-plan.md` written
- ✅ Memory pointers added
- ⏳ User confirms defaults for the 5 open decisions in §11

### M1 — Phase 1 spike: daemon-only Rust (1-2 days)

**Branch:** `feat/voice-spike-phase1` (worktree)

**Deliverable:** `voice-spike` CLI binary in a new workspace member at `voice-spike/`.

Steps:
1. Worktree from origin/main
2. Add `voice-spike/` to root `Cargo.toml` workspace members
3. `voice-spike/Cargo.toml` with `sherpa-rs`, `ort`, `hound`, `clap`, `anyhow`, `tokio`
4. `src/main.rs`: clap CLI with `--in clip.wav --out reply.wav --stt-model X --tts-voice Y`
5. `src/stt.rs`: sherpa-rs streaming Moonshine wrapper, emits partials + final via channel
6. `src/tts.rs`: ort + Kokoro-82M ONNX wrapper, emits PCM chunks
7. `src/timing.rs`: stage timer with named spans, print summary table
8. Download script for models (`scripts/voice-spike-fetch-models.sh`)
9. Sample WAV in `voice-spike/samples/`
10. Run + measure on M4 Pro
11. Document findings in `voice-spike/RESULTS.md`

**Pass criterion:** STT first-partial <300ms, TTS first-audio <250ms on M4 Pro for a representative 5s utterance.

**Out of scope:** WS, Swift, AEC, wake-word, fillers, state machine.

### M2 — Phase 2 spike: Swift mic+Opus mini-app (1-2 days)

**Branch:** `feat/voice-spike-phase2` (worktree from main)

**Deliverable:** Xcode standalone target `BoBeMacUI/VoiceSpike` with mic capture + VAD + Opus round-trip.

Steps:
1. New Xcode target separate from main BoBe target
2. SPM deps: `alta/swift-opus`, `sherpa-onnx` Swift binding
3. AVAudioEngine input node with `setVoiceProcessingEnabled(true)` (AEC + AGC)
4. TEN VAD via sherpa-onnx Swift on input frames
5. Opus encoder at 16kHz mono 16kbps VBR
6. On VAD trim: write `.opus` to ~/Desktop
7. Decoder + AVAudioEngine output: read `.opus`, play through speakers
8. Realtime status UI: speech-detect indicator, byte counter, AEC validation
9. Document findings in `BoBeMacUI/VoiceSpike/RESULTS.md`

**Pass criterion:** Round-trip Opus encode + decode + playback works cleanly. AEC suppresses speaker feedback while mic is open. Encode latency <50ms per 20ms frame.

**Out of scope:** WS, daemon, wake-word, network transport.

### M3 — Phase 3 spike: end-to-end WS integration (3-4 days)

**Branch:** `feat/voice-spike-phase3` (worktree from main, cherry-picks artifacts from M1+M2)

**Deliverable:** Working voice loop. User speaks → BoBe transcribes → Copilot SDK replies → BoBe speaks.

Steps:
1. Daemon: `BoBeService/src/speech/{mod,stt,tts,local_sherpa,local_kokoro}.rs` with traits + impls
2. Daemon: `BoBeService/src/runtime/voice_session.rs` — per-WS actor, basic state machine (`idle | capturing | thinking | speaking`)
3. Daemon: `BoBeService/src/api/handlers/voice.rs` — WS upgrade endpoint at `/voice/stream`
4. Daemon: Wire `VoiceSession` to existing `WorkerRegistry` chat session for LLM
5. Client: `BoBeMacUI/BoBe/Voice/VoicePipeline.swift` + sub-modules
6. Client: `VoiceWebSocket.swift` for WS transport (binary Opus + text JSON control)
7. Client: state machine mirror, `VoiceStore` `@Observable`
8. Push-to-talk button in overlay (NOT wake-word for now)
9. Basic filler at 800ms with one pre-cached clip
10. Basic barge-in via client VAD only (no min-words gate yet)
11. Document findings in `docs/voice-stack-spike-results.md`

**Pass criterion:**
- Push button → speak → BoBe transcribes → Copilot replies in text → BoBe speaks reply
- End-to-end p90 < 4s (relaxed from 3.5s for spike)
- Filler activates on slow turns
- Barge-in halts TTS within 300ms (relaxed from 200ms for spike)

**Out of scope:** Cloud engines, wake-word, AEC tuning, multi-tier fillers, conversation memory truncation, proactive speaking, Settings UI.

### M4 — Production integration (1-2 weeks)

**Branch:** `feat/voice-stack` (worktree from main; spike code lifted and adapted into production locations).

Production reimplementation. Spike directories (`voice-spike/`, `BoBeMacUI/VoiceSpike/`) deleted; their patterns lifted into the real module locations.

Major chunks:
- M4.1: Real `speech/` module with all 3 STT engines (`local`, `azure`, `deepgram`) + 3 TTS engines (`local`, `azure`, `elevenlabs`)
- M4.2: Full state machine with all LiveKit events, conversation memory truncation on barge-in, 2-layer barge-in detection (client VAD + server min-words)
- M4.3: Wake-word integration — openWakeWord with "hey bobe" + variants, trained via atlas-voice-training Docker
- M4.4: Settings UI — `VoicePanel.swift` with engine pickers, wake-word phrase toggles, cloud keys (keychain), voice character selection
- M4.5: i18n — `voice.*` namespace across 9 locales
- M4.6: Asset install pipeline — mirror Ollama install service; resumable downloads with progress UX
- M4.7: Proactive speaking integration with `ProactiveGenerator`
- M4.8: Backend URL config in Swift client (the related task that surfaced during voice planning)

Each M4.x lands as a separate PR. Build stays green throughout.

**Pass criterion:** Full plan in §1-§11 implemented, smoke-tested end-to-end on M4 Pro + Linux x86_64.

### M4.5 — Production-grade voice (table-stakes the M5.1 naive baseline missed)

Pushback from user (2026-05-11): "500ms is too aggressive, lifecycle is too naive, latency tricks aren't all M5." Research confirmed: real systems combine acoustic+semantic VAD, stream TTS by sentence, cache filler audio, gate barge-in on word count, and run a state machine richer than ready→recording→thinking→speaking. These are **table-stakes** for production voice, not polish — moved out of M5 into M4.5.

**M4.5.1 — Acoustic threshold floor + state-machine expansion**
- Bump silence-end from 500ms → **700ms** (LiveKit production floor; Silero default 100ms is too aggressive; 500ms cuts mid-sentence on natural pauses).
- Expand state machine to LiveKit's canonical set:
  `initializing | idle | listening | committing | thinking | tool_executing | synthesizing | speaking | speaking_with_queue | false_interruption_pending | interrupted_draining | stalled | failed`
- Add hysteresis around backchannel ("yeah", "mhm") — pause but don't commit on brief noise (`false_interruption_pending` with `false_interruption_timeout=2s` LiveKit default).

**M4.5.2 — Semantic turn detection (Pipecat smart-turn-v3.1)**
- 8MB int8 ONNX model, ~12ms CPU inference on M-class — fits BoBe's local-first stance.
- Source: [pipecat-ai/smart-turn-v3 HF](https://huggingface.co/pipecat-ai/smart-turn-v3).
- Hybrid layer: acoustic VAD (Silero in sherpa-rs) trips silence at 700ms → smart-turn confirms "linguistically complete" before commit. If smart-turn says "incomplete utterance", reset silence timer.
- Daemon-side, runs in spawn_blocking like sherpa-rs.

**M4.5.3 — Sentence-level streaming TTS**
- Mandatory per [arXiv 2508.04721] + LiveKit ElevenLabs plugin `auto_mode=true` default.
- Daemon: chunk reply text on sentence boundaries (`.!?` w/ abbreviation guard), synthesize each as Kokoro emits, stream Opus packets as ready. Cuts TTFA by 1-3s for long replies.
- Voice handler refactors `stream_opus` into a per-sentence pipeline.

**M4.5.4 — Pre-cached filler audio + 800ms trigger**
- 10-20 generic + intent-keyed phrases ("Sure", "Let me check", "One moment", "Looking at your calendar"). Pre-rendered Kokoro WAVs in `~/.bobe/cache/fillers/*.opus`, indexed by intent.
- Trigger: if no first sentence from TTS within **800ms** of STT-final, stream a filler. Don't generate filler with TTS on the fly — that defeats the point.
- Cited thresholds: gaps >800ms drop trust; >1500ms feel broken.

**M4.5.5 — Backchannel-resistant barge-in**
- Pipecat `MinWordsUserTurnStartStrategy`: do not interrupt agent on <3 words. Backchannels ("yeah", "uh-huh") cause false barge-ins otherwise.
- 3-event sequence (OpenAI Realtime canonical): cancel-LLM-stream → truncate-assistant-transcript-to-spoken-bytes → clear-tts-audio-buffer.
- Daemon-side abort: current worktree (off main) uses `LlmProvider.stream()` which has no cancellation primitive. Either (a) add `Cancel` token to LlmProvider, or (b) abandon in-flight stream (LLM tokens wasted but simpler v1). Pick (b) for M4.5; (a) when `feat/copilot-sdk-pivot` lands.

**M4.5.6 — Inter-token gap watchdog**
- Detect LLM stalls. Threshold: **>500ms between tokens during streaming = stall**. Emit a wait-filler ("Still working on that…") from the cache. GitHub Copilot CLI specifically documented to have ~60s idle silent-kill behavior.

### M5 — Wake-word + behavior polish

**M5.1 — Wake-word "Hey BoBe"** (previously M5.2): client-side keyword detector when mic permission granted but WS not connected. Auto-connect on detection. openWakeWord (TFLite, free, custom-train via atlas-voice-training) or Picovoice Porcupine (faster DX, commercial license — $6k/yr enterprise; ship-blocker, prefer openWakeWord).

**M5.2 — Hammering pushback**
- Signal: **count of distinct commits during one `thinking` phase** (not time-since-commit, not audio energy).
- 2nd commit → "I heard you, working on that…" (canned)
- 3rd → "Hold on, give me a moment to think this through" (canned)
- 4th+ → BoBe-character pushback ("Hey — let me finish this thought first")
- Source: Dasha.AI interrupt handling, Flux IT case study.

**M5.3 — Polish tricks** (subset of §12):
- Parallel SLM filler generation (WebRTC.ventures pattern)
- Speculative tool calling (getstream.io)
- Tool-call progress streaming ("Looking at your calendar…", "Found 3 events…")
- Adaptive endpoint silence per user (voice activity learning)
- Audio queue no-overlap policy (Reply > Status > Filler > Proactive)
- Speech rate adaptation per content type

## 14. References

- LiveKit Agents docs — https://docs.livekit.io/agents/multimodality/audio/
- LiveKit AgentState reference — https://docs.livekit.io/reference/agents-js/types/agents.voice.AgentState.html
- LiveKit turn detection — https://livekit.com/blog/turn-detection-voice-agents-vad-endpointing-model-based-detection
- OpenAI Realtime VAD — https://platform.openai.com/docs/guides/realtime-vad
- Pipecat interruption strategies — https://docs.pipecat.ai/server/utilities/interruption-strategies
- Pipecat issue #2791 (monologue interrupt pain) — https://github.com/pipecat-ai/pipecat/issues/2791
- Daily.co voice AI June 2025 — https://www.daily.co/blog/advice-on-building-voice-ai-in-june-2025/
- WebRTC.ventures SLM+LLM parallel — https://webrtc.ventures/2025/06/reducing-voice-agent-latency-with-parallel-slms-and-llms/
- Vapi orchestration models — https://docs.vapi.ai/how-vapi-works
- AssemblyAI universal-streaming turn detection — https://www.assemblyai.com/blog/turn-detection-endpointing-voice-agent
- Telnyx core latency budgets — https://telnyx.com/resources/voice-ai-delay-causes
- Retell AI 2025 latency benchmark — https://www.retellai.com/resources/ai-voice-agent-latency-face-off-2025
- ElevenLabs conversation flow — https://elevenlabs.io/docs/eleven-agents/customization/conversation-flow
- Stanford HAI turn-taking research — https://hai.stanford.edu/news/it-my-turn-yet-teaching-voice-assistant-when-speak
- sherpa-onnx — https://github.com/k2-fsa/sherpa-onnx
- sherpa-rs — https://docs.rs/sherpa-rs/
- Moonshine v2 paper — https://arxiv.org/abs/2602.12241
- TEN VAD — https://github.com/ten-framework/ten-vad
- alta/swift-opus — https://github.com/alta/swift-opus
- openWakeWord — https://github.com/dscripka/openWakeWord
- atlas-voice-training — https://github.com/briankelley/atlas-voice-training
- Kokoro-82M ONNX — https://huggingface.co/onnx-community/Kokoro-82M-v1.0-ONNX
- ort (Rust ONNX Runtime) — https://github.com/pykeio/ort
- tonarino/webrtc-audio-processing — https://github.com/tonarino/webrtc-audio-processing
- Azure Speech pricing — https://azure.microsoft.com/en-us/pricing/details/speech/
- azure-speech crate — https://crates.io/crates/azure-speech
- Deepgram Nova-3 — https://transcriber.talkflowai.com/blog/deepgram-nova-3-review-benchmarks-pricing
- ElevenLabs API — https://elevenlabs.io/text-to-speech-api
