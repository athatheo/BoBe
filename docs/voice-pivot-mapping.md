# Voice on Pivot — Codebase Mapping (Phase -1)

> Output of Phase -1 mapping per `docs/voice-production-plan.md` §8. Read by an Explore agent on 2026-05-11 against `feat/copilot-sdk-pivot` HEAD `f6bfaf5`. Source of truth for which file/line each voice integration touches.

## Backend

### 1. Copilot SDK session lifecycle
- Client created lazily: `BoBeService/src/copilot/client.rs:25-38` (`ClientHandle::ensure_started` calls `Client::start(opts)` once, stores in `Mutex<Option<Arc<Client>>>`).
- Sessions are owned per **WorkerClass** (`Goals`, `Vision`, `Chat`, `Consolidate`, `Decide`) inside `WorkerRegistry` at `BoBeService/src/copilot/registry.rs:31-48`. Chat session is **date-keyed** (`DatedChatWorker`); others use a stable `session.id` file per class.
- Creation/resume lives in `registry.rs:286-387` (`create_or_resume`) using `client.create_session` or `client.resume_session`. Session IDs persisted via `SessionStore` (`copilot/session_store.rs`).
- `session.send(...)` and `session.subscribe()` called in `copilot/workers/chat.rs:81-128` (the `stream!` block).
- **Voice angle**: No SDK call dedicated to voice. STT/TTS engines must live outside the SDK lifecycle in a parallel `VoiceSession` actor; existing `ChatWorker::send(ChatPrompt)` is the bottleneck the voice loop pushes transcribed text into.

### 2. Chat-pipeline entry
POST `/message` flow:
1. `api/handlers/conversation.rs:20-45` (`send_message`) — validates non-empty, calls `runtime_session.try_begin_user_message()`, returns `message_id` immediately.
2. Tokio-spawned: `runtime/session.rs:252-256` `RuntimeSession::handle_user_message` → `MessageHandler::handle_message`.
3. `runtime/message_handler.rs:42-55` `handle_message` updates cooldown, then `ensure_active_conversation` (`message_handler.rs:57-69`) → `ConversationService::append_user_turn_or_create_active` (`services/conversation_service.rs:34-68`).
4. `respond_to_message` (`message_handler.rs:71-90`) sets `IndicatorType::Streaming` then `send_via_chat_worker` (`message_handler.rs:92-104`).
5. `WorkerRegistry::chat()` (`registry.rs:214-243`) → `CopilotChatWorker::send(ChatPrompt::text(...))` (`workers/chat.rs:67-131`) → SDK `session.send(opts)` (`workers/chat.rs:96`).
6. `stream_chat_delta_response` (`runtime/response_streamer.rs:73-155`) consumes the `Stream<ChatDelta>` and pushes SSE.
7. `persist_response` (`message_handler.rs:106-152`) writes assistant turn to DB.

**Voice angle**: voice handler calls `runtime_session.handle_user_message(transcript, voice_msg_id)` — same entry point as text chat. Same flow, same memory injection, same SSE, same persistence.

### 3. Event handling
SDK events route through `CopilotChatWorker::send` which subscribes via `session.subscribe()` and runs `event_to_delta` (`workers/chat.rs:161-237`). Mapping:
- `assistant.message_delta` → `ChatDelta::MessageDelta`
- `assistant.message` → `ChatDelta::MessageComplete`
- `tool.execution_start`/`tool.execution_complete` → `ChatDelta::ToolStart`/`ToolComplete`
- `session.idle` → `ChatDelta::Done`
- `session.error` → `ChatDelta::Error`

The architectural analogue of the spike's `response_streamer` has the **same name**: `runtime/response_streamer.rs:73-155`. Pushes SSE via `EventQueue::push` (`util/sse/event_queue.rs:25-33`), drained at `/events` by `api/handlers/events.rs:12-61`.

### 4. Observer/streaming-text pattern — KEY FINDING
**The observer pattern is present in identical form to the spike.** `stream_chat_delta_response<F: FnMut(&str) + Send>` at `runtime/response_streamer.rs:73-82`. Callback fires on every `MessageDelta` (`response_streamer.rs:92`) and on `MessageComplete` when no deltas (`response_streamer.rs:116`). Today two callsites:
- `message_handler.rs:103` passes a no-op `|_| {}` (user-typed chat)
- `proactive_generator.rs:181-183` uses it to incrementally append to a draft turn (`push_proactive_stream_delta`)

