# BoBe Implementation Architecture

> **Status:** Descriptive architecture of the current working tree as of July 17, 2026
> **Scope:** Implemented behavior, current dependencies, deployment paths, and known limitations  
> **Future direction:** See [BoBe Platform Architecture](platform-architecture.md)  
> **Specialist specifications:** [Physical BoBe](physical-bobe.md), [Rust Guidelines](RUST_GUIDELINES.md)

## 1. Purpose and authority

This document explains how BoBe is implemented today. It is deliberately descriptive: a capability appears here as implemented only when the current source tree contains it, or when an explicitly identified external repository implements the other side of a tested protocol seam. Planned mobile applications, hosted control-plane services, and production Ubuntu packaging are not described as existing features.

Source code and tests remain authoritative if this document drifts. Related documents retain narrower ownership:

- [Platform Architecture](platform-architecture.md) owns future multi-surface authority and deployment direction.
- [Physical BoBe](physical-bobe.md) owns the detailed Mac-hub voice and embodiment design, including the implemented BodyLink software slice and the unvalidated physical-hardware roadmap. It remains subordinate to the platform direction where they conflict.
- [Rust Guidelines](RUST_GUIDELINES.md) defines normative Rust structure and coding rules.
- [Updating OTA](UpdatingOTA.md) owns release, signing, notarization, and update operations.

## 2. System summary

BoBe is currently a native macOS application backed by a Rust daemon.

- **`bobe-daemon`** is the authority for BoBe-owned application state and agent execution. It owns conversations, goals, memories, profiles, souls, runtime policy, proactive triggers, Copilot sessions, most tools, server-side TTS, persistence, and the HTTP/SSE/WebSocket API.
- **`BoBe.app`** is the native presentation and device-integration layer. It owns the transparent overlay, settings and setup windows, tray/menu integration, microphone capture, client speech recognition and endpointing, playback, local permissions, and local daemon process supervision.
- **BodyLink V1**, disabled by default, gives one enrolled physical endpoint a certificate-bound PTT audio/text path into the same daemon authority. The external WB-12 firmware implements the device side; the app hosts a scoped speech adapter but does not own turns or routing.
- **GitHub Copilot SDK and CLI** provide the agent session runtime. Stable Rust SDK 1.0.7 controls its pinned, separately signed CLI helper through JSON-RPC over standard input/output.
- **SQLite and files under the daemon data root** persist BoBe-owned state. External Copilot session state is not fully contained in that database; BoBe persists worker session identifiers used to reconnect to the external runtime.

```text
┌──────────────────────────────── macOS process ────────────────────────────────┐
│ BoBe.app                                                                     │
│                                                                              │
│ AppKit lifecycle and windows       SwiftUI presentation                      │
│ Overlay • Settings • Setup         Stores • panels • editors                 │
│                                                                              │
│ Voice input and output                                                        │
│ AVAudioEngine/VPIO • FluidAudio STT/VAD/EOU • playback • optional client TTS │
└──────────────────────────────┬────────────────────────────────────────────────┘
                               │
                    REST + SSE + WebSocket
                   loopback by default; HTTPS remote mode
                               │
┌──────────────────────────────▼────────────────────────────────────────────────┐
│ bobe-daemon                                                                  │
│                                                                              │
│ Axum API → handlers → services/runtime → repositories                        │
│                                                                              │
│ RuntimeSession • turn admission • proactive triggers • voice • persistence   │
│ Copilot workers • MCP • tools • goals • memories • profiles • souls          │
└───────────────┬──────────────────────────┬────────────────────────────────────┘
                │                          │
       SQLite + Markdown/files       GitHub Copilot SDK
       config + secret store         JSON-RPC over stdio
                                            │
                                       signed Copilot CLI
                                            │
                                  model providers and MCP servers
```

## 3. Current deployment topologies

### 3.1 Managed-local mode

Managed-local mode is the default product topology.

