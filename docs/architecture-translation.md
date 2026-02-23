# Architecture Translation: Hexagonal to Idiomatic Rust

This document maps every concept from the old hexagonal architecture to the new flat Rust structure.

## Directory Mapping

| Old Path | New Path | What Changed |
|----------|----------|--------------|
| `src/domain/` | `src/models/` | Renamed. All domain structs and enums unchanged. |
| `src/ports/repos/*.rs` | `src/db/mod.rs` | 10 trait files consolidated into one `mod.rs`. Traits live next to their implementations. |
| `src/ports/llm.rs` | `src/llm/mod.rs` | `LlmProvider` trait moved into `llm/mod.rs`. |
| `src/ports/embedding.rs` | `src/llm/mod.rs` | `EmbeddingProvider` trait merged into `llm/mod.rs` alongside `LlmProvider`. |
| `src/ports/llm_types.rs` | `src/llm/types.rs` | Renamed. `AiMessage`, `AiResponse`, `StreamChunk`, `ToolDefinition`, etc. |
| `src/ports/tools.rs` | `src/tools/mod.rs` | `ToolSource` trait + `ToolCategory`, `ToolResult`, `ToolExecutionContext`, `ToolExecutionNotification` all in `tools/mod.rs`. |
| `src/adapters/persistence/repos/*.rs` | `src/db/*.rs` | SQLite implementations sit next to their trait definitions. |
| `src/adapters/llm/providers/ollama.rs` | `src/llm/providers/ollama.rs` | Same file, shorter path. |
| `src/adapters/llm/providers/openai.rs` | `src/llm/providers/openai.rs` | Same file, shorter path. |
| `src/adapters/llm/providers/llamacpp.rs` | `src/llm/providers/llamacpp.rs` | Same file, shorter path. |
| `src/adapters/llm/circuit_breaker.rs` | `src/llm/circuit_breaker.rs` | Moved up one level. |
| `src/adapters/llm/factory.rs` | `src/llm/factory.rs` | Moved up one level. |
| `src/adapters/llm/shared.rs` | `src/llm/shared.rs` | Moved up one level. |
| `src/adapters/llm/ollama_manager.rs` | `src/llm/ollama_manager.rs` | Moved up one level. |
| `src/adapters/embedding/mod.rs` | `src/llm/embedding.rs` | Directory collapsed to single file within `llm/`. |
| `src/adapters/tools/native/` | `src/tools/native/` | Moved up one level. 23 native tool files unchanged. |
| `src/adapters/tools/mcp/` | `src/tools/mcp/` | Moved up one level. `adapter.rs`, `client.rs`, `config.rs`, `security.rs`. |
| `src/adapters/tools/registry.rs` | `src/tools/registry.rs` | Moved up one level. |
| `src/adapters/tools/executor.rs` | `src/tools/executor.rs` | Moved up one level. |
| `src/adapters/tools/preselector.rs` | `src/tools/preselector.rs` | Moved up one level. |
| `src/adapters/tools/tool_call_loop.rs` | `src/tools/tool_call_loop.rs` | Moved up one level. |
| `src/adapters/network/mod.rs` | `src/util/network.rs` | Directory collapsed to single file. `MdnsAnnouncer`, `get_local_ips()`, `NetworkInfo`. |
| `src/adapters/capture/` | `src/util/capture/` | Moved. `ScreenCapture` + `capture_result.rs`. |
| `src/adapters/sse/` | `src/util/sse/` | Moved. `EventQueue`, `SseConnectionManager`, `event_stream`, `factories`, `types`. |
| `src/adapters/logging.rs` | `src/util/logging.rs` | Moved. |
| `src/application/services/*.rs` | `src/services/*.rs` | Moved up one level. |
| `src/application/services/goals/` | `src/services/goals/` | Moved up one level. |
| `src/application/runtime/*.rs` | `src/runtime/*.rs` | Moved up one level. `RuntimeSession`, `MessageHandler`, `DecisionEngine`, etc. |
| `src/application/learning/` | `src/runtime/learning/` | Nested under runtime. `LearningLoop`, `LearningConfig`, `RetentionConfig`. |
| `src/application/learners/` | `src/runtime/learners/` | Nested under runtime. 5 learners + types. |
| `src/application/triggers/` | `src/runtime/triggers/` | Nested under runtime. 4 triggers + scheduler. |
| `src/application/prompts/` | `src/runtime/prompts/` | Nested under runtime. All LLM prompt templates. |
| `src/entrypoints/app.rs` | `src/api/router.rs` | Renamed for clarity. `build_router()` function. |
| `src/entrypoints/middleware.rs` | `src/api/middleware.rs` | Moved. Host validation middleware. |
| `src/entrypoints/controllers/` | `src/api/handlers/` | Renamed. 14 handler files. |
| `src/entrypoints/error_handler.rs` | `src/api/error_handler.rs` | Moved. |
| `src/shared/clock.rs` | `src/util/clock.rs` | Moved. `Clock` trait + `SystemClock`. |
| `src/shared/ids.rs` | `src/util/ids.rs` | Moved. `new_id()` UUID generator. |
| `src/composition/container.rs` | **Deleted** | `Container::build()` inlined into `bootstrap::run()`. |
| `src/composition/bootstrap.rs` | `src/bootstrap.rs` | Moved to top level. Now contains all wiring directly. |
| `src/composition/config_manager.rs` | `src/config_manager.rs` | Moved to top level. Now also contains persistence logic. |
| `src/composition/config_persistence.rs` | **Merged** into `src/config_manager.rs` | `persist_config()` and `bobe_dir()` are private functions in config_manager. |
| `src/composition/db_seeding.rs` | `src/db/seeding.rs` | Moved into db module. |