**For M4.5.3 sentence-streaming TTS**: the integration is to pass a sentence-buffering closure here — no architectural change required. **Single biggest "ready-to-go" finding for voice.**

### 5. Single-flight guard
**Present.** `RuntimeSession::try_begin_user_message` (`runtime/session.rs:258-281`) + `UserMessageGuard` (`session.rs:33-41`). Atomic `user_message_in_flight: Arc<AtomicBool>` with `compare_exchange`, plus an indicator check (`Idle` required), returns `&'static str` error mapped to `AppError::Conflict` → HTTP 409 (`api/handlers/conversation.rs:30-32`). **Voice acquires the same guard before invoking the chat worker.**

### 6. Conversation persistence
`ConversationService` (`services/conversation_service.rs:19-310`) owns lifecycle. Three patterns coexist:
- **Per-turn finalized** (user-chat): `append_user_turn_or_create_active` (line 34) then `add_turn` after stream completes (`message_handler.rs:117-119`).
- **Streaming-turn draft** (proactive only): `begin_proactive_stream`/`push_proactive_stream_delta`/`finalize_proactive_stream` (lines 70-141), backed by `DashMap<ConversationId, StreamingAssistantTurn>` (line 22). On lifecycle race, drafts are synced lazily via `sync_streaming_assistant_turn_locked` (line 155).
- Repo at `db/conversation_repo.rs`; turn model at `models/conversation.rs`. Auto-close in `RuntimeSession::close_stale_conversation_if_needed` (`runtime/session.rs:213-239`).

**Voice angle**: reuse `append_user_turn_or_create_active` for voice user turn. The streaming-turn draft pattern is the right model if voice STT delivers partial transcripts that need to be appended visibly.

### 7. Memory write path
- HTTP: `GET /memory` / `PUT /memory` at `api/handlers/memories.rs:21-39` → `MemoryFile::read` / `MemoryFile::replace_all` (`copilot/memory_file.rs`).
- Production writers:
  - **Capture learner** (`runtime/learners/capture_learner.rs:62`): `memory_file.append_under("Recent", &entry)` after each vision turn.
  - **Consolidation** (nightly 03:00 local, `copilot/consolidation.rs:36-62`): calls `consolidate` worker, replaces full body via `WriterGuard::replace_all`.
- Atomic-rename single-writer (`memory_file.rs:122-137`); `Mutex<()>` `write_lock` (line 20); `WriterGuard` for long-held locks (lines 24-36).
- `MemoryFile` is auto-injected into every Copilot session via `BobeHooks::on_hook(SessionStart)` (`copilot/hooks.rs:27-48`), so any user turn (including voice) automatically picks up memory state.

### 8. `binary_manager` current shape
`BoBeService/src/binary_manager/{mod.rs,download.rs,extract.rs}`.
- Today hardcoded to Ollama: `OLLAMA_DARWIN_URL` constant (`download.rs:11`), `extract_ollama_archive` filters for `file_name == "ollama"` (`extract.rs:51`), `managed_binary_path` returns `<data_dir>/ollama/bin/ollama` (`mod.rs:176-178`).
- Public API: `BinaryManager::new(data_dir, http)`, `find_managed_ollama`, `ensure_managed_ollama(progress_tx: &watch::Sender<DownloadProgress>)`, `validate_ollama_binary`.
- Pattern is **good** for voice: chunked-streaming download with `watch::Sender<DownloadProgress>`, partial `.part` file + atomic rename (`download.rs:44-100`), path-traversal protection (`extract.rs:38-44`).
- **Decision**: introduce new `model_manager/` module mirroring `binary_manager`'s patterns, parameterized over URL + filename + expected SHA. Don't refactor binary_manager (cleaner separation; Ollama is a binary, voice models are .onnx files — no tar/gzip extraction).