1. `BoBe.app` starts.
2. `BackendService` locates the bundled `bobe-daemon` executable.
3. The app launches and supervises the child process.
4. The daemon binds to loopback, normally `127.0.0.1:8766`.
5. The Swift client connects to the local REST, SSE, and WebSocket endpoints.
6. App shutdown owns local daemon shutdown.

The app performs PID/lifecycle handling, readiness checks, crash reporting, and recovery appropriate to a bundled child process. This process ownership is macOS-client behavior, not an inherent requirement of the daemon.

### 3.2 Secure remote-client mode

The macOS client also contains an implemented remote mode selected at process start:

- `BOBE_DAEMON_URL` supplies the remote base URL.
- `BOBE_DAEMON_TOKEN` supplies the client bearer token.
- The remote URL must use HTTPS, must not point to loopback, and cannot contain embedded credentials, a query, or a fragment.
- In remote mode, `BackendService` checks server health but does not launch, kill, or supervise a daemon process.
- REST, SSE, and voice WebSocket URLs derive from the same configured endpoint.

The daemon fails closed when either the configured bind address is non-loopback or the allowed-host list is nonempty. That configuration requires:

- a nonempty allowed-host list;
- a nonempty server API token;
- a TLS certificate path;
- a TLS private-key path.

The server-side environment setting is `BOBE_SERVER__API_TOKEN`; it is distinct in name and responsibility from the Swift client's `BOBE_DAEMON_TOKEN`.

A reverse proxy is not automatically detected. Exposing the default loopback listener while forwarding an accepted loopback Host does not activate these checks. An operator must deliberately configure the allowed-host, authentication and TLS boundary rather than assuming the presence of a proxy makes the default listener safe.

Remote transport support is implemented, but the repository does not yet provide a validated production Ubuntu deployment lane, operator runbook, standalone update channel, remote pairing flow, certificate lifecycle, or backup/restore procedure. Remote transport must not be confused with a complete hosted product.

### 3.3 Platform status

| Surface or deployment | Current status |
|---|---|
| macOS 15+ Apple Silicon application bundle | Implemented and validated by the current product build path |
| Bundled Rust daemon on macOS | Implemented and exercised with the app |
| macOS client connected to a configured HTTPS daemon | Implemented transport and lifecycle mode |
| Ubuntu x86_64 daemon | Compatibility target; standalone packaging and end-to-end deployment remain unproven |
| iPhone/iPad application | Not present |
| Android application | Not present |
| ESP32 firmware | External `espBobeToy` firmware implements the tested BodyLink minor-1 seam; all build profiles pass, but the WB-12 has not been flashed |
| Hosted account/control plane | Not present |

Parts of the Rust code are structurally portable, including Tokio, Axum, SQLite, rustls, and the Unix file secret store. Other parts remain Apple-specific. Screen capture currently uses macOS frameworks and commands such as CoreGraphics, `screencapture`, and `osascript`. Ubuntu support therefore requires real platform isolation rather than merely producing an x86_64 binary.

## 4. Repository and build structure

```text
BoBeService/
  Cargo.toml
  src/
    main.rs                 CLI and server lifecycle
    app_state.rs            process-scoped dependency graph
    bootstrap/              composition root
    api/                    router, middleware, handlers
    body/                   scoped BodyLink gateway, routes, media, and adapter IPC
    runtime/                sessions, turns, triggers and learners
    copilot/                SDK client, workers, hooks, tools, memory/session files
    models/                 domain models
    db/                     SQLite repositories and schema
    services/               domain and operational services
    voice/ and speech/      spoken-turn protocol and server TTS
    config/                 startup config, persistence and hot swapping
    mcp/                    MCP configuration and validation
    util/                   SSE, networking, capture, text and other utilities
    secrets.rs              platform secret stores

BoBeMacUI/
  Package.swift             Swift 6, macOS 15 executable package
  BoBe/
    App/                     app delegate, scenes, NSPanel/windows, managers
    DTOs/ and Stores/        wire contracts and UI/domain state
    Services/                daemon process and network clients
    Stores/                  observable application and theme state
    Voice/                   capture, STT/VAD/EOU, playback and voice protocol
    BodyLink/                correlated FluidAudio/Opus speech adapter
    Views/                   overlay and setup UI
    Features/Settings/       settings design system, panels and editors
    Utilities/               localization and bundle helpers
    Resources/               plist, assets and localization
  Tests/BoBeTests/

docs/                       design, security, release and architecture documents
justfile                    canonical developer/build commands
```