## Import Path Mapping

| Old Import | New Import |
|------------|------------|
| `crate::domain::*` | `crate::models::*` |
| `crate::ports::repos::conversation_repo::ConversationRepository` | `crate::db::ConversationRepository` |
| `crate::ports::repos::memory_repo::MemoryRepository` | `crate::db::MemoryRepository` |
| `crate::ports::repos::goal_repo::GoalRepository` | `crate::db::GoalRepository` |
| `crate::ports::repos::observation_repo::ObservationRepository` | `crate::db::ObservationRepository` |
| `crate::ports::repos::soul_repo::SoulRepository` | `crate::db::SoulRepository` |
| `crate::ports::repos::agent_job_repo::AgentJobRepository` | `crate::db::AgentJobRepository` |
| `crate::ports::repos::user_profile_repo::UserProfileRepository` | `crate::db::UserProfileRepository` |
| `crate::ports::repos::learning_state_repo::LearningStateRepository` | `crate::db::LearningStateRepository` |
| `crate::ports::repos::cooldown_repo::CooldownRepository` | `crate::db::CooldownRepository` |
| `crate::ports::repos::mcp_config_repo::McpConfigRepository` | `crate::db::McpConfigRepository` |
| `crate::adapters::persistence::repos::*_repo::Sqlite*Repo` | `crate::db::Sqlite*Repo` |
| `crate::ports::llm::LlmProvider` | `crate::llm::LlmProvider` |
| `crate::ports::embedding::EmbeddingProvider` | `crate::llm::EmbeddingProvider` |
| `crate::ports::llm_types::*` | `crate::llm::types::*` |
| `crate::ports::tools::ToolSource` | `crate::tools::ToolSource` |
| `crate::ports::tools::ToolCategory` | `crate::tools::ToolCategory` |
| `crate::ports::tools::ToolResult` | `crate::tools::ToolResult` |
| `crate::adapters::llm::providers::ollama::*` | `crate::llm::providers::ollama::*` |
| `crate::adapters::llm::providers::openai::*` | `crate::llm::providers::openai::*` |
| `crate::adapters::llm::providers::llamacpp::*` | `crate::llm::providers::llamacpp::*` |
| `crate::adapters::llm::circuit_breaker::*` | `crate::llm::circuit_breaker::*` |
| `crate::adapters::llm::factory::*` | `crate::llm::factory::*` |
| `crate::adapters::llm::ollama_manager::*` | `crate::llm::ollama_manager::*` |
| `crate::adapters::embedding::*` | `crate::llm::embedding::*` |
| `crate::adapters::tools::native::*` | `crate::tools::native::*` |
| `crate::adapters::tools::mcp::*` | `crate::tools::mcp::*` |
| `crate::adapters::tools::registry::*` | `crate::tools::registry::*` |
| `crate::adapters::tools::executor::*` | `crate::tools::executor::*` |
| `crate::adapters::network::*` | `crate::util::network::*` |
| `crate::adapters::capture::*` | `crate::util::capture::*` |
| `crate::adapters::sse::*` | `crate::util::sse::*` |
| `crate::application::services::*` | `crate::services::*` |
| `crate::application::runtime::*` | `crate::runtime::*` |
| `crate::application::learning::*` | `crate::runtime::learning::*` |
| `crate::application::learners::*` | `crate::runtime::learners::*` |
| `crate::application::triggers::*` | `crate::runtime::triggers::*` |
| `crate::application::prompts::*` | `crate::runtime::prompts::*` |
| `crate::entrypoints::app::build_router` | `crate::api::router::build_router` |
| `crate::entrypoints::controllers::*` | `crate::api::handlers::*` |
| `crate::entrypoints::middleware::*` | `crate::api::middleware::*` |
| `crate::shared::clock::*` | `crate::util::clock::*` |
| `crate::shared::ids::*` | `crate::util::ids::*` |
| `crate::composition::bootstrap::run` | `crate::bootstrap::run` |
| `crate::composition::config_manager::ConfigManager` | `crate::config_manager::ConfigManager` |
| `crate::composition::db_seeding::*` | `crate::db::seeding::*` |

