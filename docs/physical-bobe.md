# Physical BoBe: Voice, Embodiment, and Room Satellites

> **Vision snapshot:** July 11, 2026  
> **Status:** Single authoritative voice and embodiment document  
> **Working name:** BoBe Rooms

## The idea

BoBe does not have to live only as a transparent macOS overlay. During setup,
the user could choose where BoBe should live:

1. **On this Mac** — the current overlay.
2. **On a physical BoBe** — a small networked screen with microphones,
   speaker, touch controls, and an optional camera.
3. **In both places** — one companion with several bodies.

The physical device is not another AI agent and does not hold model
credentials. It is a trusted room satellite: an expressive face, local audio
front end, wake-word detector, speaker, display, controls, and optional visual
sensor. The Mac remains the brain that owns identity, memory, models, tools,
and policy.

This division is important. An ESP32 can provide excellent deterministic
real-time I/O, wake-word detection, VAD, acoustic echo cancellation, and a
small native interface. It cannot run BoBe's general STT, multimodal model,
Copilot session, memory, or agent loop at acceptable quality.

**Recommendation in one sentence:** prototype the complete experience on an
M5Stack CoreS3, keep all intelligence and credentials on the Mac, stream
processed PCM over an authenticated local connection, and treat a custom
ESP32-P4 design as a later hardware decision rather than a prerequisite.

---

## Current implemented voice architecture

This section describes the code that exists today. Later sections describe the
physical-device direction.

### Ownership boundary

BoBe ships **Mode B**:

- Swift owns microphone capture, VPIO, resampling, RMS, VAD/EOU integration,
  speech recognition, playback, and optional Supertonic-3 TTS.
- Rust owns the conversation, Copilot session, tools, memory, sentence
  extraction, stable Kokoro TTS, Opus output, turn cancellation, persistence,
  and telemetry.
- Swift sends transcripts rather than microphone audio to the daemon.
- Both typed and spoken turns use the same date-keyed chat worker. There is no
  separate voice agent or voice memory.

```text
Mac microphone
    |
AVAudioEngine VPIO -> VoiceInputProcessor -> FluidAudio STT/EOU
    |
transcript_partial / transcript_final
    |
localhost WebSocket /voice/stream
    |
RuntimeSession -> CopilotChatWorker -> sentence extraction
    |
    +--> Kokoro -> Opus -> Swift playback
    |
    +--> tts_text -> Swift Supertonic -> Swift playback
```

The daemon stays bound to `127.0.0.1`. Physical devices must not connect to
this API directly.

### Speech engine matrix

| Language | STT | Endpointing | TTS |
|---|---|---|---|
| English | FluidAudio Parakeet EOU 120M, measured 320 ms chunk tier | Built-in EOU with configurable debounce | Kokoro stable; Supertonic-3 experimental |
| Mandarin | FluidAudio Nemotron 3.5 multilingual, 1120 ms tier | Silero `VadManager` plus silence timer | Kokoro or Supertonic |
| Spanish | Nemotron multilingual | VAD-driven | Kokoro or Supertonic |
| Greek | Nemotron multilingual | VAD-driven | Kokoro or Supertonic |
| Korean | Nemotron multilingual | VAD-driven | Kokoro or Supertonic |
| Japanese | Nemotron multilingual | VAD-driven | Kokoro or Supertonic |

The app currently targets macOS 15. The selected STT language is captured at
voice-session start so a settings change cannot swap engines under live audio.

### Model ownership

| Dependency | Location | Owner |
|---|---|---|
| `bobe-daemon` | App bundle | Build/package |
| Kokoro model | `~/.bobe/models/kokoro-multi-lang-v1_0/` | Rust installer |
| Filler PCM | Memory, synthesized at daemon boot | Rust |
| Parakeet | FluidAudio model cache | Swift/FluidAudio |
| Nemotron + Silero | FluidAudio model cache | Swift/FluidAudio |
| Supertonic-3 | FluidAudio model cache | Swift/FluidAudio, Expert opt-in |
| Microphone permission | macOS TCC | User/macOS |

Kokoro remains the rollback-safe backend. Supertonic is much faster after
warmup on Apple Silicon but requires an explicit first preparation and remains
an experimental user choice.

### Turn lifecycle

1. The user enables the microphone.
2. Swift configures VPIO on input and output, starts its bounded audio path,
   and opens `/voice/stream`.
3. `hello` locks language, voice, speed, playback rate, and TTS backend.
4. FluidAudio emits cumulative partials through a bounded newest-16 callback
   stream.
5. Swift coalesces partials for the server and sends a final transcript at
   EOU.
6. Rust acquires the same `UserMessageGuard` used by typed turns.
7. The Copilot response streams through an asynchronous observer.
8. `SentencePipeline` strips Markdown and returns complete spoken sentences.
9. A bounded 16-slot channel backpressures generation into the selected TTS
   path.
10. The client reports playback start and completion where it owns TTS.
11. Rust returns to listening only after the actual completion contract.

Input and output device changes are debounced and deferred until capture,
generation, and buffered playback are safe to rebuild.

### Backpressure and ordering

Production voice paths are bounded:

- captured input: newest 16 chunks;
- STT callbacks: newest 16 cumulative events;
- Rust sentence transport: 16 complete sentences;
- client-TTS sentence transport: 16 with asynchronous producer backpressure;
- daemon outbound WebSocket queue: 64 messages.

Dropped input chunks are counted. Queue saturation never silently creates an
unbounded allocation. EOU, sentence, synthesis, and playback order are
serialized.

### Barge-in

During playback:

1. Swift VPIO supplies echo-reduced microphone frames.
2. Sustained RMS starts a candidate interruption.
3. Swift flushes the latest coalesced transcript partial.
4. Rust requires at least three words to reject coughs and backchannels.
5. The active Copilot stream and TTS child tasks are aborted.
6. Rust sends `truncate` using the actual played duration.
7. Swift drops unheard queued audio and returns to listening.

The persisted assistant turn must represent what the user actually heard, not
the text that happened to be generated.

### Perceived latency

BoBe measures separate stages rather than one misleading total:

- `voice_transcript_to_first_audio_ms`;
- `voice_turn_total_ms`;
- `voice_llm_ttft_ms`;
- `voice_first_sentence_ms`;
- `voice_tts_synth_ms`.

The filler watchdog triggers after 800 ms if server Kokoro has not produced
audio. Tool-specific cached fillers can acknowledge work without making
another model call.

Measured on the M4 Pro used for development:

- Supertonic warm synthesis: roughly 81 ms for a 4.52-second sentence;
- Kokoro warm synthesis: roughly 704 ms for the same class of sentence;
- Parakeet 320 ms tier preserved the reference phrase and outperformed the
  less accurate 160 ms configuration.

LLM TTFT remains the dominant uncontrolled stage.

### Current wire protocol

Client to daemon:

```jsonc
{
  "type": "hello",
  "session_id": "voice-1234",
  "playback_rate": 24000,
  "voice_id": "af_bella",
  "speed": 1.0,
  "language": "en",
  "tts_backend": "server_kokoro"
}

{ "type": "transcript_partial", "turn_id": "voice_<uuid>", "text": "..." }
{ "type": "transcript_final", "turn_id": "voice_<uuid>", "text": "..." }
{ "type": "barge_in", "ts_ms": 0, "playback_ms_played": 0 }
{ "type": "playback_ack", "chunk_id": 0, "played_ms": 0 }
{ "type": "wake", "phrase": "hey bobe", "score": 0.9, "ts_ms": 0 }
{ "type": "control", "action": "abort" }
{ "type": "tts_playback_started", "turn_id": "...", "synthesis_ms": 81 }
{ "type": "tts_playback_complete", "turn_id": "..." }
```

Daemon to client:

```jsonc
{ "type": "hello_ack", "voice_pack": "af_bella", "playback_rate": 24000 }
{ "type": "state", "phase": "listening", "turn_id": "..." }
{ "type": "transcript_final", "turn_id": "...", "text": "..." }
{ "type": "tts_text", "turn_id": "...", "sequence": 0, "text": "..." }
{ "type": "tts_end", "turn_id": "..." }
{ "type": "truncate", "turn_id": "...", "keep_ms": 0 }
{ "type": "error", "code": "...", "message": "..." }
```

