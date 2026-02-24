# copilot-instructions.md

**Mandatory rules for AI assistants working on this project. Violation of any rule is a hard stop.**

---

## ABSOLUTE RULES (non-negotiable)

### 1. NEVER edit the Python repo
The original Python project at `~/Repos/BoBe/ProactiveAI` is **READ-ONLY**. You may read any file there for reference. You may **NEVER** create, edit, or delete any file in that repository. All work happens in this repo (`~/Repos/bobrust`).

### 2. NO Python anywhere
This is a pure Rust project. No `.py` files, no Python scripts, no Python tooling. Not for build scripts, not for tests, not for data processing. Everything is Rust (or SQL for migrations, TOML for config, Markdown for docs).

### 3. Migrate EVERYTHING (except STT/TTS)
The job is not done until **every single feature, concept, endpoint, tool, domain model, prompt, trigger, learner, service, and behavior** from the Python codebase has a Rust equivalent. The only exclusion is STT/TTS (voice providers, audio streaming, speech pipelines — files under `infrastructure/voice/` and voice-related schemas/controllers/factories).

Things that MUST be migrated (non-exhaustive):
- All domain models: Conversation, Goal, Memory, Observation, Soul, UserProfile, Cooldown, LearningState, MCPServerConfig
- All repository traits and implementations
- All LLM providers: Ollama, OpenAI, Azure OpenAI, llama.cpp
- Circuit breaker pattern
- All HTTP endpoints (health, conversation, capture, goals, memories, souls, tools, mcp-configs, settings, models, onboarding, events/SSE, context search, user profile)
- All SSE event types and streaming
- All native tools (search_memories, search_context, search_goal, get_goals, get_souls, get_recent_context, create_memory, update_memory, create_goal, update_goal, complete_goal, archive_goal, file_reader, list_directory, search_files, fetch_url, browser_history, discover_git_repos, discover_installed_tools, launch_coding_agent, check_coding_agent, cancel_coding_agent, list_coding_agents)
- MCP client, config manager, security, tool adapter
- Tool registry, executor, preselector, tool call loop
- All triggers: CaptureTrigger, GoalTrigger, CheckinTrigger, CheckinScheduler
- All learners: CaptureLearner, MessageLearner, MemoryLearner, GoalLearner, MemoryConsolidator
- RuntimeSession, MessageHandler, DecisionEngine, ProactiveGenerator, ResponseStreamer, SentenceAccumulator
- LearningLoop + all learning config
- All prompts: ResponsePrompt, DecisionPrompt, GoalDecisionPrompt, CapturePrompt, SummaryPrompt, MemoryDistillationPrompt, ConversationMemoryPrompt, MemoryDeduplicationPrompt, GoalExtractionPrompt, MemoryConsolidationPrompt, AgentJobEvaluationPrompt
- ContextAssembler, ConversationService, SoulService, GoalsService (+ file parser, config)
- AgentJobManager, AgentOutputParsers
- ConfigManager (runtime hot-swap), ConfigPersistence, Bootstrap, DbSeeding
- Settings (all BOBE_* env vars)
- CLI (serve, setup, version commands)
- Screen capture + OCR (macOS)
- Local embedding provider
- Ollama manager (auto-start, auto-pull)
- mDNS/network discovery
- SSE connection manager, event queue, event stream, factories
- Host validation middleware, CORS
- Exception/error handlers
- Database migrations
- Default assets (SOUL.md, GOALS.md, USER_PROFILE.md)
- Logging setup

### 4. Verify completeness by file audit
Before declaring the migration complete, do a full `find ~/Repos/BoBe/ProactiveAI/src -name '*.py' -not -name '__init__.py'` and confirm every file has a Rust counterpart. No exceptions besides STT/TTS voice files.

---

## Architecture Rules

### Layer Structure (same as Python, Rust idioms)

```
domain/         — Pure structs + enums. No framework deps. No async.
ports/          — Trait definitions (interfaces). Depends only on domain/.
application/    — Business logic. Depends on ports/ + domain/. Never on adapters/.
adapters/       — Trait implementations. Depends on ports/ + domain/.
entrypoints/    — HTTP handlers (Axum). Depends on application/ + domain/.
composition/    — Wires everything. The ONLY place that knows all concrete types.
shared/         — Tiny leaf utilities. No domain deps.
```

**Import direction:** `entrypoints/ → application/ → ports/ ← adapters/`. Composition wires them.

### Dependency Injection
No DI framework. Constructor injection via `AppState` (Arc-wrapped, passed through Axum State extractor). Repos get a pool reference; background tasks get cloned Arcs.