The Rust service is one unpublished binary crate rather than a workspace of reusable libraries. Most internal APIs are crate-visible. The Swift side is one executable Swift package, not currently a cross-platform client SDK.

## 5. Rust daemon

### 5.1 Entry point and lifecycle

The daemon uses Clap and exposes `serve` and version behavior. Server startup performs the following broad sequence:

1. Load layered configuration.
2. Validate bind, allowed-host, authentication, and TLS combinations.
3. Initialize tracing and metrics.
4. Construct repositories, services, runtime components, Copilot workers, voice components, and shared state through `bootstrap`.
5. Build the Axum router and middleware layers.
6. Bind with plain HTTP for the permitted loopback-only case or rustls TLS for configured secure exposure.
7. Start background runtime work.
8. Handle SIGINT/SIGTERM and coordinate cancellation-driven shutdown.

The daemon uses Tokio's multi-threaded runtime. `CancellationToken` coordinates shutdown rather than process termination from internal code.

### 5.2 Dependency composition

BoBe uses explicit constructor injection rather than a dependency-injection framework. `bootstrap` is the composition root, and `AppState` holds process-scoped shared dependencies, including:

- configuration management;
- database/repositories;
- conversation and domain services;
- runtime session and turn admission;
- Copilot worker registry/client;
- SSE connection/event infrastructure;
- voice state and services;
- local runtime/install services;
- secret storage;
- metrics and operational state.

This graph assumes one personal BoBe authority per process. It does not contain a tenant registry or per-request owner context.

### 5.3 Layering

The intended flow is:

```text
Axum handler
    │ validate/extract HTTP input
    ▼
service or runtime component
    │ apply domain/runtime behavior
    ▼
repository or external adapter
    │ SQLite/files/Copilot/MCP/system service
    ▼
result mapped to API response or AppError
```

Handlers normally return `Result<T, AppError>`. Production code avoids `unwrap()` and `expect()`. The primary chat instructions live in `copilot/skills/chat.md`; additional runtime and worker instructions currently remain at their call sites, including proactive generation, consolidation and batch workers.

## 6. API and transport

### 6.1 REST API

The Axum router exposes groups for:

- health and status;
- user messages and conversation lifecycle;
- SSE events;
- voice sessions;
- settings and runtime configuration;
- goals, memories, souls, profiles, cooldowns and related entities;
- MCP configuration and secrets;
- engine/Copilot authentication and status;
- local runtime/model installation;
- diagnostics and metrics-related behavior.

`POST /message` accepts typed content plus a client-generated `request_id`.
The request ID deterministically owns the server message identifier and a
local durable job (`queued`, `running`, `completed`, or `failed`). Queued work
is recoverable after restart. Running work is never replayed after a crash
because tools may already have produced side effects; the client instead gets
an explicit unsafe-replay state. Retrying active or completed work returns the
same message identifier rather than executing tools twice. Incremental output
is delivered separately through the main event stream. Routine retention and
privacy purge erase request content but retain content-free UUID tombstones, so
an old network retry cannot reopen tool side effects.

Short-lived HTTP routes receive a 30-second timeout. Long-lived routes, including the main event stream, voice WebSocket, local-runtime status stream, and Copilot login event stream, are deliberately excluded from that timeout.

### 6.2 Main SSE stream

`GET /events` carries incremental daemon events to the macOS application. Event types include response text deltas, indicators, tool activity, errors, heartbeat and conversation closure.