## Concept Mapping

### Hexagonal Layers to Flat Modules

| Hexagonal Concept | Old Location | New Location | Why |
|-------------------|-------------|--------------|-----|
| **Inbound Ports** | `ports/` (traits) | Trait lives in the module it belongs to (`db/mod.rs`, `llm/mod.rs`, `tools/mod.rs`) | Trait next to implementation reduces navigation |
| **Outbound Ports** | `ports/` (traits) | Same as above | No separate ports directory needed |
| **Inbound Adapters** | `entrypoints/` | `api/` | Simpler name, Rust convention |
| **Outbound Adapters** | `adapters/` | `db/`, `llm/`, `tools/`, `util/` | Each adapter lives with its trait |
| **Domain** | `domain/` | `models/` | Rust convention; avoids DDD jargon |
| **Application Services** | `application/services/` | `services/` | One less nesting level |
| **Application Runtime** | `application/runtime/` | `runtime/` | One less nesting level |
| **Composition Root** | `composition/container.rs` | `bootstrap.rs` (inlined) | No DI container; wiring is explicit in `run()` |
| **Config Persistence** | `composition/config_persistence.rs` | Merged into `config_manager.rs` | Tightly coupled, ~100 lines |

### What Stayed the Same

- **All trait signatures** — `LlmProvider`, `EmbeddingProvider`, `ToolSource`, all 10 repository traits
- **All `Arc<dyn Trait>` usage** — providers, repos, and tools remain trait objects for decoupling
- **`ArcSwap<Config>`** — hot-reload config pattern unchanged
- **`AppState` struct** — same fields, same Axum extractor pattern
- **All business logic** — services, learners, triggers, decision engine untouched
- **All domain models** — structs, enums, methods, derives identical
- **Database migrations** — `./migrations/` directory unchanged
- **SSE streaming** — event queue, connection manager, keep-alive unchanged

### What Was Removed

| Removed | Reason |
|---------|--------|
| `Container` struct | Bag of fields with no behavior; wiring inlined into `bootstrap::run()` |
| `ports/` directory | Traits moved next to their implementations |
| `adapters/` directory | Implementations moved into their domain modules |
| `application/` directory | Flattened to `services/` and `runtime/` |
| `entrypoints/` directory | Renamed to `api/` |
| `composition/` directory | `bootstrap.rs` and `config_manager.rs` moved to top level |
| `shared/` directory | Renamed to `util/` |
| `hostname` crate | Replaced with direct `libc::gethostname()` call |
| `once_cell` direct dep | Zero source-level usage; only transitive |
| All `unsafe std::env::set_var` | Config patched in-memory instead of env var round-trip |

## New Module Structure