### Error Handling
- `thiserror` for typed domain errors
- Every handler returns `Result<T, AppError>` where `AppError: IntoResponse`
- No `unwrap()`/`expect()` in library code — always propagate errors
- Circuit breakers on ALL external calls (LLM, MCP, HTTP)

### Async
- Tokio runtime
- `spawn_blocking` for sync I/O (file reads, CPU work)
- Background tasks tracked as `JoinHandle` for graceful shutdown
- `tokio::select!` for cancellation

### Database
- SQLite default (sqlx, WAL mode). PostgreSQL optional.
- Vector search: sqlite-vec for SQLite, pgvector for Postgres
- Migrations as plain SQL via sqlx-cli

### Security
- Bind `127.0.0.1` only (never `0.0.0.0` without explicit opt-in)
- Host validation middleware
- File tools: `canonicalize()` + ancestry check
- MCP: command blocklist + env injection guard
- CORS locked to `http://localhost:5173`
- No `unsafe` without documented justification

---

## Coding Standards

- Max 500 lines per file
- Prompts in `application/prompts/`, never inline
- Persisted state > in-memory state (must survive restart)
- `Arc<dyn Trait>` for runtime polymorphism
- Naming: Handler, Loop, Trigger, Learner, Engine, Generator, Service, Provider, Repo

---

## Crate Choices

| Purpose | Crate |
|---------|-------|
| HTTP framework | `axum` |
| Async runtime | `tokio` |
| Database | `sqlx` (sqlite + optional postgres feature) |
| HTTP client | `reqwest` |
| Serialization | `serde` + `serde_json` |
| Config | `serde` + `envy` (BOBE_ prefix) |
| CLI | `clap` (derive) |
| Logging | `tracing` + `tracing-subscriber` |
| Errors | `thiserror` |
| UUID | `uuid` |
| DateTime | `chrono` |
| SSE | `axum::response::Sse` + `tokio-stream` |
| Embedding | `candle-core` + `candle-transformers` or `ort` |
| Screenshot | `screenshots` or `core-graphics` via `objc2` |

---

## Working Process Rules

### How to work on this migration

1. **Always read the Python source first.** Before writing any Rust module, read the corresponding Python file(s). The Python code is the source of truth — not the docs, not the README. If docs and code disagree, follow the code.
2. **Read docs for context, code for implementation.** The `docs/` folder in ProactiveAI gives architectural context. But when translating a specific file, read the `.py` file itself.
3. **After finishing a module, review it.** Re-read the Python source to make sure nothing was missed: every method, every field, every edge case.
4. **Cross-reference the docs.** After implementing a subsystem (e.g., all triggers), re-read the relevant doc (e.g., `docs/how-to/add-new-trigger.md`) to ensure the Rust version supports the same patterns.
5. **Translation is not 1:1.** Python async patterns become Tokio futures. Python Protocols become Rust traits. Python dataclasses become Rust structs with `#[derive(...)]`. SQLAlchemy ORM becomes sqlx queries. Litestar Controllers become Axum handlers. Understand the _intent_ of the Python code and express it idiomatically in Rust.
6. **Install real dependencies.** Use `cargo add` to add crates. Set up clippy, rustfmt. The project must compile and pass `cargo clippy` at all times.
7. **Write professional code.** No placeholder stubs, no `todo!()` macros left behind, no `unimplemented!()`. Every module must be complete and compilable.
8. **The original Python repo is NEVER edited.** Not even to add a comment. Read only.

---

## Native macOS Desktop App (desktopMac/)

### Current Task: Swift/SwiftUI Port of Electron Desktop App

The `desktop/` folder contains an Electron + React + TypeScript app. The `desktopMac/` folder is the native macOS port using Swift and SwiftUI. The goal is a pixel-faithful, feature-complete replacement of the Electron app as a native macOS .app bundle.

### What must be ported