The current main event implementation has an important single-surface constraint:

- there is one current main `/events` connection;
- a new main connection replaces the previous connection;
- producers push into one shared queue;
- the consumer destructively removes queued events.

This is adequate for the current single macOS presentation surface. It is not a broadcast or resumable multi-client protocol. Other streams, such as Copilot login events and local-runtime status, have separate behavior and should not be conflated with the main event connection.

### 6.3 Voice WebSocket

`GET /voice/stream` implements the current `bobe.voice.v1` spoken-turn contract.

The client sends structured messages such as:

- hello/session information;
- partial and final transcripts;
- wake events;
- playback acknowledgements;
- barge-in evidence;
- control messages.

The daemon sends:

- state changes;
- transcript confirmation;
- response/TTS text;
- binary Opus output when server TTS is selected;
- truncation/cancellation information;
- structured errors.

The current protocol intentionally does not accept microphone audio from the client. Swift performs speech recognition and sends transcripts. This makes the existing voice protocol unsuitable for a microphone-only ESP32 without a separate audio-uplink profile.

Only one voice WebSocket is active process-wide. A second connection receives a busy response. The selected TTS path is frozen for the lifetime of a voice session so that a settings change cannot change the backend under active playback.

### 6.4 Physical-body status

The current tree contains an opt-in BodyLink V1 vertical slice:

- a dedicated WSS `/body/v1` listener with CA validation and exact enrolled
  client-certificate pinning;
- daemon-issued body session, connection generation, request, lease, turn, and
  media-stream correlation;
- one expiring physical voice lease participating in the existing shared turn
  admission gate;
- endpoint-only LLM/token/tool delivery, targeted text/face/audio output, and
  playback credit plus played-sample drain;
- a bearer-authenticated loopback `/body/adapter` socket;
- a Swift `BodySpeechAdapter` that performs FluidAudio STT and Opus-to-PCM
  conversion only—it does not open `/voice/stream`, mint turn IDs, or own
  routing;
- a non-content `conversation_changed` SSE signal plus
  `/conversation/current` snapshot so the Mac converges on persisted final
  turns without receiving private body deltas or tool activity.

BodyLink is disabled by default and currently supports one explicitly enrolled,
certificate-pinned WB-12 identity. Mock-adapter and real-Swift-adapter smokes
exercise the full daemon/agent/TTS path. The recovered physical unit has not
yet been flashed because it is not connected; product enrollment/revocation UI,
fleet OTA, multiple enrolled certificates, wake word, and camera remain future
work. See [Platform Architecture](platform-architecture.md) and
[Physical BoBe](physical-bobe.md).

The speech adapter has a persistent adapter-only mode with no visible windows,
so body turns do not depend on overlay, settings, store, or main voice-session
state. It is still packaged in the `BoBe` executable and must be launched and
supervised separately when the main app is absent. The daemon and firmware
alone cannot currently perform FluidAudio STT or Apple Opus decoding.

### 6.5 Middleware and exposure controls

The router uses distinct controls with different scopes:

- **Host validation** rejects unapproved Host headers.
- **Bearer authentication**, when configured, compares tokens in constant time.
- **HTTP CORS** permits an exact configured list of local origins for browser-style requests.
- **Request tracing/logging** records transport activity.
- **Concurrency limiting** caps active requests.
- **Timeouts** apply to short-lived routes while excluding streaming routes.
- **TLS** is provided through rustls for configured secure serving.

CORS is not authentication and does not secure arbitrary native clients. Host validation, bearer credentials, TLS, CORS, and any handler-specific upgrade/origin behavior are separate layers.

## 7. Runtime and conversation model

### 7.1 RuntimeSession

`RuntimeSession` coordinates typed, spoken and proactive work. It owns or connects:

- conversation state;
- response indicators;
- turn admission;
- worker/session selection;
- response streaming;
- cancellation;
- proactive triggers;
- persistence and event emission.