### 9. Bootstrap pattern
`BoBeService/src/bootstrap/mod.rs:14-121`. Ollama install service wired at lines 77-100. Voice model loading follows exactly this pattern: HTTP endpoint `POST /voice/install` wired through a `VoiceInstallService` analogous to `OllamaInstallService` (`services/ollama_install_service.rs:50-100`).

### 10. Tool events on pivot
Wrapped, not direct. SDK events → `ChatDelta::ToolStart`/`ToolComplete` in `workers/chat.rs:186-221` → `stream_chat_delta_response` pushes via `tool_call_start_event`/`tool_call_complete_event` (`util/sse/factories.rs:83-123`) → SSE bundle types `EventType::ToolCallStart`/`ToolCallComplete`. Swift side handles in `BobeStore.handleToolCall` (`BoBeMacUI/BoBe/Stores/BobeStore.swift:546-548`) → `ToolExecutionController`.

**Voice angle (filler intent-keying)**: cleanest hook is extending the observer in `response_streamer.rs` with a second callback like `on_tool_event: FnMut(&ToolEventKind)`, or reading tool name off `ChatDelta::ToolStart` events. Both possible without changing SDK boundary.

### 11. App state shape
`BoBeService/src/app_state.rs:20-36`. Current: `db`, `config: Arc<ArcSwap<Config>>`, `event_queue`, `connection_manager`, `soul_repo`, `user_profile_repo`, `goals_service`, `runtime_session`, `screen_capture`, `config_manager`, `mcp_config_lock`, `mdns_announcer`, `workers`, `memory_file`, `ollama_install`. **Voice fields slot directly here**: `voice_stt: Option<Arc<dyn SttEngine>>`, `voice_tts: Option<Arc<dyn TtsEngine>>`, `voice_session: ArcSwap<Option<VoiceSessionHandle>>`. Wiring at `bootstrap/mod.rs:102-118`.

### 12. Cargo deps
`BoBeService/Cargo.toml`. Key:
- `github-copilot-sdk = { version = "0.1", features = ["embedded-cli"] }` (line 140) — bundles the Copilot CLI binary.
- **No `ollama-rs`** — Ollama hit via raw `reqwest` from `ollama_manager.rs`.
- **No `LlmProvider` abstractions remain** — deleted in the pivot.
- Voice-relevant existing: `reqwest` (line 127, `rustls-tls,stream,json`), `flate2` + `tar` (already present, reusable), `tokio` full features (line 105).
- New deps voice will add: `sherpa-rs`, `ort` (for smart-turn/wake-word), audio framework crates.

## Frontend

### 13. Overlay UI
`BoBeMacUI/BoBe/Views/Overlay/OverlayView.swift:88-98` (`overlayContent` VStack). Chat history at `chatHistorySection` (lines 152-162) renders `ChatStack` (`Views/Overlay/ChatViews.swift:15-100+`). User and BoBe messages share `ChatMessage` model with `sender: .user | .bobe`. Composer is `MessageInput` (`Views/Overlay/MessageInput.swift`) invoked at `OverlayView.swift:188-200`. **Voice indicator slots above or beside `composerSection`.**

### 14. Settings categories
`Features/Settings/SettingsWindow.swift:3-44` (`SettingsCategory` enum). Groups at lines 46-73: `context`, `integrations`, `preferences`, `advanced`. **Engine lives under `INTEGRATIONS`** (line 57), not PREFERENCES. Voice fits either INTEGRATIONS (depends on locally-installed models) or PREFERENCES (user preference). Recommendation: **PREFERENCES** alongside appearance/behavior/privacy — the model deps are a hidden implementation detail. Mirror `EnginePanel.swift` for structure.

### 15. Welcome wizard
`Views/Setup/WelcomeWizard.swift:4-21` defines `WelcomeStep` (`welcome`, `engineChoice`, `cloudAuth`, `localSetup`, `permissions`, `done`); 5 progress dots. Existing `PermissionsStepView` at `WelcomeWizardSteps.swift:166-287` handles screen capture only. **No microphone permission code exists anywhere** (verified). Add either a new step `.microphonePermission` between `.permissions` and `.done`, or extend `PermissionsStepView` to also handle audio.

