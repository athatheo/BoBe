# Whole-Codebase Audit Fixes

Date: 2026-07-15
Branch: `fix/full-codebase-audit`

## Scope

This document records the fixes made after the full BoBe product review across the Rust daemon, Swift/macOS application, voice pipeline, REST/SSE/WebSocket boundary, security, lifecycle, settings, accessibility, localization, CI, and runtime behavior.

The implementation preserved the existing uncommitted Copilot SDK and physical-voice pivot. No compatibility shims or fake test services were introduced.

## Remote daemon and transport security

- Added explicit Swift daemon endpoint modes:
  - managed local daemon on loopback;
  - remote daemon configured with `BOBE_DAEMON_URL` and `BOBE_DAEMON_TOKEN`.
- Remote endpoints require HTTPS, a non-loopback host, and a bearer token.
- Remote mode no longer launches, restarts, stops, or reconciles the bundled local daemon.
- Added bearer authentication to:
  - normal REST requests;
  - primary SSE events;
  - local-runtime progress SSE;
  - Copilot-login SSE;
  - voice WebSocket upgrades.
- Added Rust bearer-token middleware for remote deployments.
- Added rustls-backed HTTPS serving through `axum-server`.
- Non-loopback startup now fails closed unless API token, certificate, key, and explicit allowed hosts are configured.
- Separated listener bind addresses from allowed client Host values.
- Kept unauthenticated plaintext transport available only for loopback operation.
- Allowed the `Authorization` header through configured CORS handling.

## Application and daemon lifecycle

- Prevented a duplicate BoBe app instance from terminating the active instance's daemon.
- Made daemon shutdown dependent on process ownership rather than merely finding a PID file.
- Added cancellable automatic-restart task ownership and lifecycle generations so stale delayed restarts cannot kill a successful manual restart.
- Remote mode now performs endpoint readiness without taking local process ownership.
- Added explicit settings flush during application termination instead of racing equal-duration sleeps.
- Added bounded Rust server drain with active SSE/WebSocket connections.
- Connected SSE producers to daemon cancellation.
- Preserved bounded background-task, install-job, worker, text-turn, and database shutdown.

## Voice lifecycle and protocol

- Explicit Abort and Reset now bypass the acoustic minimum-word gate.
- Automatic barge-in now carries its transcript evidence atomically in the WebSocket message.
- Barge-in candidates can retry after early evidence rejection.
- Cleared stale partial-transcript evidence at turn boundaries.
- Terminal WebSocket receive/send failures now use canonical teardown while retaining the visible failed state.
- Teardown releases keepalive, socket, VPIO, microphone, local TTS, playback, and session-scoped state.
- Made AVAudioEngine configuration transactional with explicit tap/VPIO ownership and rollback after partial failure.
- Prevented Hello from being sent after audio setup failure.
- Client Supertonic no longer requires the daemon Kokoro model:
  - Rust accepts the socket and reads Hello before deciding whether server TTS is required;
  - server TTS is optional for client-TTS sessions;
  - Swift readiness follows the selected TTS backend;
  - onboarding, settings, and mic setup install/validate the selected backend.
- Froze STT language and TTS backend for each live voice session.
- Added protocol tests for atomic barge-in evidence.

## Agent behavior and live product configuration

- Injected enabled Souls and User Profiles into Copilot behavior context.
- The same behavior context now applies to typed, proactive, and shared voice-chat turns.
- Added coherent runtime application for capture and check-in settings.
- Screen Awareness settings now immediately start or stop runtime capture.
- Check-in schedules rebuild from live settings rather than boot-time copies.
- MCP config saves and resets now update the running WorkerRegistry and recycle affected workers.
- Successful Copilot login now triggers registry reload independently of SSE consumption.
- Added registry lifecycle synchronization around worker creation, submissions, reloads, MCP replacement, and shutdown.

## Concurrency and persistence

- Added a shared turn-admission primitive for typed chat, voice, capture, goal, and check-in work.
- Goal and check-in proactivity can no longer overlap an admitted user turn.
- Replaced SSE producers recheck ownership after waking; events are returned before the obsolete producer exits.
- Made goal patching atomic across read, mutation, and rename.
- Validated goal filename IDs against Markdown front-matter IDs.
- Added versioned collision-resistant MCP Keychain account names.
- Added migration support for existing legacy MCP secret references.
- Serialized MCP secret normalization with config mutation.

## Settings and editor behavior

- Added tri-state Swift PATCH fields: unchanged, concrete value, and explicit JSON null.
- Clearing provider URLs/models/reasoning values now clears them in Rust rather than silently omitting the field.
- Added central Save / Discard / Cancel coordination for:
  - Soul and User Profile row selection;
  - Goal selection;
  - Memory reload;
  - settings category changes;
  - settings window close and root replacement.