The current domain permits one pending or active conversation. Typed and voice turns enter the same personal conversation and use the same date-keyed chat worker rather than creating separate voice identity or memory.

### 7.2 Shared turn admission

One admission permit is shared across current turn sources:

- typed user messages;
- spoken user turns;
- screen-capture-derived work;
- goal evaluation;
- proactive check-ins.

This prevents two agent turns from mutating the same personal context concurrently. It is broader than a UI busy flag and should be understood as process-wide serialization of authoritative agent work.

### 7.3 Reactive flow

```text
User input
   │ typed REST or final voice transcript
   ▼
turn admission
   ▼
active conversation service
   ▼
Copilot chat worker/session
   │ tool calls and streamed model output
   ▼
response streamer / sentence extraction
   ├── main SSE text and activity events
   └── voice TTS pipeline when spoken
   ▼
persistence and final state
```

### 7.4 Proactive flow

Turn-admitted background triggers can initiate work when policy allows it:

- screen capture and summarization;
- goal evaluation;
- scheduled check-ins.

These triggers use the same personal runtime and compete with typed and spoken user turns for the shared turn-admission permit. The main SSE connection currently also acts as a desktop-presence signal for capture lifecycle, which is another reason it cannot simply be reused as a generic device-presence mechanism.

Nightly consolidation is separate. `ConsolidationScheduler` does not acquire the shared turn-admission permit; it submits independently and protects `memory.md` with compare-before-commit optimistic concurrency.

## 8. Copilot SDK architecture

### 8.1 Process topology

BoBe pins `github-copilot-sdk` and ships the matching Copilot CLI as a
separately signed executable under `Contents/Helpers`. The SDK build script
downloads the pinned asset, verifies its published SHA-256, and extracts it
into project-local staging. App assembly copies that executable into the
bundle, and the Swift launcher passes its absolute path through
`COPILOT_CLI_PATH`. The SDK then:

1. starts the explicitly selected helper process;
2. communicates through JSON-RPC over standard input/output;
3. exposes `Client` and `Session` abstractions to the daemon.

The Cargo `bundled-copilot-cli` feature remains a self-contained fallback for
direct Cargo consumers, but normal debug/release recipes do not embed the
compressed CLI archive in `bobe-daemon`.

Staging records both the `Cargo.lock` hash and the extracted helper hash.
`just build-backend` removes and re-extracts a helper when either hash drifts,
then checks the GitHub Developer ID before use. The SDK server and both login
paths pass `--no-auto-update`; Copilot CLI updates therefore arrive only with a
new BoBe/Sparkle release and cannot rewrite the signed app bundle.

BoBe creates one shared SDK client and separate logical sessions/workers for different responsibilities. Worker session identifiers are persisted so the runtime can recover continuity where supported.

This architecture is a desktop/server process architecture. The signed helper
and login flow are material runtime dependencies, not implementation details
that can be assumed to work inside a mobile application sandbox.

### 8.2 Worker responsibilities

The `copilot/` subsystem contains the principal agent boundary:

- client startup and lifecycle;
- worker registry;
- chat, batch and vision workers;
- hooks and permissions;
- session persistence;
- memory-file integration;
- typed daemon-owned memory and goal tools;
- skills/tool construction;
- MCP integration;
- typed SDK/event adapters.

Workers share BoBe's domain context but use different session roles and lifecycle rules. Provider selection is deliberately narrow: GitHub Copilot cloud or local Ollama.

### 8.3 Tool boundary

Tools execute through BoBe-controlled adapters and Copilot SDK hooks. Tool access includes application data and, when configured, MCP capabilities. The current permission behavior is:

- shell, arbitrary file-write, unknown and absent permission kinds are denied;
- read, URL, memory and hook requests are approved;
- read approval does not currently enforce a daemon-side canonical-path or approved-root check;
- custom tools require the Chat worker and one of the exact `bobe_memory_append`/`bobe_goal_*` names;
- MCP requests require a reported tool name and are approved unless that name is in the configured exclusion set;
- memory appends and goal creation/updates execute through daemon services, validation and atomic storage rather than arbitrary paths;
- MCP command and environment definitions are validated against blocklists and injection constraints;
- secret references are resolved by the daemon rather than exposed through ordinary DTOs.

