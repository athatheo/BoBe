# Post-review plan (2026-05-16)

Output of six parallel Opus 4.7 reviewers run against `main` after the audit-cleanup session. Each finding cited below was spot-checked against the actual code before recording. Workflow for each item: **review (re-verify) → ask the user → fix**.

## Reviewer summary

| # | Reviewer | Findings | Real bugs | Cleanup wins |
|---|----------|---------:|----------:|-------------:|
| 1 | Bugs + correctness | 4 | 1 HIGH + 1 MED (+2 FP-RISK) | |
| 2 | Lifecycle | 10 | 1 CRITICAL + 1 HIGH + 4 MED + 4 LOW | |
| 3 | Dead code | 22 (17 confirmed + 5 MAYBE) | 1 mini-bug | 17 |
| 4 | Split-brain | 15 | 1 LIVE BUG | 14 dedup hazards |
| 5 | Readability | 15 | none | 15 |
| 6 | Architecture | 10 + 5 rename proposals | none | 15 |

---

## Phase 1 — Real bugs (priority order)

### B01 — CRITICAL: voice locks up after 1 natural turn  *(release-blocker)*
`BoBeService/src/voice/modes/transcript_in.rs:54-55` sets `s.current_turn = Some(...)`. The spawned task running `run_text_turn` ends at `voice/run_text_turn.rs:163` with `state(Listening)` + `drop(guard)` and has NO access to `&mut session` so cannot clear `current_turn`. The only writers that clear are abort/barge-in/cancel-phrase (`turn_flow.rs:30`) and disconnect (`api/handlers/voice.rs:210`). The next `TranscriptFinal` is dropped at `transcript_in.rs:35` with `voice.transcript_final_during_active_turn_dropped`. Voice mode is unusable for multi-turn unless the client tears down + reconnects between turns.