1. **Transparent overlay window** — Frameless, always-on-top NSPanel with transparent background, anchored bottom-right. Contains the avatar, chat stack, message input, and indicator bubbles. Dynamic window resizing based on content.
2. **Avatar with animated eyes** — Circle avatar with state-dependent eye expressions (sleeping, capturing, thinking, speaking, eager, attentive), status label typewriter effect, thinking numbers ring, speaking wave bars, connection dot, message badge.
3. **Chat system** — Stacking chat bubbles (user + bobe), streaming text with blinking cursor, auto-scroll, expand/collapse for message history, max 4 visible messages.
4. **Message input** — Text input panel with send on Enter, Escape to close, auto-resize, disabled while thinking.
5. **Indicator bubble** — Shows thinking/analyzing state, tool execution progress with history, UX smoothing (delay before show, minimum display time).
6. **SSE client** — Connect to `http://localhost:8766/events`, parse StreamBundle JSON, handle indicator/text_delta/tool_call/error/heartbeat/conversation_closed events, accumulate text deltas, manage tool execution state.
7. **HTTP API client** — All REST calls to the Rust daemon: health, status, capture start/stop, send message, dismiss, goals CRUD, souls CRUD, memories CRUD, user profiles CRUD, tools list/enable/disable, MCP servers CRUD, settings get/update, models list/pull/delete.
8. **System tray (NSStatusItem)** — Menu bar icon with status text, capture toggle, show/hide, settings, quit.
9. **Settings window** — Separate window (1050×720) with sidebar navigation and content panels:
   - Context: Souls, Goals, Memories, User Profiles (split-pane with list + Monaco-equivalent editor)
   - Integrations: Tools (list with enable/disable), MCP Servers (CRUD + reconnect)
   - Preferences: Appearance (theme picker), AI Model (Ollama/OpenAI/Azure), Behavior (capture, check-ins, memory, conversation, tools), Goal Worker, Privacy
   - Advanced: For Nerds (similarity thresholds, intervals, projects directory, MCP toggle)
10. **Setup wizard** — Onboarding flow: choose local/cloud model → download Ollama + model → permissions check → complete.
11. **Theme system** — 6 themes (Bauhaus, Bauhaus Pastel, Bubblegum, Cotton Candy, Midnight Clay, Twilight Rose) with CSS variable equivalents as SwiftUI environment values.
12. **Backend service management** — Spawn bundled `bobe` binary, health check, graceful shutdown (SIGTERM → SIGKILL).
13. **Ollama service** — Download Ollama binary, verify SHA256, manage lifecycle.
14. **Security** — Bind localhost only, permission checks (screen recording, data directory).
15. **State management** — Observable state store equivalent to useSyncExternalStore pattern, with derived state types and selector-based observation.

### Architecture for desktopMac/

```
BoBe/                         — Xcode project root
  BoBe/
    App/                      — @main App, AppDelegate, WindowManager
    Models/                   — Domain types (BobeContext, ChatMessage, ToolExecution, etc.)
    Services/                 — DaemonClient (HTTP+SSE), BackendService, OllamaService, SetupService
    Stores/                   — ObservableObject state stores (BobeStore, SettingsStore, ThemeStore)
    Views/
      Overlay/                — OverlayWindow, Avatar, ChatStack, MessageInput, IndicatorBubble
      Settings/               — SettingsWindow, sidebar, all settings panels
      Setup/                  — SetupWizard steps
      Components/             — Shared UI components
    Theme/                    — ThemeConfig, color definitions
    Utilities/                — Extensions, helpers
    Resources/                — Assets.xcassets, icons
```

### Swift/SwiftUI equivalents

| Electron/React | Swift/SwiftUI |
|---|---|
| BrowserWindow (transparent, frameless) | NSPanel subclass with `.nonactivatingPanel`, `isOpaque = false`, `backgroundColor = .clear` |
| alwaysOnTop | `panel.level = .floating` + `collectionBehavior = [.canJoinAllSpaces, .fullScreenAuxiliary]` |
| Framer Motion animations | SwiftUI `.animation()`, `withAnimation()`, `matchedGeometryEffect` |
| React state + useSyncExternalStore | `@Observable` class (Observation framework) or `ObservableObject` + `@Published` |
| EventSource (SSE) | URLSession with `URLSessionDataDelegate` streaming, or AsyncBytes |
| Tailwind CSS | SwiftUI view modifiers + custom `ViewModifier` |
| Monaco Editor | `TextEditor` or NSTextView wrapped in `NSViewRepresentable` |
| framer-motion spring | SwiftUI `.spring(duration:bounce:)` |
| localStorage | UserDefaults |
| IPC (contextBridge) | Direct Swift function calls (no IPC needed — native app) |
| electron-builder | Xcode archive + notarization via `xcodebuild` |

### Rules for desktopMac/

1. **Pure Swift + SwiftUI.** No Objective-C bridging headers unless absolutely required (e.g., specific AppKit APIs). Use `@objc` only when needed for NSPanel delegate methods.
2. **macOS 14+ (Sonoma) minimum.** Use `@Observable` macro, modern SwiftUI APIs.
3. **Same visual design.** Match colors, sizing, animations, and layout exactly. Use the same color hex values, same border radii, same spacing.
4. **Same API contract.** Communicate with the same Rust backend on `localhost:8766`. Same REST endpoints, same SSE event format.
5. **Swift Package Manager** for dependencies (if any). Prefer Foundation/AppKit/SwiftUI built-ins.
6. **Max 300 lines per file.** Split views into subviews aggressively.
7. **No stubs.** Every view and service must be fully implemented.