This is not an interactive approval boundary for every consequential MCP or custom-tool operation. Stronger per-operation policy and filesystem confinement are future hardening needs, especially for hosted execution.

## 9. Persistence and configuration

### 9.1 Data root

File-backed daemon paths derive from `BOBE_DATA_DIR`. The default is under the user's BoBe directory, but documentation and code must not assume every deployment literally uses `~/.bobe`.

When no explicit database URL is configured, SQLite is derived from the resolved data root as `sqlite:<data_dir>/data/bobrust.db`. `BOBE_DATABASE__URL` remains an independent explicit override.

### 9.2 State ownership

| State | Current storage | Authority and notes |
|---|---|---|
| Conversations and messages | SQLite | Daemon-owned application history |
| Message request jobs and replay guards | SQLite | Content-bearing jobs are bounded; content-free request UUID tombstones preserve at-most-once side effects |
| Cooldowns | SQLite | Runtime scheduling state |
| Souls and user profiles | SQLite | Personal companion/domain state |
| Goals | Markdown files under the data root | File-backed, human-readable goal model |
| Long-term memory | `memory.md` and related runtime files | Copilot-readable memory surface |
| Worker session identifiers | Files under the data root | Pointers to external Copilot session continuity, not a complete copy of external state |
| Runtime configuration | `config.toml` plus environment overrides | Persisted and layered configuration |
| MCP configuration | Daemon-managed configuration files/state | References secrets rather than embedding ordinary plaintext DTO values |
| Secrets on macOS | Data Protection Keychain | Platform secret store |
| Secrets on non-macOS Unix | `secrets.json`, mode `0600` | Single-user daemon fallback |
| Models and runtime artifacts | Model/runtime cache directories | Large downloadable or extracted artifacts |
| Logs | Configured log location | Operational data; retention is deployment-dependent |
| Swift presentation settings | UserDefaults and app-local state | Client-owned UI/device preferences |
| Swift speech models | FluidAudio-managed cache | Client-side STT/VAD/experimental TTS artifacts |

There is not yet a documented, validated whole-runtime backup and restore procedure. A complete restore must account for database/files, secret re-provisioning, model caches or redownloads, and external Copilot session behavior.

`DELETE /privacy/data` acquires the shared turn-admission permit, blocking new text, voice and proactive turns for the full operation. It disconnects live workers and deletes every current, historical or hard-reload-retired SDK session referenced by BoBe before removing local IDs. SDK deletion is concurrent and deadline-bounded; on failure a retirement ledger preserves IDs so the purge can be retried instead of falsely reporting success. The purge deletes message content, digests and job status while retaining only content-free request UUID replay guards.

### 9.3 Configuration layering

Daemon configuration layers:

1. compiled defaults;
2. persisted TOML under the data root;
3. `BOBE_*` environment variables.

Configuration is not uniformly hot-reloadable.

- Server bind/port, database URL, mDNS and some logging settings require restart.
- Many behavior and engine settings can be persisted and applied at runtime.
- Hard engine changes may rebuild the SDK client and sessions.
- Soft engine changes can preserve chat continuity.

The settings API and Swift UI mirror this distinction with restart-required state where appropriate.

## 10. Voice architecture

The current product uses Mode B:

- Swift owns microphone capture, voice-processing I/O, resampling, speech recognition, VAD/end-of-utterance logic and playback.
- Swift can optionally own experimental client TTS.
- Rust owns the authoritative conversation and agent work, sentence extraction, cancellation, persistence, telemetry, and the stable server-TTS fallback.
- Swift sends transcripts rather than microphone audio.
- Rust sends text and, when using server TTS, binary Opus audio.