Server Kokoro additionally emits binary frames:

```text
[8-byte big-endian chunk_id][1-byte flags][Opus payload]
```

### Runtime state and readiness

Swift's visible state is:

```text
idle -> connecting -> listening -> capturing -> thinking -> speaking
                                                   |
                                                   +-> cancelling -> listening
```

The daemon is authoritative for the wire phases; Swift adds client-only
transition states.

`VoiceReadiness` folds together:

- user enablement;
- microphone permission;
- daemon Kokoro install state;
- active-language STT presence and load state.

The UI distinguishes preparing, disabled, permission missing, installing,
models missing, failed, and ready. Missing models re-enter setup rather than
creating an unresponsive mic.

### Current code map

Rust:

```text
BoBeService/src/
├── speech/
│   ├── protocol.rs
│   ├── tts.rs
│   └── providers/sherpa/kokoro_tts.rs
└── voice/
    ├── context.rs
    ├── control.rs
    ├── engines.rs
    ├── filler_library.rs
    ├── install_artifacts.rs
    ├── install_service.rs
    ├── opus.rs
    ├── protocol_helpers.rs
    ├── run_text_turn.rs
    ├── sentence_pipeline.rs
    ├── session.rs
    ├── sinks.rs
    ├── telemetry.rs
    ├── transcript_in.rs
    └── turn_flow.rs
```

Swift:

```text
BoBeMacUI/BoBe/Voice/
├── Audio/OpusDecoder.swift
├── ClientTtsEngine.swift
├── ClientTtsPlayback.swift
├── Providers/FluidAudio/
│   ├── FluidAudioStt.swift
│   ├── FluidAudioNemotronStt.swift
│   └── model-presence helpers
├── TtsPlayback.swift
├── VoiceInputProcessor.swift
├── VoicePipeline.swift
├── VoicePipeline+Audio.swift
├── VoicePipeline+AudioDevice.swift
├── VoicePipeline+WebSocket.swift
├── VoiceProtocol.swift
├── VoiceReadiness.swift
├── VoiceSttEngine.swift
└── WakeWord/WakeWordTap.swift
```

### Current constraints physical BoBe must address

- `VoicePipeline.shared` assumes one local microphone/player.
- `voice_ws_active` permits one connected voice WebSocket.
- `VoiceSink` routes filler feedback through one slot.
- the voice protocol transports transcripts, not remote microphone audio;
- proactive output targets the Mac experience;
- camera attachments are supported by the Copilot vision worker but not by a
  voice turn.

The physical architecture must generalize endpoint ownership without
duplicating the agent or exposing the daemon to the LAN.

### Future voice improvements retained

These are candidates, not shipped behavior:

- **Mac wake word:** `WakeWordTap` remains a scaffold. A custom, locally
  licensed "Hey BoBe" model needs false-positive testing before activation.
- **Voice modes:** conversational, dictation/input-only, and focus/PTT.
- **Adaptive filler tiers:** short acknowledgement around 800 ms, truthful
  tool status later, and explicit long-wait acknowledgement for large work.
- **Tool status:** use actual tool-start/tool-complete events; never fabricate
  narration.
- **Hammering protection:** merge a quick addendum, acknowledge repeated input,
  then cancel/restart only when necessary.
- **BTW/cancel classification:** distinguish "also...", "wait/never mind", and
  acknowledgements without another general LLM call.
- **Speech-rate adaptation:** faster short acknowledgements, slower technical
  or numeric content.
- **Endpointing adaptation:** bounded session-level tuning with a manual
  sensitivity escape hatch; no opaque permanent user profiling.

Strict invariant: exactly one audible stream per endpoint. Reply, status,
filler, and proactive audio share one priority queue and never overlap.

---

## Has this been done?

### Yes: the satellite architecture is proven

The central architectural idea is not speculative. Several independent
systems have converged on the same split:

| Precedent | What it proves | What BoBe would do differently |
|---|---|---|
| **Home Assistant Voice Preview Edition** | A shipped ESP32-S3 room device can provide dual-mic capture, local wake, speaker output, physical privacy controls, and delegate expensive STT/TTS/agent work to a stronger host. Home Assistant's developer documentation explicitly calls these devices "Voice Satellites." | BoBe is a persistent personal companion rather than a home-control intent endpoint. It adds memory, goals, proactive behavior, an expressive screen, and optional intentional vision. |
| **ESPHome Voice Assistant** | ESP-class devices already stream one or two microphone channels, run `microWakeWord`, receive TTS, preserve conversation IDs, expose timers, and publish detailed listening/STT/TTS lifecycle events. | BoBe would use a stricter paired-device trust model and route the same personal conversation across Mac and room endpoints. |
| **XiaoZhi ESP32** | The closest technical precedent: an actively maintained ESP32-C3/S3/P4 client with offline ESP-SR wake, displays and emotions, Opus audio, WebSocket or MQTT+UDP transport, streaming ASR→LLM→TTS, OTA, and device/cloud MCP. It supports more than 70 boards. As of July 11, 2026, its public repository has roughly 28,000 stars and 6,300 forks. | XiaoZhi demonstrates feasibility and demand, but its default experience is server/account centric. BoBe's Mac remains the private trust, memory, model, and permission boundary. |
| **Willow** | An open, self-hosted Echo/Google Home alternative used ESP32-S3-BOX hardware as the voice endpoint and a separate inference server for STT, TTS, and LLM work. | BoBe already has the local agent, memory, tools, and macOS speech stack; it needs an endpoint layer rather than another inference platform. |
| **Rhasspy/Wyoming satellites** | Remote 16 kHz microphone/speaker nodes, wake words, VAD, AGC/noise suppression, discovery, TTS events, timers, and server-side pipelines have worked for years on small Linux devices. | BoBe can start on an MCU rather than Linux because Apple Silicon performs the expensive speech work. |
| **Open Home Foundation Linux Voice Assistant** | The maintained Wyoming successor continues the satellite model, now using the ESPHome protocol and supporting continued conversation, announcements, timers, local wake, and peripheral APIs. It explicitly recommends far-field microphone arrays and XMOS DSP hardware for high-quality capture. | It reinforces BoBe's need to treat acoustics as the main hardware risk, not the agent protocol. |
| **Echo Show / Nest Hub class products** | Consumers understand a room device with a face/screen, microphone, speaker, camera, and a remote intelligence layer. Multiple endpoints sharing one account is established behavior. | Their cloud coupling and opaque privacy model are precisely where a local-first BoBe can differentiate. |

XiaoZhi's published protocol is remarkably close to the proposed BoBe
satellite flow: device hello and capabilities, a session ID, JSON control
messages, binary audio, local wake, listen/start/stop, abort, STT text, LLM
emotion, TTS state, and device-side MCP. This means BoBe would not be inventing
whether an ESP32 can do the job. The work is in adapting a proven endpoint
pattern to BoBe's stronger local identity, privacy, and multi-device semantics.

### The migration history is also evidence

The older Wyoming Satellite project is now archived and points users to the
Open Home Foundation's Linux Voice Assistant using the newer ESPHome protocol.
That is not evidence against satellites; it is evidence against freezing a
narrow, bespoke protocol.

BoBe should therefore:

- negotiate protocol and capability versions from the first hello;
- keep audio transport separate from semantic device features;
- support unknown control messages safely;
- distinguish connection lifecycle from conversation lifecycle;
- avoid putting agent policy inside firmware;
- make firmware and hub independently upgradeable;
- maintain protocol conformance fixtures across Rust, Swift, and ESP-IDF.

### What is still meaningfully new

The research did not reveal a mature open product with this exact combination:

- one proactive companion rather than a command assistant;
- one memory, soul, and goal system across Mac and physical devices;
- local Apple Silicon STT/TTS and agent orchestration;
- expressive screen presence that mirrors the same character;
- privacy-first, intentional still-image vision;
- multi-room routing without giving each device a separate agent;
- no mandatory cloud device account.

The components are proven. The integrated product thesis is differentiated.

### Is it a good idea?

**Yes, as an endpoint strategy and prototype. Not yet as a commitment to
manufacture custom hardware.**