### 16. Settings round-trip
PATCH `/settings` endpoint. DTOs at `BoBeMacUI/BoBe/Models/SettingsTypes.swift:6-91`.
- `DaemonSettings` (line 6) is full state; `SettingsUpdateRequest` (line 42) all-optional patch; `SettingsUpdateResponse` (line 78) carries `appliedFields`, `restartRequiredFields`, `persistFailed`.
- Existing: `capture_enabled/interval`, `checkin_*`, `conversation_*`, `goal_check_interval_seconds`, `mcp_enabled`, `engine`, `provider_*`.
- Backend: `BoBeService/src/api/handlers/settings.rs` + `config_manager/fields.rs`. Restart-vs-hot-swap classified in `config_manager`.
- **Voice fields to add** (both sides): `voice_enabled`, `voice_stt_model`, `voice_tts_voice`, `voice_wake_word_enabled`, `vad_sensitivity_dbfs`, `vad_silence_ms`, `voice_volume`, `voice_persona` (soul id).

### 17. Overlay history append
`BobeStore.sendMessage` (`Stores/BobeStore.swift:296-333`): appends synthetic user `ChatMessage` (line 298-308), calls `client.sendMessage`, marks sent/failed. BoBe-side text deltas arrive via SSE through `processBundle` → `handleTextDelta` (line 462-487) which buffers in `streamingMessage` and flushes via `flushStreamingToUI` (489-512). End-of-turn (`done: true`) triggers `finalizeStreamingMessage` (514-544).

**Voice equivalent of `appendUserVoiceMessage`**: extend `sendMessage` to accept `source: .typed | .voice`, treat identically, or add `appendVoiceTranscript(_ text:)` performing the same `messages.append` + setting a `transcriptSource` field on `ChatMessage`. The SSE-deltas-to-UI flow is engine-agnostic — voice gets the assistant reply via the existing path.

### 18. Mic permission
**No microphone permission code exists.** Verified via grep. Required:
- `Info.plist` needs `NSMicrophoneUsageDescription` (set via `BoBeMacUI/Package.swift` or project.yml `info.properties`).
- New `PermissionState`-style flow using `AVCaptureDevice.authorizationStatus(for: .audio)` + `AVCaptureDevice.requestAccess(for: .audio)`.
- Mirror `PermissionsStepView` (`WelcomeWizardSteps.swift:166-287`) as the implementation template.

## Gaps + Risks

**Things that DON'T exist on the pivot but voice needs:**
1. No AVFoundation/audio code anywhere — voice is greenfield on both ends.
2. No microphone permission flow.
3. No WebSocket endpoints — `/events` is SSE-only. WS for STT audio framing requires adding `tokio-tungstenite` or `axum::extract::ws` and a new route on `api/router.rs:36-138`.
4. No model-download pipeline for non-Ollama binaries; `binary_manager` is Ollama-hardcoded (`download.rs:11`, `extract.rs:51`).
5. No `VoiceSession`-style actor. Today only `RuntimeSession` (`runtime/session.rs:19`) exists, chat-pipeline-focused.
6. No filler-intent / sentence-boundary text post-processing. `util/text.rs` only has `truncate_str`.

**Things that exist in a surprising shape:**
1. **Observer pattern already there** at `runtime/response_streamer.rs:73-82`. Spike used `FnMut(&str)`, pivot uses `F: FnMut(&str) + Send`. M4.5.3 is a near-drop-in — pass a sentence buffer instead of `|_| {}` at `message_handler.rs:103`.
2. **`UserMessageGuard` exists with right semantics** (`session.rs:33-41` + `258-281`). Voice acquires the same guard before pushing a transcribed turn.
3. **Streaming-turn drafts already exist** (`conversation_service.rs:70-141`) but only used by `proactive_generator`. Voice may want the same shape for partial-STT UX.
4. **Memory auto-injection is automatic** via `BobeHooks::SessionStart` (`hooks.rs:27-48`). Voice-driven turns inherit this for free.
5. **Per-WorkerClass sessions with daily Chat rotation** (`registry.rs:50-53`, `session_store.rs:25-33`). **Voice turns must go through `workers.chat()`** — bypassing creates a separate conversation context invisible to the chat history.
6. SSE `EventQueue` bounded at 100 (`bootstrap/infra.rs:22`). High-frequency voice partial-transcript events risk overflow drops. Bump capacity or run voice events on a separate channel.