```text
Mac microphone
   │
AVAudioEngine / VPIO
   │
FluidAudio STT + endpointing
   │ partial/final transcripts
   ▼
bobe.voice.v1 WebSocket
   │
RuntimeSession → Copilot worker → sentence extraction
   ├── server TTS (Kokoro/sherpa path) → Opus → Swift playback
   └── TTS text → optional Swift client TTS → Swift playback
```

Voice latency metrics cover model time-to-first-token, first sentence, synthesis, transcript-to-first-audio, total turn time, fillers and barge-in behavior.

For codec framing, queue behavior, endpoint pairing, physical-device transport, acoustic processing, board choices, privacy UX and the embodiment roadmap, see [Physical BoBe](physical-bobe.md).

## 11. Native macOS application

### 11.1 Frameworks and dependencies

The Swift package targets Swift 6 and macOS 15. Its major dependencies are:

- **SwiftUI** for declarative views and observable state;
- **AppKit** for application lifecycle, nonactivating panels, windows, menu/tray integration and system-level behavior;
- **FluidAudio** for Apple Silicon speech recognition, VAD and experimental client TTS;
- **Sparkle** for application updates;
- **Textual** for rich text/Markdown presentation.

### 11.2 Application and window lifecycle

The app combines SwiftUI scenes with explicit AppKit managers:

- `BoBeApp` and the app delegate coordinate startup, activation and termination.
- `OverlayPanel` provides the transparent, nonactivating overlay.
- `OverlayWindowManager` manages overlay visibility and placement.
- `SettingsWindowManager` and `SetupWindowManager` own their respective window lifecycles.
- `BobeMenuBarScene` and tray components expose menu-bar behavior.
- `BackendService` owns managed-local versus remote backend lifecycle.

This split is intentional: SwiftUI owns view composition, while AppKit owns behaviors that require precise `NSWindow`/`NSPanel` control.

### 11.3 State and networking

`BobeStore` is the primary observable application state. `ThemeStore` owns theme state. `DaemonClient` wraps REST calls and the main SSE connection; voice networking is managed by the voice pipeline.

The Swift client manually mirrors daemon DTOs and event contracts. Cross-language constants and voice protocol variants are checked by the local validation lane, but there is no generated OpenAPI or schema-driven client. DTO changes still require coordinated Rust and Swift updates.

### 11.4 Settings and setup

The app provides native configuration for:

- engine and authentication;
- appearance and behavior;
- privacy and capture;
- voice and model installation;
- MCP servers;
- memories, goals, souls and profiles;
- advanced/expert settings.

Onboarding coordinates engine choice, downloads, permissions and voice readiness. Local process installation and model preparation are administrative client responsibilities; they are not suitable assumptions for a headless remote daemon.

## 12. Security boundaries

### 12.1 Network boundary

The safe default is loopback-only serving. Configured remote exposure is fail-closed and requires authenticated TLS plus allowed hosts. The current global bearer token authorizes the daemon surface broadly; it is suitable for a controlled personal remote deployment but not for embedding in an untrusted toy or for fine-grained multi-surface authorization.

### 12.2 Host and browser boundary

Host validation protects against malformed or unexpected Host routing. HTTP CORS limits browser origins. Native clients still require authentication; CORS is not a native-client authorization system.

### 12.3 Secret boundary

Provider and MCP secrets remain daemon-side. The secret-store implementation abstracts macOS Keychain and Unix file storage. Secret values should not be serialized through settings APIs or stored in ordinary configuration.

### 12.4 File and command boundary

Copilot shell and arbitrary file-write permission requests are denied. Read requests are approved without daemon-side path confinement, so the implementation must not yet be described as enforcing approved roots for all reads. Exact allowlisted memory and goal tools are the only agent mutation path. MCP command and environment configuration receives separate blocklist and injection validation. A hosted design requires stronger filesystem, process, credential and per-operation tool isolation than the current personal daemon.

### 12.5 Voice and capture privacy