Why it is strategically good:

- BoBe's value is companionship and presence; a stable physical place makes
  that value more legible than a window that appears only on a Mac.
- The architecture has strong precedent and unusually direct ESP32 reference
  implementations.
- The required Swift decoupling is beneficial even if hardware is abandoned:
  it enables simulated endpoints, better voice testing, future iPhone/iPad
  endpoints, and clean local-vs-remote audio ownership.
- BoBe already owns the difficult product layer that generic satellites lack:
  personality, memory, goals, tools, proactive decisions, and a polished face.
- A CoreS3 prototype can test the thesis before PCB, enclosure, certification,
  or manufacturing investment.

Why it could still be a bad product:

- A MacBook is not an always-on home server. If it sleeps or leaves the house,
  room devices become unavailable.
- Far-field full-duplex audio is much harder than the networking demo.
  Home Assistant uses an XMOS XU316; current Open Home guidance still
  recommends microphone arrays with dedicated XMOS processing.
- Proactivity that feels welcome on a personal screen may feel intrusive when
  it speaks from a room.
- A camera can destroy trust faster than it adds utility.
- Multi-room wake arbitration, handoff, timers, and routing multiply edge
  cases.
- Hardware can consume the project while the core software product is still
  finding its strongest daily use.

The correct bet is therefore staged:

1. perform the endpoint/speech decoupling;
2. add a simulated satellite;
3. build one CoreS3 push-to-talk prototype;
4. use it daily before adding wake word or camera;
5. prove far-field audio and barge-in;
6. add a second device to test arbitration;
7. consider custom hardware only after the device creates measurable daily
   value.

### Evidence-weighted verdict

| Question | Verdict |
|---|---|
| Can an ESP32 be the screen/mic/speaker endpoint? | **Proven** |
| Can speech and agent compute live on a stronger local host? | **Proven and shipped** |
| Can ESP32 handle wake, AFE, display, Opus/PCM, and state? | **Proven** |
| Can one host serve several room endpoints? | **Proven generally; BoBe routing remains new work** |
| Is high-quality far-field barge-in guaranteed on CoreS3? | **No; must be measured** |
| Is an optional still camera technically feasible? | **Proven; product trust remains unproven** |
| Is custom ESP32-P4 hardware justified now? | **No** |
| Is a one-device prototype justified? | **Yes** |

---

## Market evidence: what to adopt and reject

Public sales and retention data are unavailable, so GitHub interest and owner
threads are directional rather than market-size estimates. The recurring
failure patterns are still technically specific and consistent across
independent projects.

### What users value

- local control and provider choice;
- physical mute and visible listening state;
- timers, reminders, lists, weather, room context, and family routines;
- custom automations and tools;
- several inexpensive endpoints sharing one intelligence;
- open firmware and freedom from vendor lock-in;
- an ambient display they control.

Home Assistant Voice PE established a credible enthusiast anchor at
**$69 / €59** for a screenless satellite with dual microphones, XMOS audio,
speaker, controls, and physical mute. That does not determine BoBe's price,
but it proves that some enthusiasts buy several host-offloaded room endpoints.

### Complaint patterns and BoBe decisions