**Fix sketch**: pass `Arc<Mutex<Option<VoiceSession>>>` (or a oneshot completion channel back to `control.rs`'s recv loop) into the spawned task so natural completion clears the slot. Alternatively, check `current_turn.as_ref().map(|t| t.join.is_finished()) == Some(true)` in the `transcript_in.rs:35` gate.

### B02 — HIGH: second `Hello` on same WS orphans the prior turn
`BoBeService/src/voice/control.rs:59` does `*session = Some(new)` unconditionally. `JoinHandle::drop` does NOT abort the underlying Tokio task — the orphan keeps emitting TTS frames over the WS, holds the Kokoro mutex, and is unreachable for barge-in.

**Fix**: before reassigning, if `session.is_some()` call `abort_active_turn(&mut old, ctx, 0, "rehello").await`.

### B03 — HIGH: `/status` returns wrong-case indicator strings
`BoBeService/src/runtime/session.rs:303` serializes `current_indicator().as_str()` → returns TitleCase (`"Idle"`/`"ScreenCapture"`/`"Thinking"`/`"Streaming"`). Bypasses the `#[serde(rename_all = "SCREAMING_SNAKE_CASE")]` attr at `util/sse/types.rs:16`. Swift `Models/AppState.swift:18-25` accepts only `SCREAMING_SNAKE_CASE`; anything else → `.unknown`. After every reconnect, `BobeStoreCapture.swift:82-85` (`synchronizeStatus`) snaps `ctx.thinking`/`speaking`/`captureInProgress` to false even when the daemon is mid-turn.

**Fix**: replace line 303 with `serde_json::to_value(self.event_queue.current_indicator())?` (or match-format to SCREAMING_SNAKE_CASE).

### B04 — MED: voice-turn child tasks not on `TurnInFlight`
`BoBeService/src/voice/run_text_turn.rs:113-119` spawns `kokoro_task` + `filler_task` independently. `TurnInFlight` only tracks the parent. On barge-in/disconnect/cancel-phrase, the parent aborts but the children continue until their current `spawn_blocking(synthesize)` returns (1-3s), holding `LocalKokoroTts::tts: Mutex<OfflineTts>` the entire time. Next WS connect's first synth blocks behind the orphan → audible TTS dead-time.

**Fix**: store all spawned handles on `TurnInFlight` (use a `JoinSet` or `AbortOnDropHandle`) so abort cascades.

### B05 — MED: `try_begin_user_message` Err leaves `current_turn` pinned
`transcript_in.rs:54-55` sets `current_turn` BEFORE the spawned task calls `try_begin_user_message`. If the guard returns `Err` (concurrent capture indicator etc.), the task exits early at `run_text_turn.rs:72-79` but `current_turn` is still `Some` pointing at the dead handle. Same lockup as B01 via a different race.

**Fix**: fold into B01's clear-on-completion plumbing.

### B06 — MED: graceful shutdown wipes ALL chat session IDs
`BoBeService/src/main.rs:201` calls `shutdown_all()` → `reload()` (`registry.rs:241-243`). `reload()` at `:215-226` does `session_store.forget(class, now)` for all 5 classes — appropriate for engine swaps, NOT graceful shutdown. Every clean `ctrl_c` resets chat history; only SIGKILL preserves it.

**Fix**: split `shutdown_all()` from `reload()` — preserve session IDs on clean stop; only the engine-change listener should call the variant that forgets.

### B07 — MED: settings hot-swap aborts mid-voice-turn stream
`BoBeService/src/bootstrap/mod.rs:85-95` spawns `registry.reload()` detached on engine config change. `registry.rs:210` does `self.chat.lock().await.take()` + `client.stop()` even while a voice turn is awaiting `stream.next()` in `run_text_turn.rs:131-133`. RAII guards drop but no response reaches the user.

**Fix**: gate the reload on `voice_turn_active == false` and `user_message_in_flight == false`, or queue it until the active turn ends.

### B08 — MED: SSE writer task never joined on shutdown
`BoBeService/src/api/handlers/events.rs:25-58` spawns per-connection writer with 1s `queue.pop` polling. `main.rs:163-185` (`drain_background_tasks`) doesn't track it. `SseConnectionManager.disconnect` callback (`bootstrap/wiring.rs:42-48`) never fires → `RuntimeSession::on_disconnection` doesn't run → capture won't stop cleanly.

**Fix**: track the JoinHandle (e.g. `Arc<Mutex<Vec<JoinHandle<()>>>>` in `SseConnectionManager`) and drain on shutdown.

### B09 — MED: text-turn HTTP handler spawns untracked Tokio task
`BoBeService/src/api/handlers/conversation.rs:37-40` `tokio::spawn` with no `JoinHandle` tracked anywhere. On `run_graceful_shutdown` (`main.rs:187`) the DB pool closes before the task finishes → orphaned sqlx panic + lost assistant turn.

**Fix**: add a `BackgroundHandles`-style tracker for text handler spawns; drain on shutdown.

### B10 — MED: `last_effort` written before `set_model` awaited *(FP-RISK)*
`BoBeService/src/copilot/workers/chat.rs:109-129` writes `*slot = Some(want_effort)` BEFORE awaiting `session.set_model(...)`. If `set_model` returns `Err`, the only action is a warn and `last_effort` is now stale. A subsequent same-effort send skips the RPC, so a transient failure permanently strands chat on the previous reasoning effort.

**Fix**: only update `slot` inside the `Ok(_)` arm of `set_model`.

### B11 — MED: `CTR_CANCEL_PHRASE` declared but never incremented
`BoBeService/src/voice/telemetry.rs:32` describes `voice_cancel_phrase_match_total`; cancel-phrase regex match at `voice/control.rs:145` exists but never calls `metrics::counter!(CTR_CANCEL_PHRASE).increment(1)`.

**Fix**: add the increment at the match site (or drop the metric — see DC07).

### B12 — LOW: capture trigger doesn't acquire `UserMessageGuard`
`BoBeService/src/runtime/triggers/capture_trigger.rs:110-128` pushes `Thinking`/`Idle` indicators and calls `ProactiveGenerator::generate_proactive_response` without acquiring the guard at `runtime/session.rs:276`. A capture-driven proactive engagement overlapping a voice turn toggles the SSE indicator under the voice turn's `IndicatorGuard` → daemon vs client store disagree.

**Fix**: have capture trigger acquire the guard; if held, skip the proactive (cooldown will retry).

### B13 — LOW: Qwen3 `eouTimer` can fire after `disconnect()`
`BoBeMacUI/BoBe/Voice/VoicePipeline.swift:431-433` fires `Task { try? await engine.reset() }` detached/unawaited. `FluidAudioQwen3Stt.swift:187-194` is the reset body that cancels `eouTimer`. `disconnect` returns before reset runs; a pending `speechEnd`-scheduled timer can fire on the reset engine and try to push `transcript_final` after `pendingTurnId` is nil.

**Fix**: `await engine.reset()` from `disconnect` (block teardown until the timer is gone).

### B14 — LOW: `SystemPowerObserver.start()` never paired with `.stop()`
`BoBeMacUI/BoBe/App/BoBeApp.swift:80` starts it; no caller invokes `stop()` defined at `SystemPowerObserver.swift:55`. Will-sleep notifications during the 20s terminate window can race `BackendService.stop()` SIGTERM.

**Fix**: call `SystemPowerObserver.shared.stop()` in `applicationShouldTerminate`.

### B15 — LOW: `VoicePipeline.init` NotificationCenter observers never deregistered *(benign for singleton)*
`BoBeMacUI/BoBe/Voice/VoicePipeline.swift:217-227` calls `addObserver(forName:...)` without retaining the returned `NSObjectProtocol`. Singleton lifetime makes this benign in practice; bad pattern if singleton were ever replaced.

**Fix**: store observer tokens in an instance array and `removeObserver` in deinit.

### B16 — LOW *(FP-RISK)*: `AVAudioPCMBuffer` captured across async hop
`BoBeMacUI/BoBe/Voice/VoicePipeline.swift:596-606` `Task { [buffer] in await engine.acceptAudio(buffer) }` captures the tap buffer across the main-actor hop. Apple docs say tap buffer storage may be reused after the block returns. Latent under heavy CPU load; Parakeet engine has internal queue so currently observed clean.

**Fix**: copy buffer (`AVAudioPCMBuffer(pcmFormat:..., frameCapacity:...)` + memcpy channel data) inside `handleInputBuffer` before the Task hop.

---

## Phase 2 — Dead code purge

### DC01 — `Cargo.toml:168-169`: drop `tract-onnx = "0.21"` + `mel_spec = "0.3"` (smart-turn deps, ripped in M6.B)
### DC02 — `BoBeService/tests/`: remove empty directory (`voice_corpora.rs` deleted in `bbbf127`)
### DC03 — `ollama_manager.rs:55`: drop stale doc comment referencing deleted `stop()` method
### DC04 — `CLAUDE.md` + `CONTRIBUTING.md`: fix references to non-existent `runtime/prompts/` directory
### DC05 — `voice/telemetry.rs:21`: drop `HIST_STT_MS` (STT moved to Swift)
### DC06 — `voice/telemetry.rs:22`: drop `HIST_SMART_TURN_MS` (smart-turn ripped)
### DC07 — `voice/telemetry.rs:27`: drop `CTR_TURN_ERROR` (never incremented)
### DC08 — `voice/telemetry.rs:31`: drop `CTR_SEGMENT_DROP` (logic now client-side)
### DC09 — `util/sse/factories.rs:9-28`: collapse `indicator_event_with_progress` (all callers pass `None`/`None` for `progress`+`message`)
### DC10 — `SharedControls.swift:146-186`: delete `DebouncedDecimalInput` view (zero callers; `DebouncedNumberInput` is the live one)
### DC11 — `SharedControls.swift:287-294`: delete `formatBytes(_:)` (no callers) + 3 L10n keys × 9 locales = 27 entries
### DC12 — `Models/Validations.swift:39`: delete `validateGoalPriority(_:)` + `goalPriorityRange` static + 1 L10n key × 9 locales (GoalsEditor uses its own `priorityRange = 0...5`)
### DC13 — `voice/filler_library.rs:27/29/37`: decide on `FillerKind::ListenResume`/`LookupBridge`/`ErrorReconnecting` — drop or wire (currently `#[allow(dead_code)]` awaiting paused C-tasks)
### DC14 — `scripts/fetch-test-corpora.sh`: orphaned without `voice_corpora.rs`; decide drop vs keep for future fixtures

---

## Phase 3 — Split-brain dedup round 2

### SB01 — TTS sample rate `24_000`: Rust `voice/session.rs:12`, Swift `VoicePipeline.swift:109+385`. Move to constants pair, add to drift script.
### SB02 — Pause-sensitivity ms (`600/1280/2000`): Swift hardcodes; Rust enum strings only. Ship integer over `/settings` instead.
### SB03 — Kokoro model dir name `kokoro-multi-lang-v1_0`: 4 sites. Centralize.
### SB04 — Voice model kind label `"tts"`: Rust `install_artifacts.rs:23`, Swift filter `VoicePanel.swift:195+245`. Type as enum, dedup.
### SB05 — Voice persona default `"af_bella"`: Rust `config/voice.rs:54` + filler library, Swift `VoicePanel.swift:142`. Constants pair.
### SB06 — 53 Kokoro voice names: Rust `kokoro_tts.rs:138-203`, Swift `VoiceSettingsEnums.swift:12-28`. Daemon should expose `GET /voice/voices`.
### SB07 — Voice install status strings (`idle/running/complete/canceled/failed`): hardcoded both sides. Type as Codable enum on Swift mirroring Rust serde-rename.
### SB08 — Local-runtime install status strings: same shape as SB07.
### SB09 — Tool-call `status` literals `"start"`/`"complete"`: loose strings both sides. `Codable` enum.
### SB10 — Voice error codes (`engines_unavailable`/`voice_disabled`/`voice_busy`): currently low risk; Swift only references in comment. Type as enum if Swift starts surfacing user-friendly text.
### SB11 — Voice WS message `type` literals (12 values): manual encode/decode in Swift `VoiceProtocol.swift:47-77/119-149`. Type as Codable enum AND extend drift script to grep each pair.
### SB12 — VoicePhase wire values (`idle/listening/thinking/speaking`): Rust `protocol.rs:26-33`, Swift `VoiceProtocol.swift:18-23`. Group with SB11.
### SB13 — TTS frame binary header (9B: 8B chunk_id BE + 1B flags): Rust `protocol.rs:104-114`, Swift `VoiceProtocol.swift:165-186`. Ship a single fixture file from daemon + decode test on Swift side.
### SB14 — `~/.bobe` data dir + CORS dev origin: Rust `paths.rs:13-20` + `config/server.rs:23`, Swift `BackendService.swift:38`. Centralize.

---

## Phase 4 — Readability + complexity

### R01 — `copilot/registry.rs:117-159`: extract generic helper for 4 duplicated lazy-init-on-mutex methods (`goals`/`consolidate`/`decide`/`vision`)
### R02 — `runtime/session.rs:118-210`: extract `run_trigger(name, timeout, fut)` for 3 near-identical `timeout + match` blocks
### R03 — `Services/DaemonClient.swift:159-231`: merge `fetch<T>` + `fetchVoid` (70 LOC duplication)
### R04 — `Features/Settings/{Voice,Engine,Behavior,Advanced}Panel.swift`: extract `SettingsDebouncer` (4 copies × 30 LOC = 120 LOC)
### R05 — `VoicePanel.swift:265+288`: parameterize `parakeetStatus` + `qwen3Status` over presence namespace
### R06 — `runtime/triggers/capture_trigger.rs:163-205`: extract `announce_vision_recovered()` (vision-restored reset block duplicated)
### R07 — `runtime/{message_handler.rs:177-181, proactive_generator.rs:198-202}`: `StreamResult::chunks_per_sec()` + `stream_complete_log()`
### R08 — `services/goals/goal_md.rs:102-122`: chain section iterators, walk once
### R09 — `runtime/triggers/checkin_scheduler.rs:69-100, 134-150`: single `refresh_schedules(now)` at the top of both methods
### R10 — `api/handlers/voice.rs:62-250`: extract `acquire_permit` / `spawn_writer` / `spawn_keepalive` / `teardown_session` from 188-LOC `handle_socket`
### R11 — `Voice/VoicePipeline.swift`: strip ~30 WHAT-comments ("VPIO delivers multichannel deinterleaved…") per project convention
### R12 — `BackendService.swift:73, 189-206, 400`: name magic numbers (`12`/`0.2`/`1.5`/`5.0`/`30`/`50`)
### R13 — `Views/Overlay/AvatarView.swift:30-291`: name magic pixel values (`116`/`132`/`76`/`38`/`30`/`34`/`410`/`460`/`16`/`20`)
### R14 — `runtime/decision_engine.rs:59, 62, 77, 150`: name truncation constants (`600`/`200`/`150`)
### R15 — `copilot/workers/chat.rs:178/187`: drop dead `WorkerClass` parameter from `build_message_options` (`let _ = class;`)

---

## Phase 5 — Architecture + folder moves

### A01 — Move `speech/{markdown_strip,sentence_buffer}.rs` → `voice/text_prep/` (only consumer is `voice/sentence_pipeline.rs`)
### A02 — Move `Models/AppState.swift` → `Stores/BobeStoreState.swift` (store internal state, not a wire DTO)
### A03 — Split `Features/Settings/` kitchen sink into `Panels/` + `Editors/` + `DesignSystem/`; move `VoiceSettingsEnums.swift` → `Voice/`
### A04 — Extract `Voice/Views/` subfolder for SwiftUI views (`MicButton`/`StopButton`/`VoicePartialCaption`/`VoiceModelCard`/`VoiceModelRow`)
### A05 — Move `services/conversation_service.rs` → `runtime/conversation_service.rs` (consumed only by runtime/triggers, never API handlers)
### A06 — Move `DeleteOutcome` enum from `souls_service` to `services/mod.rs` (or `services/shared.rs`)
### A07 — Flatten single-file directories: `services/souls/`, `services/user_profile/`, `runtime/learners/`, `voice/modes/`
### A08 — Resolve `mcp_config_service.rs` shape (flat free-fns vs struct subdirectory)
### A09 — Merge `config_manager/` into `config/` (`config/{manager,persistence,fields}.rs`)
### A10 — Cluster `ollama_manager.rs` + `binary_manager/` under `services/ollama/`
### A11 — Rename `Models/` → `DTOs/` once A02 lands (wire types only)

---

## Workflow per item

1. **Review** — open the cited file(s), confirm the finding still applies (code may have moved). Maybe spawn a targeted Opus agent to re-verify and propose a fix.
2. **Ask the user** — present the proposed fix; user confirms or redirects.
3. **Fix** — implement, build, lint, test. Commit in a focused series.
4. **Mark task completed**.

## Tracked tasks

All Phase 1 items have individual task entries (`B01`-`B16`). Phase 2-5 items are clustered into category-level tasks; the markdown reference is the source of truth for the contained sub-items.