**Top 5 architectural decisions:**
1. **Transport**: WebSocket route vs separate Unix socket vs new SSE channel. SSE is one-way to client; STT needs audio uplink → **WS**. Add to `api/router.rs:36-138`.
2. **`VoiceSession` placement**: parallel to `RuntimeSession` or composed. **Parallel** — RuntimeSession is heavy (triggers + capture + chat dispatch); new VoiceSession in AppState keeps concerns clean.
3. **Per-WorkerClass voice extension**: reuse `workers.chat()` or new `WorkerClass::Voice`. **Reuse** — same session id = unified conversation history, memory injection, MCP tools. Voice is a UI mode, not a session class.
4. **Sentence-streaming hook**: observer closure in `message_handler.rs:103` vs new layer. **The closure is already there; pass a real one.**
5. **Voice model lifecycle**: extend `binary_manager` vs new module. **New `model_manager/`**, parameterized on URL + filename + expected SHA, mirroring `binary_manager`'s atomic-rename + watch-progress patterns. Wire identically to `OllamaInstallService` (`bootstrap/mod.rs:77-100`).

## Essential files

Reference for the implementer when each milestone touches each file.

- `BoBeService/src/app_state.rs` — voice fields added here
- `BoBeService/src/bootstrap/mod.rs` — voice engine load + install-service wire
- `BoBeService/src/bootstrap/wiring.rs`
- `BoBeService/src/copilot/client.rs` — SDK lifecycle
- `BoBeService/src/copilot/registry.rs` — workers/sessions
- `BoBeService/src/copilot/workers/chat.rs` — `ChatPrompt::text` entry point
- `BoBeService/src/copilot/types.rs`
- `BoBeService/src/copilot/memory_file.rs` — auto-injected via hooks
- `BoBeService/src/copilot/hooks.rs` — SessionStart auto-attaches memory.md
- `BoBeService/src/runtime/session.rs` — `handle_user_message` + `try_begin_user_message`
- `BoBeService/src/runtime/message_handler.rs` — `respond_to_message`, observer callsite line 103
- `BoBeService/src/runtime/response_streamer.rs` — `stream_chat_delta_response<F: FnMut(&str) + Send>`
- `BoBeService/src/runtime/proactive_generator.rs` — observer precedent at line 181-183
- `BoBeService/src/services/conversation_service.rs` — streaming-turn pattern
- `BoBeService/src/services/ollama_install_service.rs` — installer pattern to mirror
- `BoBeService/src/binary_manager/{mod,download,extract}.rs` — download/atomic-rename patterns to mirror in new `model_manager/`
- `BoBeService/src/api/handlers/conversation.rs` — `send_message` text-chat entry
- `BoBeService/src/api/handlers/events.rs` — `/events` SSE drain
- `BoBeService/src/api/handlers/memories.rs` — memory.md endpoints
- `BoBeService/src/api/router.rs` — add WS `/voice/stream` route here
- `BoBeService/src/util/sse/{event_queue,factories,types}.rs`
- `BoBeService/Cargo.toml` — add `sherpa-rs`, `ort`, audio deps
- `BoBeMacUI/BoBe/Stores/BobeStore.swift` — `sendMessage`, `handleTextDelta`, tool events
- `BoBeMacUI/BoBe/Models/SettingsTypes.swift` — DTO extension
- `BoBeMacUI/BoBe/Features/Settings/SettingsWindow.swift` — Settings category enum
- `BoBeMacUI/BoBe/Features/Settings/EnginePanel.swift` — pattern template for VoicePanel
- `BoBeMacUI/BoBe/Views/Setup/WelcomeWizard.swift` — wizard step enum
- `BoBeMacUI/BoBe/Views/Setup/WelcomeWizardSteps.swift` — PermissionsStepView template
- `BoBeMacUI/BoBe/Views/Overlay/OverlayView.swift` — overlay content
- `BoBeMacUI/BoBe/Views/Overlay/ChatViews.swift` — ChatStack render
- `BoBeMacUI/BoBe/Services/DaemonClient.swift` — HTTP client + new WS client