| Repeated complaint | Evidence | BoBe response |
|---|---|---|
| Accent/background-noise STT errors make the whole assistant look unintelligent | Voice PE owner reports; Willow far-field/wake issues | Audio quality is the first hardware gate. Keep Apple Silicon STT, calibrate noise, and test dedicated DSP if ESP-SR is insufficient. |
| Local assistants are too slow for trivial commands | Voice PE owners retain Alexa for response speed | Timers, stop, mute, volume, and local state must be deterministic fast paths. Measure every latency stage. |
| Assistant hears and responds to its own TTS | [Voice PE #537](https://github.com/esphome/home-assistant-voice-pe/issues/537) | State follows actual playback completion; AEC receives the playback reference; mic reopening is explicit. |
| Visual state ends before audio begins | [Voice PE #514](https://github.com/esphome/home-assistant-voice-pe/issues/514) | Face and screen follow the render/playhead, not server stream completion. |
| Streaming gap silently wedges the device until power cycle | [Voice PE #612](https://github.com/esphome/home-assistant-voice-pe/issues/612) | Every stage has a timeout and terminal reason; one turn can reset without reboot. |
| Second API client steals state or disconnects another client's playback | [Linux Voice Assistant #328](https://github.com/OHF-Voice/linux-voice-assistant/issues/328) | Registry of connections; broadcast public state deliberately; scope teardown to the owning endpoint. |
| Tool replies leak to every connected client | [XiaoZhi #2020](https://github.com/78/xiaozhi-esp32/issues/2020) | Route by endpoint, connection generation, session, turn, and request identity. Never broadcast private tool/vision results. |
| Several rooms all wake and execute | [XiaoZhi #1960](https://github.com/78/xiaozhi-esp32/issues/1960) | Hub arbitration occurs before earcon; only the winner captures and acts. |
| Setup exposes TLS, URLs, media reachability, and YAML | Voice PE setup reports and issues | BLE provisioning, automatic local certificates/discovery, no URL field in normal setup, separate diagnostics. |
| Updates break wake, TTS, lights, music, or builds | Voice PE issue history | Stable/preview channels, canary one device, signed A/B OTA, compatibility gate, automatic rollback. |
| Broad board support creates permanent compatibility churn | XiaoZhi's 70+ board issue matrix | One reference prototype board, then one supported production board. |
| Ambient screen becomes an ad surface | Echo Show owner reports | No ads, sponsored cards, engagement feed, or remote upsell UI. |
| Dedicated AI gadget could have been an app | Rabbit R1 owner/review discussions | Hardware must win through room placement, far-field I/O, physical controls, privacy, and glanceable presence. |

### Architecture patterns to keep

1. Thin endpoint, strong local hub.
2. Local wake and in-RAM pre-roll.
3. Dedicated AFE/DSP path.
4. Capability-negotiated hello.
5. Binary media plus semantic control.
6. Actual playback acknowledgements.
7. Complete per-endpoint routing identity.
8. Room as first-class metadata.
9. Physical privacy controls.
10. Stable/preview firmware channels with rollback.
11. Push-to-talk fallback.
12. Open protocol without endpoint autonomy.

### Architecture patterns to reject

1. One global current socket, player, or sink.
2. Correlation by request ID without source connection.
3. Broadcasting tool, camera, or memory results.
4. Treating text-stream end as playback end.
5. Reopening the microphone before audible completion.
6. Copilot credentials or tool policy on the satellite.
7. Mandatory vendor account or separate hardware subscription.
8. Exposing the loopback daemon API to the LAN.
9. Raw unauthenticated local WebSockets.
10. Dozens of supported boards before one reliable reference design.
11. Community wake words as product defaults without measured false positives.
12. Continuous camera capture or camera-triggered proactivity.
13. Forced fleet updates or unannounced room-UI redesigns.
14. Music-platform, smart-home-parity, or multi-user scope in the first
    prototype.
15. Custom PCB before daily value is demonstrated.

### Hardware justification

The physical BoBe must do something the app cannot:

- remain in a useful room location;
- hear at a distance;
- provide a dedicated speaker and screen;
- offer physical mute, shutter, stop, and volume;
- respond without finding or unlocking another device;
- carry the same relationship and unfinished context between places.

If the prototype is mainly used for generic questions that are easier on the
Mac or phone, the hardware thesis has failed.

### Market-facing position

Good:

> Give the BoBe companion on your Mac a physical place on your desk or in a
> room. It hears locally, keeps credentials and memory on your Mac, and has
> physical privacy controls.

Bad:

- "A revolutionary AI device."
- "Replaces your phone."
- "Replaces Alexa everywhere."
- "An autonomous embodied agent."
- "Always watching so it can help."
- "More capabilities will arrive after launch."

The first customer is an existing BoBe user with an Apple Silicon Mac, a
local-first preference, and tolerance for preview hardware—not the mainstream
smart-speaker buyer.

---

## Product vision

### A companion with places, not copies

Each physical BoBe is a presence endpoint with a room and personality
expression:

- **Desk BoBe** notices that the user is working, shows subtle status, accepts
  questions, and speaks without requiring the overlay to be open.
- **Kitchen BoBe** handles hands-free conversation and can intentionally look
  at an ingredient, label, or object.
- **Bedside BoBe** defaults to a dim, silent, camera-free mode with strong
  quiet hours.
- **Workshop BoBe** uses louder audio, a more patient endpointing profile, and
  optional camera snapshots for objects or errors.

There is still one BoBe:

- one memory;
- one active personality and soul;
- one goal system;
- one conversation history;
- one permission and tool policy;
- one authoritative agent loop.

Room devices add context such as "John is speaking from the kitchen." They do
not create independent personalities or fragmented memories.

### What the screen should feel like

The screen should not be a miniature settings window. Its primary surface is
BoBe's face and state:

| State | Physical expression |
|---|---|
| Idle | Dim eyes, clock or one quiet contextual card |
| Wake detected | Immediate eye contact and a short listening cue |
| Listening | Live presence ring and optional partial caption |
| Thinking | Calm motion; tool activity represented abstractly |
| Speaking | Mouth/eye animation synchronized to actual playback |
| Muted | Unmistakable crossed-microphone state |
| Camera armed | Live privacy indicator and preview |
| Hub unavailable | Sleeping/offline expression with a clear reason |
| Updating | Progress and rollback-safe status |

Touch is secondary but useful:

- tap to talk;
- tap to interrupt;
- swipe to dismiss a card;
- press and hold for volume;
- confirm or reject a camera snapshot;
- open a temporary QR code for pairing or diagnostics.

Do not remotely render SwiftUI onto the device. The Mac sends a small semantic
scene description, and the ESP32 renders it natively with LVGL or an equivalent
embedded UI:

```text
scene = speaking
expression = warm
caption = "The timer is set for 12 minutes."
accent = "#8F7CFF"
volume = 0.62
```

That keeps animation local, resilient, and low bandwidth.

### Camera: intentional sight, never ambient surveillance

The camera is valuable, but it changes the trust model more than any other
component. Version one should support **intentional still images only**:

- "BoBe, look at this."
- "What is this connector?"
- "Can I use this ingredient?"
- "Read this label."
- a tap on a visible camera button followed by preview and confirmation.

The device captures one JPEG, shows the exact frame being sent, lights a
hardware camera indicator, transfers it to the Mac, and discards it after the
turn unless the user explicitly saves it.

Version one should not include:

- continuous room video;
- passive face recognition;
- automatic identity inference;
- silent occupancy recording;
- autonomous photo capture;
- remote camera access from outside the home.

A product device should have a physical shutter or sensor-power switch. The
camera LED should be electrically tied to the sensor power rail so firmware
cannot capture invisibly.

### Proactivity in a room

Physical presence makes proactive behavior more powerful and more dangerous.
The room policy must be explicit:

- **Visual only:** BoBe may glance or show a card but never speak first.
- **Gentle:** BoBe may speak when recent interaction or proximity indicates
  the user is present.
- **Open:** BoBe may initiate according to normal behavior settings.
- **Quiet hours:** per-device schedule and a hard maximum volume.

A proactive event should route to one suitable device, not every device:

1. current active device;
2. most recently used device;
3. room with explicit presence;
4. user's preferred default;
5. Mac overlay as fallback.

Broadcast audio must be an explicit intent such as "tell every room dinner is
ready," not a side effect of owning multiple devices.

### Embodiment: beyond the decision to speak

The long-term decision is not merely "speak or stay silent." BoBe may choose
to:

- remain still;
- change expression;
- display a card;
- play an earcon;
- speak;
- orient eyes, head, or camera toward a sound;
- request an intentional image;
- move a robot body to a named station;
- ask for consent or clarification.

```text
world state + available bodies + user policy + current goal
                              |
                              v
                    Embodiment Planner
                              |
                              v
                 deterministic Action Governor
                              |
                              v
                 capability/body router
                              |
              +---------------+---------------+
              |               |               |
           screen          camera/head      robot base
           speaker         perception       navigation
```

The model proposes intent. It never emits motor commands, camera register
writes, or unconstrained navigation.

#### Capability registry

Each body advertises typed capabilities and current constraints:

```jsonc
{
  "endpoint_id": "kitchen-bobe",
  "room": "kitchen",
  "capabilities": [
    "display.scene",
    "audio.speak",
    "audio.listen",
    "attention.orient",
    "vision.snapshot",
    "mobility.go_to_station",
    "mobility.dock"
  ],
  "state": {
    "battery": 0.82,
    "docked": false,
    "camera_shutter_open": false,
    "quiet_hours": false
  }
}
```

The planner chooses among available capabilities. Missing capability is a
normal state, not an error or reason to invent a fallback.

#### Action risk tiers

| Tier | Examples | Default policy |
|---|---|---|
| 0 — ambient | expression, dimming, user-selected card | May run automatically within room policy |
| 1 — communicative | earcon, speech, orient eyes toward sound | Allowed by proactivity and quiet-hour policy |
| 2 — perception | open/rotate camera, capture image, inspect an object | Explicit user intent or per-action confirmation |
| 3 — bounded movement | go to named station, approach when summoned, dock | Opt-in map, geofence, speed/obstacle checks |
| 4 — physical consequence | free exploration, stairs, following people, manipulation | Forbidden initially |

The same logical action may change tier by context. Speaking a reminder on a
private desk is lower risk than speaking sensitive content in a shared room.

#### Action Governor

The governor is deterministic code, independent of model persuasion. It checks:

- user permission and whether consent is still fresh;
- room privacy and quiet hours;
- endpoint health, battery, and firmware;
- camera shutter and hard indicator state;
- navigation geofence, speed, floor/stair, obstacle, and emergency-stop state;
- action expiry and observation freshness;
- whether another endpoint/turn owns the user interaction;
- cumulative action and proactivity budget.

A denied proposal returns a typed reason to the agent. It does not silently
pretend the action happened.

#### Device-local controllers

High-rate safety loops stay on the body:

- obstacle and cliff detection;
- motor current and thermal limits;
- emergency stop;
- balance and gait;
- camera hard limits;
- microphone/camera power state;
- speaker clipping and volume ceiling.

For a robot dog, BoBe may request `go_to_station("desk")`. The robot's own
navigation stack plans and validates the path. The language model never
produces joint angles, velocity loops, or an arbitrary destination.

#### Active perception

"Look" is a first-class action with provenance:

1. BoBe states why another observation is useful.
2. Policy checks whether the user directly requested it.
3. The device visibly orients and enables its camera indicator.
4. The user sees the exact frame or field of view.
5. One bounded image is analyzed.
6. The observation is timestamped and treated as ephemeral.
7. Long-term memory stores meaning only when appropriate, not the raw frame.

Orientation without capture is distinct from image capture. A stationary
ESP32 can direct its animated gaze using microphone direction-of-arrival
without turning on a camera.

#### One generation, not another decision agent

The earlier separate Decide worker was removed because it duplicated model
work and created split-brain behavior. Embodiment must not reintroduce it.

The existing proactive generation can return a structured engagement proposal
alongside text:

```jsonc
{
  "engage": true,
  "target": "kitchen-bobe",
  "actions": [
    { "kind": "display", "card": "oven-reminder" },
    { "kind": "speak", "text": "You asked me to remind you about the oven." }
  ],
  "expires_in_ms": 30000
}
```

The governor authorizes and routes the proposal. If the proposal needs new
evidence, a bounded perception action can produce one follow-up observation.
Unbounded perceive-plan loops are not allowed.

#### Body selection as differentiation

One BoBe can choose the right body:

- the desk face displays a quiet goal card;
- the kitchen satellite speaks a hands-free timer;
- a camera endpoint inspects an object after explicit request;
- a robot dog moves to a named room when summoned;
- sensitive content stays on the Mac or a private endpoint;
- no body acts when stillness is the better choice.

This is the core embodied product thesis: one intelligence selecting the right
place, modality, perception, and safe action—not several independent agents.

---

## Setup experience

### New onboarding choice

The current setup can introduce a simple question after BoBe's value is clear:

> **Where should BoBe live?**  
> Keep BoBe on this Mac, add a physical BoBe, or use both.

"On this Mac" remains the default. Choosing a physical device starts a pairing
flow without blocking the rest of setup.

### First-device flow

1. **Power on**
   - Device shows BoBe's eyes and "Finish setup on your Mac."
   - A QR code contains a device ID and unique proof-of-possession secret.

2. **Discover**
   - The Mac discovers an unprovisioned device over Bluetooth LE.
   - The user confirms that the shape/code on both screens matches.

3. **Provision Wi-Fi**
   - The Mac sends Wi-Fi credentials using ESP-IDF Unified Provisioning.
   - Use Security 2: SRP6a key derivation plus AES-256-GCM.
   - The user enters the network password; BoBe should not pretend macOS can
     silently retrieve it.

4. **Pair identities**
   - Device joins the LAN and advertises `_bobe-satellite._tcp`.
   - Mac and device perform a one-time authenticated pairing.
   - The Mac issues a per-device certificate; the device pins the BoBe hub CA.

5. **Name and place**
   - "Kitchen BoBe," "Desk BoBe," or a custom name.
   - Choose room behavior, quiet hours, volume ceiling, and camera policy.

6. **Audio calibration**
   - Test speaker.
   - Measure microphone noise floor.
   - Verify echo cancellation while BoBe speaks.
   - Tune wake threshold and endpointing preset.

7. **Privacy confirmation**
   - Demonstrate the physical microphone mute.
   - If a camera exists, demonstrate shutter, indicator, preview, and
     retention policy.

8. **First interaction**
   - Device says a short locally cached welcome.
   - User completes one real turn from the device.
   - Setup reports measured network, STT, model, synthesis, and playback
     readiness rather than a generic success check.

### Additional devices

Settings gains **BoBe Devices**:

- Add another room
- Name and room
- Connection and firmware health
- Microphone/speaker test
- Volume and quiet hours
- Camera policy
- Last seen
- Certificate fingerprint
- Revoke and factory-reset

Adding a second device should take under a minute because models, personality,
and user configuration already live on the Mac.

---

## Hardware direction

### Prototype recommendation: M5Stack CoreS3

The CoreS3 is unusually close to the concept in one purchasable module:

- ESP32-S3, dual-core 240 MHz;
- 16 MB flash and 8 MB PSRAM;
- 2-inch 320x240 capacitive touch display;
- dual-microphone input through an ES7210 codec;
- 1 W speaker through an I2S amplifier;
- 0.3 MP GC0308 camera;
- proximity sensor, IMU, RTC, microSD, USB-C;
- small 54 x 54 mm main unit and a mountable base.

It is sufficient to prove:

- face and state rendering;
- local wake word and VAD;
- 16 kHz audio streaming;
- returned speech playback;
- touch interaction;
- deliberate camera snapshots;
- pairing, OTA, and multi-device coordination.

Its limitations are equally useful to expose early:

- 2.4 GHz Wi-Fi only;
- 640x480-class camera quality;
- no product-grade physical microphone cutoff;
- unknown far-field/AEC quality in BoBe's enclosure;
- limited thermal, acoustic, and speaker headroom.

It is a prototype platform, not the assumed final product.

### Voice-quality reference: Home Assistant Voice Preview Edition

Home Assistant's device is strong evidence for the satellite model. It uses:

- ESP32-S3 with 16 MB flash and 8 MB PSRAM;
- dual microphones;
- a dedicated XMOS XU316 for echo cancellation, stationary-noise removal,
  and automatic gain;
- physical microphone power cutoff;
- speaker, rotary control, LED feedback, and open firmware.

It deliberately offloads full speech processing to a stronger local or cloud
host. BoBe should learn from its audio and privacy design even though BoBe
needs a screen and optional camera.

If the CoreS3 cannot meet far-field and full-duplex quality gates, the next
prototype should prioritize a dedicated audio DSP or a proven voice front end
before improving the display.

### Product candidate: ESP32-P4 plus ESP32-C6

ESP32-P4 is the better eventual platform if BoBe requires richer animation,
higher-resolution vision, or more local media work:

- dual-core RISC-V up to 400 MHz plus an LP core;
- up to 32 MB PSRAM on the official multimedia board;
- MIPI-CSI/DSI, image signal processing, pixel acceleration, and H.264 encode;
- USB 2.0 high speed, Ethernet, I2S, and extensive I/O;
- security features including secure boot, flash encryption, key management,
  and cryptographic acceleration.

ESP32-P4 does **not** integrate Wi-Fi. The official Function EV board pairs it
with an ESP32-C6 companion for 2.4 GHz Wi-Fi 6 and Bluetooth LE. That board
also demonstrates a 7-inch touch display, 2 MP camera, microphone, 3 W speaker
output, and Ethernet.

A real BoBe product would use the same P4+C6 pattern in a much smaller board:

| Component | Recommended direction |
|---|---|
| Compute | ESP32-S3 first; P4+C6 only after measured need |
| Display | 2.0-3.5 inch IPS, 320x240 or 480x480, capacitive touch |
| Microphones | Two digital MEMS mics with known geometry |
| Audio reference | Playback reference channel wired into AFE |
| Speaker | 1-3 W with enclosure tuned for speech |
| Privacy | Hard microphone power switch and hard camera shutter |
| Camera | 2 MP or better, JPEG still capture; no video requirement |
| Presence | Proximity/ambient-light sensor, not a surveillance camera |
| Power | USB-C, always-on; battery is optional, not foundational |
| Recovery | Hardware reset plus signed A/B OTA rollback |

### Why not an ESP32-CAM

An inexpensive ESP32-CAM is the wrong baseline. It lacks the integrated
display, audio codec, microphone geometry, speaker path, PSRAM headroom, and
physical privacy controls needed to evaluate the product. A cheap board would
mostly measure the defects of the board.

---

## What runs where

### Physical satellite

The ESP32 owns deterministic, local behavior:

- microphone capture;
- ESP-SR audio front end;
- acoustic echo cancellation using the speaker reference;
- noise suppression, AGC, VAD, and wake word;
- a 500 ms in-RAM audio pre-roll;
- screen rendering and animation;
- buttons, touch, mute, shutter, volume;
- speaker playback and playback-position acknowledgement;
- JPEG still capture;
- encrypted transport, reconnect, health, and OTA;
- a small cache of earcons and failure prompts.

It does not own:

- general speech recognition;
- language-model inference;
- Copilot authentication;
- tool permissions;
- user memory;
- goals or proactive decisions;
- long-term raw audio or images.

### Mac speech and device hub

For the first implementation, the macOS process is the satellite hub because
it already owns FluidAudio, Core ML/ANE access, Supertonic, audio state, and
the loopback connection to the daemon.

New Swift components:

```text
SatelliteGateway actor
├── NWListener + TLS
├── Bonjour advertisement/discovery
├── paired-device registry
├── bounded per-device transport
└── SatelliteEndpoint actors

VoiceSessionCoordinator actor
├── local Mac microphone endpoint
├── physical satellite endpoints
├── wake arbitration
├── one active conversational turn
├── endpoint-aware playback routing
└── handoff / interruption

SpeechRuntime
├── shared FluidAudio model lifecycle
├── per-turn STT state
├── Supertonic client TTS
└── stage telemetry by device
```

`VoicePipeline.shared` is currently a singleton tied to AVAudioEngine and one
WebSocket. It should become one local endpoint behind
`VoiceSessionCoordinator`, not be copied once per device.

The gateway should use a TLS-protected length-prefixed protocol over
`Network.framework`, not expose the current HTTP service. This preserves the
daemon's `127.0.0.1` boundary and avoids implementing a WebSocket server in
Swift solely for convention.

### Rust daemon

The daemon remains authoritative for:

- conversation lifecycle;
- Copilot sessions and tools;
- persistence and memory;
- goals and proactive generation;
- prompt and permission policy;
- multimodal model calls;
- response text and turn cancellation.

Existing pieces are directly reusable:

- `ChatAttachment::ImageBytes` and `VisionWorker` already accept image bytes;
- `SentencePipeline` already produces bounded TTS sentences;
- client Supertonic already accepts `tts_text`;
- playback acknowledgements and barge-in semantics already exist;
- the runtime already prevents overlapping user turns.

Current constraints that must change:

| Current behavior | Physical-device requirement |
|---|---|
| `/voice/stream` accepts transcripts, not audio | Swift hub converts satellite audio to transcripts |
| `voice_ws_active` permits one connected voice WebSocket | Many devices may stay connected; only active turns remain single-flight |
| `VoiceSink` has one global slot | Route fillers and tool feedback by active endpoint/turn |
| `VoicePipeline.shared` owns one mic and player | Coordinator owns endpoints; local pipeline becomes one endpoint |
| Voice final has text only | Multimodal turn can include one or more intentional image attachments |
| Proactivity targets the Mac UI | Select an eligible device using room policy and presence |

Do not bind the existing daemon API to `0.0.0.0`. A new LAN surface must not
weaken host validation, CORS, or the loopback-only REST/SSE contract.

### Later extraction

If many satellites, headless operation, or process isolation become important,
extract `SpeechRuntime` and `SatelliteGateway` into a signed Swift/XPC service.
That service can stay alive independently of the settings/overlay UI while
retaining Core ML and FluidAudio. This is a later operational improvement, not
an MVP requirement.

---

## BoBe Satellite Protocol

### Transport

Use one mutually authenticated TLS connection per paired device:

- Bonjour advertises `_bobe-hub._tcp`;
- device validates the pinned hub CA and expected hub identity;
- hub validates the device certificate and revocation state;
- one bounded reader and writer task per connection;
- heartbeat and monotonic sequence numbers;
- explicit reconnect/resume, never silent success-shaped fallback.

For LAN version one, use PCM rather than Opus:

- microphone: signed 16-bit mono PCM, 16 kHz;
- playback: signed 16-bit mono PCM, 24 kHz;
- 20 ms frames;
- approximately 32 KB/s upstream and 48 KB/s downstream while active.

That bandwidth is trivial on local Wi-Fi and removes codec complexity and
latency from the microcontroller. Opus can be negotiated later for remote
links or unusually dense deployments.

### Frame shape

Control frames can remain JSON for debuggability; binary frames use a compact
fixed header:

```text
[u8 version]
[u8 kind]
[u16 flags]
[u32 sequence, big endian]
[u64 monotonic_timestamp_us, big endian]
[payload]
```

Representative control messages:

```jsonc
{ "type": "device_hello", "device_id": "...", "firmware": "...",
  "capabilities": ["screen", "touch", "dual_mic", "speaker", "camera"] }

{ "type": "wake_candidate", "wake_id": "...", "snr_db": 18.2,
  "rms_dbfs": -24.0, "pre_roll_ms": 500 }

{ "type": "capture_start", "turn_id": "...", "sample_rate": 16000 }
{ "type": "capture_end", "turn_id": "...", "reason": "vad_end" }

{ "type": "vision_offer", "turn_id": "...", "mime": "image/jpeg",
  "width": 640, "height": 480, "bytes": 42811 }

{ "type": "playback_start", "turn_id": "...", "sample_rate": 24000 }
{ "type": "playback_ack", "turn_id": "...", "played_samples": 28800 }
{ "type": "barge_in", "turn_id": "...", "played_samples": 28800 }

{ "type": "scene", "state": "thinking", "expression": "focused",
  "caption": "Looking at that label..." }
```

### Backpressure and failure

- Audio queues are bounded by time, not arbitrary frame count.
- Start with 500 ms input pre-roll and no more than 500 ms queued network
  audio.
- A sequence gap is observable telemetry.
- If a queue overruns, abort the turn and surface "Network too slow"; do not
  continue with silently corrupted speech.
- Camera transfers have an explicit byte ceiling and one-frame default.
- Control messages have priority over audio so mute, abort, and barge-in are
  never trapped behind media.

---

## Multi-device behavior

### Connection concurrency is not turn concurrency

All paired devices should remain connected and healthy. Only one conversational
turn needs to own the interactive agent initially.

This means replacing the current global connection permit:

```text
Today: one connected voice WebSocket
Target: many connected endpoints -> one active interactive turn
```

The existing global user-message single-flight guard remains useful. Later,
parallel household users could receive separate conversations, but that is a
different identity and product problem and should not be smuggled into the
satellite MVP.

### Wake arbitration

Two room devices may hear the same wake phrase. They should not both answer.

1. Every candidate reports wake confidence, SNR, energy, room, and monotonic
   timestamp.
2. Hub opens a short arbitration window, approximately 120-180 ms.
3. Hub chooses the strongest eligible candidate with a small preference for
   the last-active room.
4. Winner receives `capture_granted`.
5. Losers immediately suppress their listening animation and retain no audio.

The arbitration window is short enough to feel immediate and long enough to
avoid two devices beginning separate sessions.

### Routing

Default response rules:

- answer on the device that captured the user;
- keep screen cards on that device for the turn;
- if the user taps another device, offer explicit handoff;
- never start speaking on a different room device merely because its network
  latency is lower;
- use the Mac overlay when the originating device disconnects;
- require explicit wording for broadcast.

### Shared context, room-scoped facts

Room and device context is ephemeral prompt context:

```text
Input endpoint: Kitchen BoBe
Capabilities: screen, speaker, camera
Local time: 18:42
Camera used this turn: yes
```

Do not automatically write room presence, wake history, or camera metadata
into long-term memory. Memory should record user-relevant meaning, not device
telemetry.

---

## Audio design

### Device-side pipeline

```text
dual mic + playback reference
        |
ESP-SR AFE
  AEC -> noise suppression -> AGC -> VAD -> WakeNet
        |
500 ms RAM pre-roll
        |
16 kHz mono PCM over authenticated LAN
        |
FluidAudio STT on Mac ANE
```

Espressif's current AFE supports:

- up to two-microphone AEC;
- noise suppression;
- blind source separation or best-channel selection;
- VAD and VAD cache;
- AGC;
- WakeNet;
- an explicit playback-reference channel.

The 2026 ESP-SR full-duplex AEC exposes dedicated full-duplex modes and
recommends `AEC_MODE_FD_LOW_COST` as the initial performance/resource balance.
This is a better starting point than trying to infer barge-in from unprocessed
microphone energy while BoBe is speaking.

### Wake word

Run "Hey BoBe" locally:

- WakeNet9/9l on ESP32-S3 or ESP32-P4, or a trained microWakeWord model;
- VAD-gated detection;
- downloadable model version controlled by signed firmware/config;
- adjustable threshold with false-accept and false-reject telemetry;
- push-to-talk always available when wake detection is disabled.

The wake detector receives only a short ring buffer. Before a validated wake,
audio never leaves the device.

### Endpointing

The device VAD controls transport start/stop, but the Mac STT remains
authoritative for linguistic end-of-utterance:

- device VAD starts streaming quickly and suppresses empty network traffic;
- Mac Parakeet/Nemotron produces partials and semantic/linguistic EOU;
- device sends VAD end as a hint, not an irreversible final;
- barge-in uses processed VAD plus partial-word evidence, preserving BoBe's
  current minimum-word protection.

### Latency goals

The physical device should add almost no perceived latency:

| Stage | Product goal |
|---|---|
| Wake to visible listening | under 200 ms |
| Device LAN contribution | under 50 ms P95 |
| Audio frame jitter buffer | 60-120 ms typical |
| End of speech to first audio | under 1.2 s P50, under 2.0 s P95 when model TTFT permits |
| Spoken interruption to stopped playback | under 250 ms |

Model TTFT will still dominate difficult turns. Instrument each stage with
`device_id`, room, Wi-Fi RSSI, queue depth, audio gaps, STT EOU, LLM TTFT,
synthesis, and first-played-audio timestamps.

---

## Security and privacy model

### Trust boundaries

1. The ESP32 is a paired I/O endpoint.
2. The Mac is the trust and model boundary.
3. Copilot credentials and provider keys never leave the Mac.
4. The existing daemon API remains loopback-only.
5. Satellite traffic is local-only by default and separately authenticated.

### Required controls

- ESP-IDF Security 2 provisioning or stronger.
- Unique device identity and proof-of-possession.
- Per-device certificate, revocation, and rotation.
- TLS peer verification; insecure skip-verification builds never ship.
- Secure Boot v2 or the equivalent supported by the selected SoC.
- Flash encryption in production release mode.
- Signed A/B OTA, first-boot self-test, and automatic rollback.
- Encrypted storage for device private keys and Wi-Fi credentials.
- No secrets in logs, crash reports, QR screenshots, or diagnostics.
- Physical microphone power cutoff.
- Physical camera shutter and hard capture indicator.
- Raw audio retained only in RAM and only around an active wake/turn.
- Images deleted after analysis unless explicitly saved.
- Device list and one-click revocation in Settings.

### Mac sleep and availability

A room device is only as available as its hub:

- If the Mac is awake and BoBe is running, interaction is local and immediate.
- If the Mac sleeps, the device shows a clear sleeping-hub state.
- The device may send Wake-on-LAN, but must not pretend the hub is ready until
  the authenticated session returns.
- "Keep BoBe available to room devices" should be an explicit power setting,
  not a hidden assertion that prevents laptop sleep.
- An always-on Mac mini is the natural hub for whole-home use.

The ESP32 can retain wake word, mute, clock, simple timers, and cached failure
prompts while offline. It should not fall back to an unrelated cloud agent.

---

## Decision history and rejected paths

The deleted planning documents described several earlier architectures. Their
durable findings are retained here; branch recovery instructions, speculative
file maps, and completed milestone tables are not.

### Evolution

1. **Initial stack design**
   - Rust daemon accepted microphone Opus.
   - Daemon-side Moonshine, acoustic VAD, semantic turn detection, and Kokoro
     owned the whole speech pipeline.
   - Swift was primarily capture/playback.

2. **Copilot pivot**
   - Voice was mapped onto the same chat session, memory injection, tools,
     persistence, and single-flight guard as text.
   - Native `session.abort()` made barge-in cancellation practical.
   - Sentence observation was inserted into the existing streaming path rather
     than creating a voice worker.

3. **Mode B simplification**
   - Apple Silicon and FluidAudio provided materially better Mac STT.
   - Swift took ownership of VAD/EOU/STT.
   - Microphone audio stopped crossing the daemon boundary.
   - Daemon-side ASR, semantic-turn models, and duplicate ONNX runtime pressure
     were removed.

4. **Multilingual and latency work**
   - Removed the Qwen3 STT path after upstream API changes.
   - Added Nemotron multilingual plus Silero VAD.
   - Kept Parakeet at the measured 320 ms tier because 160 ms produced word
     errors and slower completion on the M4 test.
   - Added off-main ordered resampling, bounded queues, and stage telemetry.

5. **Client TTS**
   - Added Supertonic-3 behind an Expert rollback setting.
   - Kept Kokoro/sherpa for stable daemon TTS.
   - Added playback-start/completion acknowledgements so state follows audible
     output.

6. **Agent-loop simplification**
   - Removed the separate Decide worker.
   - Capture and goals use one generation that may return silence.
   - Scheduler triggers are isolated, proactive work is single-flight, and
     autonomous sessions have credit ceilings.

### Durable patterns retained from the research

- one chat session for typed and spoken turns;
- cached fillers after an 800 ms TTFT gap;
- bounded sentence streaming;
- one audible stream per endpoint;
- client-local fast interruption detection plus server word gate;
- actual playback position for truncation;
- conversation persistence matching what the user heard;
- tool-aware progress only when truthful;
- explicit wake and voice-mode controls;
- stage-specific latency and failure telemetry.

### Rejected or deferred technology

| Candidate | Decision |
|---|---|
| Daemon Moonshine STT for the Mac app | Rejected for current Mac product; FluidAudio/ANE is faster and better integrated |
| Daemon semantic-turn ONNX model | Removed with Mode B; reassess only for a non-Mac/headless host |
| TEN VAD as the main endpoint | Rejected in favor of current FluidAudio/ESP-SR paths and clearer licensing |
| Apple SpeechAnalyzer as primary | Deferred; current FluidAudio stack is implemented and measured |
| Porcupine custom wake word | Rejected for commercial licensing cost |
| Default openWakeWord models | Do not ship without validating model license and false-positive rate |
| Voice cloning | Deferred; not needed for a trustworthy first physical device |
| Cloud STT/TTS fallback on the satellite | Rejected; providers remain a Mac/hub concern |
| Separate `WorkerClass::Voice` | Rejected; would fragment history, tools, memory, and policy |
| Separate LLM decision call | Rejected; creates cost, latency, and split-brain behavior |
| Continuous camera/video | Rejected for privacy and product-focus reasons |
| Phone/SIP and general media platform | Out of scope |
| Multi-user biometric identity | Deferred until privacy and household semantics are designed |

### Patterns not to resurrect

- branch-specific plans as architectural truth;
- duplicated model stacks merely for cross-platform symmetry;
- unbounded queues;
- synchronous callbacks on hot audio paths;
- daemon-authoritative guesses about client playback;
- state machines that do not encode the actual owner of capture and playback;
- future-feature placeholders treated as shipped behavior.

---

## Delivery roadmap

### Phase 0 — protocol and UX simulator

- Add an in-app simulated satellite endpoint.
- Exercise setup, routing, scenes, disconnects, camera consent, and multiple
  devices without hardware.
- Define metrics and protocol traces before firmware exists.

**Exit:** the product behavior is understandable with two simulated rooms.

### Phase 1 — CoreS3 push-to-talk

- ESP-IDF firmware, native display, touch-to-talk.
- TLS transport and manual pairing.
- 16 kHz PCM to Mac, transcript to existing daemon.
- 24 kHz PCM response to device.
- No wake word and no camera in the first vertical slice.

**Exit:** a complete turn is reliable for hours and the satellite adds less
than 50 ms P95 transport overhead.

### Phase 2 — full-duplex voice

- ESP-SR AFE with dual-mic AEC, NS, AGC, VAD.
- Local "Hey BoBe" wake word and pre-roll.
- Barge-in and playback acknowledgements.
- Audio calibration and false-wake telemetry.

**Exit:** far-field speech and interruptions work while BoBe is speaking.

### Phase 3 — intentional vision

- Camera shutter/indicator behavior.
- Preview and confirmation.
- One bounded JPEG attachment per turn.
- Vision result returns to the same device.

**Exit:** camera use is useful, obvious, and impossible to trigger invisibly.

### Phase 4 — multi-room

- Persistent endpoint registry.
- Wake arbitration.
- Per-room quiet hours and routing.
- Proactive eligibility and explicit broadcast.
- Signed OTA fleet updates and rollback.

**Exit:** three devices can remain connected without double wakes, crossed
audio, or split conversation state.

### Phase 5 — custom hardware decision

Only after measured prototypes:

- retain ESP32-S3 if it meets UI, AFE, and still-image needs;
- move to ESP32-P4+C6 if richer vision/UI is demonstrably valuable;
- add dedicated audio DSP if far-field quality, not MCU compute, is the
  limiting factor;
- design enclosure, microphone geometry, speaker chamber, thermal behavior,
  mute switch, shutter, and repairability together.

### Phase 6 — bounded embodiment

Only after stationary perception and multi-room routing are safe:

- simulate body capabilities and the Action Governor;
- add orientation without camera capture;
- integrate a robot simulator or vendor SDK behind typed capabilities;
- permit only named stations, docking, and summoned approach;
- validate geofence, obstacles, stairs, speed, emergency stop, and expiry;
- keep free exploration, following, and manipulation disabled.

**Exit:** a denied or failed movement is explicit, the language model cannot
bypass safety policy, and no actuator depends on raw model output.

---

## Go/no-go measurements

The prototype should earn the right to become hardware:

| Test | Gate |
|---|---|
| Wake at 1 m and 3 m | Measured by room/noise condition |
| False wakes | Fewer than an agreed daily ceiling in a 24-hour media test |
| Barge-in | Stops audible playback without echo-triggered false interruption |
| Speech loss | No clipped first word due to wake or VAD |
| Network recovery | Recovers from AP restart and hub restart without re-pairing |
| Queue integrity | No silent frame loss; overruns become explicit failures |
| Camera privacy | No capture with shutter closed or indicator off |
| OTA failure | Automatic rollback after failed first-boot self-test |
| Multi-device arbitration | One winner for the same wake in adjacent rooms |
| Mac sleep | Honest offline state and predictable recovery |
| Added latency | Less than 50 ms P95 LAN/transport contribution |
| Camera action | No orientation/capture outside policy and hard-indicator state |
| Robot movement | Named-station navigation passes geofence, obstacle, cliff, and emergency-stop tests |

The most likely product blocker is not ESP32 compute. It is room-grade acoustic
quality: microphone placement, speaker echo, enclosure resonance, and reliable
barge-in. Validate those before investing in industrial design.

---

## Decisions to preserve

1. **One BoBe, many endpoints.**
2. **Mac remains the brain and trust boundary.**
3. **Connection concurrency is separate from turn concurrency.**
4. **Local wake and AFE; general STT and TTS stay on Apple Silicon.**
5. **PCM first on LAN; codecs only when evidence requires them.**
6. **Camera is intentional still capture, not ambient vision.**
7. **Physical privacy controls outrank software indicators.**
8. **Keep the existing daemon loopback-only.**
9. **Prototype on available integrated hardware before designing a PCB.**
10. **Measure acoustics and latency before promising a product.**
11. **The model proposes embodiment; deterministic policy authorizes it.**
12. **Stillness is a valid and often preferable action.**

## Things not to do

- Do not run a second agent or independent memory on every device.
- Do not put Copilot credentials on the ESP32.
- Do not expose port 8766 directly to the LAN.
- Do not begin with continuous video.
- Do not make camera presence mandatory.
- Do not use Matter as the media transport; it may be useful for future
  smart-home capability discovery, not low-latency conversational audio.
- Do not add parallel household conversations before identity and privacy are
  deliberately designed.
- Do not treat a laptop that may leave the house as an always-on home hub
  without explaining the availability tradeoff.
- Do not confuse a successful desk demo with far-field room audio quality.
- Do not expose motor velocity, joint, or camera-register control to the LLM.
- Do not permit free robot exploration, following, stairs, or manipulation in
  the initial embodiment.

---

## Research basis

Primary and directly relevant sources reviewed for this vision:

- [FluidAudio](https://github.com/FluidInference/FluidAudio),
  [Parakeet EOU](https://huggingface.co/FluidInference/parakeet-realtime-eou-120m-coreml),
  [sherpa-onnx](https://github.com/k2-fsa/sherpa-onnx), and
  [GitHub Copilot SDK](https://github.com/github/copilot-sdk) — the current
  speech, TTS rollback, and agent-runtime foundations.

- [ESP32-P4 product overview](https://www.espressif.com/en/products/socs/esp32-p4)
  and [ESP32-P4 architecture announcement](https://www.espressif.com/en/news/ESP32-P4)
  — CPU, memory, security, MIPI, media acceleration, and wireless-companion
  architecture.
- [ESP32-P4 Function EV Board](https://docs.espressif.com/projects/esp-dev-kits/en/latest/esp32p4/esp32-p4-function-ev-board/user_guide.html)
  — P4+C6, touch display, MIPI camera, audio, speaker output, and Ethernet.
- [M5Stack CoreS3 documentation](https://docs.m5stack.com/en/core/CoreS3)
  — integrated ESP32-S3 prototype hardware specifications.
- [ESP-SR Audio Front End](https://docs.espressif.com/projects/esp-sr/en/latest/esp32s3/audio_front_end/README.html)
  — AEC, NS, BSS/MISO, VAD, AGC, WakeNet, channel format, and 16 kHz PCM.
- [ESP-SR full-duplex AEC](https://docs.espressif.com/projects/esp-sr/en/latest/esp32s3/acoustic_echo_cancellation/README.html)
  — 2026 full-duplex modes and resource guidance.
- [ESP-SR framework and 2026 updates](https://github.com/espressif/esp-sr)
  and [WakeNet documentation](https://docs.espressif.com/projects/esp-sr/en/latest/esp32s3/wake_word_engine/README.html)
  — supported chips, wake-word generations, custom wake direction, VADNet,
  DOA, and full-duplex work.
- [Home Assistant Voice Preview Edition](https://www.home-assistant.io/voice-pe/)
  — practical local voice satellite, dedicated XMOS audio processing,
  physical mute, and host-offloaded speech.
- [ESPHome Voice Assistant](https://esphome.io/components/voice_assistant/)
  and [microWakeWord](https://esphome.io/components/micro_wake_word/)
  — production prior art for ESP-based audio satellites, local wake word,
  VAD, conversation continuity, and streamed speech.
- [XiaoZhi ESP32](https://github.com/78/xiaozhi-esp32) and its
  [WebSocket protocol](https://github.com/78/xiaozhi-esp32/blob/main/docs/websocket.md)
  — high-adoption ESP32 display/audio assistant, offline wake, streaming
  ASR/LLM/TTS, Opus, lifecycle state, OTA, and MCP.
- [Willow](https://github.com/HeyWillow/willow) — self-hosted
  ESP32-S3-BOX voice endpoint plus separate inference server.
- [Wyoming Satellite](https://github.com/rhasspy/wyoming-satellite) and its
  maintained successor,
  [Linux Voice Assistant](https://github.com/OHF-Voice/linux-voice-assistant)
  — years of remote voice-satellite practice and an instructive protocol
  migration toward continued conversations, timers, and peripheral support.
- [ESP-IDF Unified Provisioning](https://docs.espressif.com/projects/esp-idf/en/latest/esp32/api-reference/provisioning/provisioning.html)
  — BLE/SoftAP provisioning and Security 2.
- [ESP-TLS](https://docs.espressif.com/projects/esp-idf/en/latest/esp32s3/api-reference/protocols/esp_tls.html),
  [Secure Boot v2](https://docs.espressif.com/projects/esp-idf/en/stable/esp32s3/security/secure-boot-v2.html),
  [Flash Encryption](https://docs.espressif.com/projects/esp-idf/en/stable/esp32s3/security/flash-encryption.html),
  and [ESP-IDF OTA](https://docs.espressif.com/projects/esp-idf/en/latest/esp32/api-reference/system/ota.html)
  — transport verification, signed boot, encrypted storage, A/B updates, and
  rollback.
- [Espressif camera driver](https://github.com/espressif/esp32-camera) —
  sensor support, JPEG guidance, PSRAM requirements, and Wi-Fi contention.
- [Voice PE self-trigger](https://github.com/esphome/home-assistant-voice-pe/issues/537),
  [premature visual state](https://github.com/esphome/home-assistant-voice-pe/issues/514),
  and [streaming wedge](https://github.com/esphome/home-assistant-voice-pe/issues/612)
  — concrete playback/state and recovery failures.
- [Linux Voice Assistant multi-client state corruption](https://github.com/OHF-Voice/linux-voice-assistant/issues/328)
  and [multi-device wake behavior](https://github.com/OHF-Voice/linux-voice-assistant/issues/215)
  — endpoint-registry and arbitration lessons.
- [XiaoZhi cross-client MCP exposure](https://github.com/78/xiaozhi-esp32/issues/2020)
  and [wake-arbitration request](https://github.com/78/xiaozhi-esp32/issues/1960)
  — routing identity and multi-room safety.
- [Voice PE owner use cases](https://old.reddit.com/r/homeassistant/comments/1hs24zs/those_of_you_who_bought_at_least_one_ha_voice/),
  [recognition/action failures](https://old.reddit.com/r/homeassistant/comments/1ntez5h/voice_preview_edition_useless_hilarious_both/),
  and [local-vs-cloud tradeoffs](https://old.reddit.com/r/homeassistant/comments/1py4hip/smart_speaker_final_thoughts_decision_after_weeks/)
  — directional user sentiment.
- [Echo Show forced UI/advertising](https://old.reddit.com/r/amazonecho/comments/1qbrvtc/woke_up_to_a_forced_echo_show_upgrade_removed_the/)
  and [Rabbit R1 hardware-value criticism](https://old.reddit.com/r/Rabbitr1/comments/1cgfk5m/rabbit_r1_barely_reviewable/)
  — ambient-screen ownership and hardware-justification lessons.