Swift owns microphone permission and visible device interaction. Screen capture currently observes the daemon host's macOS desktop and uses macOS permission infrastructure. A remote or physical endpoint does not automatically grant the daemon awareness of that endpoint's local environment.

## 13. Build, test and release

Canonical commands are defined by `just`:

```bash
just run
just backend
just build
just clean
just check
just test
just profile-startup
just release 1.0.0
NOTARIZE_KEYCHAIN_PROFILE=bobe-notary just ship 1.0.0
```

`just check` is the broad verification lane: formatting, Clippy, Rust and Swift tests, audits and builds. Rust uses edition 2024, MSRV 1.97.1, `unsafe_code = "deny"`, and pedantic Clippy with documented targeted exceptions.

The current release artifact is the signed/notarized macOS application and its bundled daemon. Sparkle publication, signing and notarization are documented in [Updating OTA](UpdatingOTA.md).

The repository does not yet operate independent release channels for:

- a standalone Linux daemon;
- container images;
- mobile applications;
- ESP32 firmware;
- device OTA metadata;
- a hosted control plane.

## 14. Current architectural constraints

These are facts about the present implementation, not criticisms of the intended product:

1. `AppState` is one personal process-scoped graph and is not tenant-aware.
2. There is one pending/active personal conversation.
3. One admission permit serializes user and proactive agent turns.
4. The main `/events` stream has one current consumer and destructive queue semantics.
5. The voice protocol permits one process-wide WebSocket and requires client-side transcripts.
6. The global bearer token has broad daemon authority and no per-device scopes.
7. Client-supplied voice session IDs are correlation identifiers, not enrolled device identities.
8. Main HTTP routes are not namespaced under a versioned public API prefix.
9. Rust and Swift wire types are manually mirrored.
10. Main SSE presence is coupled to desktop capture lifecycle.
11. Remote mode is selected through startup environment variables and is not a user-facing pairing flow.
12. mDNS advertisement exists, but the macOS client does not implement remote discovery/provisioning UI.
13. There is no resumable event cursor or snapshot/replay contract.
14. There is no validated daemon backup/restore procedure.
15. Ubuntu x86_64 is a compatibility target, but no continuously validated or packaged standalone deployment exists.
16. macOS capture dependencies prevent claiming that the current daemon source is already platform-neutral.
17. The Copilot SDK depends on a child CLI process and desktop/server authentication assumptions.
18. No mobile application, device firmware, hosted control plane or fleet service exists in this repository.
19. BodyLink is an opt-in, one-enrolled-device PTT vertical slice; product enrollment, multi-device certificate registry, fleet lifecycle, and physical hardware validation remain incomplete.
20. The BodyLink speech adapter can run without visible UI, but it remains packaged in the app executable and has no independent installation or supervision path.
21. English UI copy is complete; non-English localization catalogs are partial and intentionally fall back to English for missing keys.

## 15. Architectural invariants already worth preserving

Even before the future platform design is implemented, several current properties are valuable:

- BoBe-owned state has one authoritative writer.
- Typed and spoken interaction share one companion identity and memory.
- The presentation client does not own agent credentials or durable intelligence.
- The daemon is reachable remotely only through explicit secure configuration.
- The safe default remains loopback-only.
- Secrets stay behind a daemon-side abstraction.
- File and command tools are validated at system boundaries.
- The Mac voice path remains optimized for native Apple Silicon capabilities rather than forced through a lowest-common-denominator device pipeline.

## 16. Related documents

- [BoBe Platform Architecture](platform-architecture.md) — proposed hosted, self-hosted, mobile and physical-device evolution.
- [Physical BoBe](physical-bobe.md) — voice, embodiment, room satellites and hardware detail.
- [Rust Guidelines](RUST_GUIDELINES.md) — normative Rust design and implementation rules.
- [Updating OTA](UpdatingOTA.md) — macOS release, signing, notarization, and update automation.
- [Security Policy](../SECURITY.md) — vulnerability reporting and security summary.