```
src/
├── main.rs                 # CLI entry, startup, graceful shutdown
├── lib.rs                  # 13 pub mod declarations
├── config.rs               # Config struct (155 fields) + LlmBackend enum
├── config_manager.rs       # ArcSwap hot-reload + .env persistence
├── error.rs                # AppError enum + IntoResponse for Axum
├── app_state.rs            # AppState (30+ fields, Axum State extractor)
├── bootstrap.rs            # DB setup + all dependency wiring + seeding
│
├── models/                 # Domain entities (was domain/)
│   ├── types.rs            # Shared enums (ConversationState, GoalStatus, etc.)
│   ├── conversation.rs     # Conversation + ConversationTurn
│   ├── agent_job.rs        # AgentJob (state machine)
│   ├── goal.rs, memory.rs, observation.rs, cooldown.rs
│   ├── learning_state.rs, mcp_server_config.rs
│   └── soul.rs, user_profile.rs
│
├── db/                     # Repository traits + SQLite implementations
│   ├── mod.rs              # 10 trait definitions + re-exports of Sqlite* types
│   ├── conversation_repo.rs, memory_repo.rs, goal_repo.rs
│   ├── observation_repo.rs, soul_repo.rs, agent_job_repo.rs
│   ├── user_profile_repo.rs, learning_state_repo.rs
│   ├── cooldown_repo.rs, mcp_config_repo.rs
│   └── seeding.rs          # Default soul/profile seeding
│
├── llm/                    # LLM traits + all provider implementations
│   ├── mod.rs              # LlmProvider + EmbeddingProvider traits
│   ├── types.rs            # AiMessage, AiResponse, StreamChunk, ToolDefinition
│   ├── providers/          # ollama.rs, openai.rs, llamacpp.rs
│   ├── circuit_breaker.rs  # Resilience wrapper (decorator)
│   ├── factory.rs          # LlmProviderFactory
│   ├── shared.rs           # Helpers shared across providers
│   ├── ollama_manager.rs   # Ollama process lifecycle
│   └── embedding.rs        # LocalEmbeddingProvider
│
├── tools/                  # Tool traits + all tool implementations
│   ├── mod.rs              # ToolSource trait + ToolCategory, ToolResult, etc.
│   ├── registry.rs         # ToolRegistry (aggregates tool sources)
│   ├── executor.rs         # Executes tool calls
│   ├── preselector.rs      # LLM-based tool filtering
│   ├── tool_call_loop.rs   # Iterative tool calling until done
│   ├── native/             # NativeTool trait + 23 built-in tools + adapter
│   └── mcp/                # McpToolAdapter + client + config + security
│
├── services/               # Business logic orchestration
│   ├── conversation_service.rs, context_assembler.rs, soul_service.rs
│   ├── agent_job_manager.rs, agent_output_parsers.rs
│   └── goals/              # GoalsService + config + file parser
│
├── runtime/                # Background processing + decision engine
│   ├── session.rs          # RuntimeSession (main event loop)
│   ├── message_handler.rs  # Handles user messages
│   ├── decision_engine.rs  # Decides when to engage
│   ├── proactive_generator.rs, response_streamer.rs
│   ├── sentence_accumulator.rs, state.rs
│   ├── learning/           # LearningLoop + LearningConfig + RetentionConfig
│   ├── learners/           # 5 learners (capture, goal, memory, message, consolidator)
│   ├── triggers/           # 4 triggers (capture, checkin, goal, agent_job) + scheduler
│   └── prompts/            # LLM prompt templates (base, capture, decision, etc.)
│
├── api/                    # Axum HTTP layer
│   ├── router.rs           # build_router() with all route definitions
│   ├── middleware.rs        # Host validation (DNS rebinding protection)
│   ├── error_handler.rs
│   └── handlers/           # 14 handler files (goals, memories, settings, etc.)
│
└── util/                   # Infrastructure utilities
    ├── clock.rs, ids.rs    # Time + UUID generation
    ├── network.rs          # MdnsAnnouncer + get_local_ips()
    ├── logging.rs          # Tracing setup
    ├── capture/            # ScreenCapture + capture_result
    └── sse/                # EventQueue, SseConnectionManager, event_stream
```

## Dependency Wiring: Container vs Bootstrap

### Before (Hexagonal)

```
main.rs
  → composition/bootstrap.rs::run()
    → composition/container.rs::Container::build()
      → Creates Container struct (bag of 30+ Arc<dyn Trait> fields)
    → Post-build setup (Ollama, seeding, tool registration)
    → Destructures Container into AppState
```

### After (Flat)

```
main.rs
  → bootstrap.rs::run()
    → DB setup + migrations
    → All wiring as local variables (no Container struct)
    → Post-build setup (Ollama, seeding, tool registration)
    → Builds AppState directly from local variables
```

The ~500 lines of `Container::build()` are now inlined into `bootstrap::run()`. The Container struct added no behavior — it was just a temporary holding bag. Removing it eliminates one level of indirection without changing any wiring logic.

## Auxiliary Changes

### `hostname` crate removed

The `hostname` crate was replaced with a direct `libc::gethostname()` helper in `src/util/network.rs`. The project already depended on `libc` for process signal handling, so this removes one dependency.

### `unsafe std::env::set_var` eliminated

All 4 `set_var` call sites were removed:

| Location | Old Approach | New Approach |
|----------|-------------|--------------|
| `main.rs` | Set CLI args as env vars, then `Config::from_env()` | `Config::from_env()` then override fields directly |
| `config_persistence.rs` | Write .env + set_var in running process | Write .env only (pure disk writer) |
| `config_manager.rs` | set_var for API keys + `Config::from_env()` rebuild | Clone current config + patch fields in-memory |
| `onboarding.rs` | set_var for API keys directly | Route through `config_manager.update()` |

The live config is now always patched in-memory via `ArcSwap`. The `.env` file is only for persistence across restarts. `Config::from_env()` is only called once at startup.

### `once_cell` removed from direct dependencies

Zero source-level usage. It remains as a transitive dependency of other crates.
