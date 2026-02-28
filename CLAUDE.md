# CLAUDE.md

## Project Overview

BoBe — local-first proactive AI companion. Rust backend (`bobe-daemon`) + native Swift/SwiftUI macOS overlay app. Communication via HTTP REST + SSE on `localhost:8766`.

## Build Commands

All via `just` (see `justfile`):

```bash
just dev          # Debug build (backend + frontend)
just serve        # cargo run -- serve
just build        # Release build + bundle .app
just clean        # Clean all artifacts
just test         # fmt + clippy + cargo test + swiftlint + swift build
just release 1.0.0  # build → sign → DMG
```

## Architecture

```
bobe-daemon (Rust/Axum, :8766)  ←── SSE + HTTP ──→  BoBe.app (Swift/SwiftUI)
         │                                                   │
         ▼                                                   ▼
   LLM Backend                                    Transparent overlay
   (Ollama/OpenAI/Azure/llama.cpp)                + settings + tray
```

### Rust Backend (`src/`)

`main.rs` — clap CLI (`serve`, `setup`, `version`). `serve` boots Axum on `127.0.0.1:8766`, spawns background tasks (SSE heartbeat, RuntimeSession triggers, LearningLoop, GoalWorkerManager), graceful shutdown via `ctrl_c()`.

| Module | Purpose |
|--------|---------|
| `api/` | Axum routes (`router.rs`), middleware (host validation, CORS), handler modules under `handlers/` |
| `models/` | Domain structs (Conversation, Goal, Memory, Observation, Soul, UserProfile, etc.) |
| `db/` | Repository layer — one repo per entity, sqlx queries against SQLite |
| `runtime/` | State machine: RuntimeSession, DecisionEngine, ProactiveGenerator, ResponseStreamer, MessageHandler |
| `runtime/triggers/` | Background triggers: capture, goal eval, check-in scheduling, agent jobs |
| `runtime/learners/` | Learning pipeline: message→memory, capture→observation, dedup, goal extraction, consolidation |
| `runtime/prompts/` | All LLM prompt templates (never inline elsewhere) |
| `services/` | Business logic: ConversationService, ContextAssembler, SoulService, GoalsService, AgentJobManager, GoalWorker |
| `tools/` | ToolRegistry, ToolExecutor, ToolCallLoop (agentic), Preselector; native tools under `native/`; MCP under `mcp/` |
| `llm/` | Provider abstraction (Ollama, OpenAI, Azure via OpenAI, llama.cpp); circuit breaker, embedding, runtime swappable |
| `bootstrap/` | Dependency wiring → `AppState` construction |
| `config.rs` | All `BOBE_*` env var settings; `ConfigManager` supports runtime hot-swap |
| `config_manager/` | Persistence to `~/.bobe/.env` |
| `util/` | SSE connection manager, screen capture, similarity math, text utils |

**DI:** No framework. Constructor injection via `AppState` (Arc-wrapped, Axum State extractor).

**Database:** SQLite via sqlx. Migrations in `migrations/`, auto-run on startup. All entities use UUID (BLOB) primary keys. Data at `~/.bobe/data/bobrust.db`.

### Swift macOS App (`desktopMac/`)

Swift 6.0, macOS 14+ (Sonoma), SPM.

| Directory | Purpose |
|-----------|---------|
| `App/` | `@main` + AppDelegate, OverlayPanel (NSPanel), SettingsWindow, TrayManager |
| `Models/` | API DTOs, app types, entity types, settings types |
| `Services/` | BackendService (daemon lifecycle), DaemonClient (HTTP + SSE), OllamaService |
| `Stores/` | BobeStore (`@Observable` main state), ThemeStore |
| `Views/Overlay/` | Transparent overlay: avatar, chat bubbles, message input, indicators |
| `Views/Settings/` | Settings sidebar: AI model, appearance, behavior, tools, MCP, memories, goals, souls, profiles, advanced |
| `Views/Setup/` | Onboarding wizard (LLM choice → download → model pull → permissions) |

**Patterns:** `@Observable`, NSPanel `.nonactivatingPanel`, `URLSession.bytes(for:)` for SSE, UserDefaults.

## Coding Conventions

### Rust
- Edition 2024, MSRV 1.93, `unsafe_code = "deny"`
- Clippy pedantic (see `[lints.clippy]` in Cargo.toml)
- `thiserror` errors, handlers return `Result<T, AppError>`, no `unwrap()`/`expect()`
- Prompts only in `runtime/prompts/`
- Config via `BOBE_*` env vars

### Swift
- SwiftLint: see `desktopMac/.swiftlint.yml`
- `sorted_imports` enforced, `force_unwrapping` opt-in (avoid)
- Split large views into subviews

## Security

- Binds `127.0.0.1` only
- Host validation middleware on all routes
- File tools: `canonicalize()` + ancestry checks
- MCP: command blocklist + env injection guards
- CORS locked to localhost