- Guided Markdown editing now preserves inline Markdown.
- Guided Markdown parsing now ignores heading-like lines inside fenced code blocks.
- Added concrete daemon-unreachable messaging during voice setup.
- Propagated daemon voice-install failure details through the HTTP DTO and surfaced them in onboarding and Voice settings.
- Preserved backend-aware voice installation and cancellation behavior.
- Added the shared observable environment objects at the Settings root. This fixed a real SwiftUI crash when opening Behavior settings.

## Accessibility and UX

- Kept chat and microphone satellite controls in the accessibility hierarchy.
- Hidden satellites become visible when keyboard focus reaches them, avoiding invisible Full Keyboard Access targets.
- Added a stable localized accessibility label to the message composer.
- Hid the rotating visual placeholder from accessibility output.
- Added a meaningful accessibility label to each Goal row action.
- Expanded chat follows streaming only while the reader remains near the bottom.
- Preserved the screen-recording denial state after returning from System Settings.
- Added missing STT setup labels to every supported locale.
- Added native translations for the new System Settings and composer accessibility strings in German, Greek, Spanish, French, Japanese, Korean, Brazilian Portuguese, and Simplified Chinese.
- Kept the onboarding and Settings layouts at productive macOS window sizes.

## CI and regression coverage

- Added a dedicated Ubuntu cross-language contract job to GitHub Actions.
- The job runs when Rust, Swift, the contract script, or CI configuration changes.
- Added or expanded tests for:
  - remote server security validation;
  - explicit/automatic voice cancellation contracts;
  - tri-state settings PATCH encoding;
  - guided Markdown inline and fenced-code preservation;
  - localization coverage;
  - unsaved-transition coordination;
  - goal persistence and ID validation;
  - MCP secret account collision avoidance and migration;
  - runtime settings application and turn admission.

## Runtime validation performed

### Repository gate

`just check` passed on the integrated primary tree:

- Rust formatting and Clippy passed.
- Rust test suite passed.
- `cargo deny` completed with advisories/bans/licenses/sources accepted; existing duplicate/yanked transitive warnings remain informational.
- `cargo machete` found no unused dependencies.
- SwiftLint passed.
- Swift debug build passed.
- 37 Swift tests in 10 suites passed.
- Swift runtime artifacts check passed.
- Rust/Swift cross-language constants and voice protocol drift check passed.

### Release bundle

- `just build` passed.
- Release Rust daemon and Swift frontend built successfully.
- `build/BoBe.app` was produced with bundled backend and resources.

### Backend/API exercises

Using temporary `BOBE_DATA_DIR` directories and alternate ports:

- An insecure `0.0.0.0` bind exited with an explicit security error.
- A daemon with an active `/events` client handled SIGTERM and completed shutdown in 6.33 seconds, including the configured five-second connection drain.
- Sending `provider_base_url: null` changed the value from a configured URL to JSON null.
- PATCHing `capture_enabled` changed `/status.capturing` from false to true and back to false immediately.

### Peekaboo macOS validation

Validated with Peekaboo 3.4.1 using granted Screen Recording, Accessibility, and Event Synthesizing permissions:

- Built app launched under bundle ID `com.bobe.app`.
- Initial onboarding rendered with a populated AX tree and clear hierarchy.
- Engine choice rendered both cloud and local options with correct disabled Continue behavior.
- Settings overview rendered at the expected productive size with accessible navigation and search.
- Opening Behavior initially exposed a real missing-environment crash.
- After injecting the shared settings environments, the same Behavior flow passed with 306 AX elements, visible privacy state, toggles, schedules, inputs, and permission action.
- Quitting the app also stopped its owned bundled daemon.
- Generated Peekaboo screenshots and AX dumps were removed after validation.

## Post-fix reviews

Fresh security, lifecycle, and UX/accessibility passes were run after implementation.

The surviving findings were fixed:

- missing auth on custom SSE and voice WebSocket paths;
- Soul/Profile row selection bypassing unsaved-change coordination;
- unlabeled Goal row action;
- invisible keyboard focus on hover satellites;
- fenced-code Markdown corruption;
- untranslated permission/composer accessibility strings;
- missing settings environment objects causing a runtime crash.

The lifecycle review agent timed out before returning a final summary, but the same paths were covered by the initial audit, implementation review, complete test gate, active-SSE SIGTERM exercise, duplicate-instance ownership changes, release app launch, and app-owned-daemon cleanup test.

## Files intentionally not included

- Peekaboo screenshots and JSON snapshots were validation artifacts and were removed.
- No commits or pushes were created.
