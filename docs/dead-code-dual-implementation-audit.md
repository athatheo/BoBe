# Dead Code & Dual-Implementation Audit (Rust + Swift)

## Scope
- Rust backend: `src/**/*.rs` (172 files scanned)
- Swift desktop app: `desktopMac/BoBe/**/*.swift` (37 files scanned)
- Method/declaration sweep:
  - Rust declarations indexed: 1462
  - Rust `#[allow(dead_code)]` annotations indexed: 132
  - Swift declarations indexed: 405

## Entrypoints traced

### Rust
- `src/main.rs` (`Commands::Serve`, `Commands::Setup`, `Commands::Version`)
- `src/api/router.rs` route graph + middleware stack
- Runtime loop wiring in `bootstrap.rs` and `runtime/session.rs`

### Swift
- `desktopMac/BoBe/App/BoBeApp.swift` (`@main`, `AppDelegate`)
- App startup path:
  - `BackendService.shared.start()`
  - `DaemonClient.shared.getOnboardingStatus()`
  - `SetupWizard` / overlay + store SSE connection

---

## Confirmed isolated code removed

These were verified as unreferenced from entrypoint flows and removed after reference checks.

### Rust removals
1. **Whole modules deleted**
   - `src/runtime/sentence_accumulator.rs`
   - `src/util/sse/event_stream.rs`
   - `src/services/goal_worker/ask_user.rs`
   - `src/util/clock.rs`
   - `src/util/ids.rs`
2. **Goal-worker dead protocol path deleted**
   - Removed `action_request` SSE/event type usage and API surface:
     - `EventType::ActionRequest`
     - `/api/goal-worker/action-response` handler/route
     - `AskUserBridge` wiring from `bootstrap` and `AppState`
3. **Dormant service methods removed**
   - `AgentJobManager`: removed unused lifecycle APIs (`check`, `cancel`, `continue_job`, `poll_completed_unreported`, `cleanup_on_shutdown`, `recover_orphaned_jobs`)
   - `ContextAssembler`: removed unused methods (`get_memories`, `get_active_goals`, `get_recent_observations`, `AssembledContext::is_empty`)
   - `GoalsService`: removed unused methods (`ensure_goals_file_exists`, `get_by_id`, `update_status`, `find_similar`)
   - `ConversationService` + `ConversationRepository`: removed unused `get_recent_turns` path
   - `SoulService`: removed unused sync reload API and unused cache field
   - `goals_file_parser`: removed unused formatting helpers (`format_inferred_goal`, `format_goals_file`)
   - `util/network`: removed unused `get_local_ips` and `NetworkInfo`

### Swift removals
1. **Model/type cleanup**
   - `desktopMac/BoBe/Models/APITypes.swift`: removed unused `HeartbeatPayload`, `ActionRequestPayload`, and `.actionRequest` event case
   - `desktopMac/BoBe/Models/EntityTypes.swift`: removed unused goal-plan/MCP helper types
   - `desktopMac/BoBe/Models/BobeTypes.swift`: removed unused `SpringConfig`
2. **View/store cleanup**
   - `desktopMac/BoBe/Views/Settings/BehaviorPanel.swift`: removed unused `SettingsSection`
   - `desktopMac/BoBe/Stores/BobeStore.swift`: removed unreachable `.actionRequest` handling

---

## Dual implementations (resolved in this pass)

### Rust
1. **HTTP request logging**
   - Removed duplicate `TraceLayer::new_for_http()` path.
   - Kept single custom request-logging middleware.
2. **SSE construction**
   - Consolidated queue/streaming emission through shared factories in `util/sse/factories.rs`.

### Swift
1. **HTTP transport**
   - Moved Privacy panel `/app/data-size` and `/app/delete-all-data` calls into `DaemonClient`.
   - Removed direct `URLSession` transport split from settings UI.

---

## Additional non-critical removals (latest pass)

After the critical-path wiring fixes, a second sweep removed dormant helpers that were still isolated from any entrypoint/runtime flow.

### Rust removals
- `RuntimeSession::is_running`
- `CaptureTrigger::{is_enabled, context_count}`
- `CheckinTrigger::is_scheduled`
- `checkin_scheduler` dormant API (`CHECKIN_MESSAGES`, `get_random_checkin_message`, `update_schedule`, `enable`, `disable`, `is_enabled`)
- `DecisionEngine::decide_for_message`
- `ProactiveGenerator::{send_proactive_checkin, send_checkin}`
- `ToolCallLoop::{run, execute_tools_with_notifications}` (kept streaming loop path only)
- `ConfigManager::{current, current_llm, current_embedding}`
- `Goal::{complete, archive}` helper methods
- `ToolRegistry` dormant helper APIs (`unregister`, `get_tools_by_category`, `get_source`, `source_names`, `get_source_name_for_tool`, `health_check_all`)
- `NativeToolAdapter` dormant toggling helpers (`enable_tool`, `disable_tool`, `set_tool_enabled`, `is_tool_enabled`) and unused write-lock helper
- Removed dormant `ToolSource::categories` surface and adapter category-cache fields (native + MCP), keeping only active get/execute source contracts.
- Removed dormant native-tool category plumbing (`ToolCategory` enum, `NativeTool::category`, and per-tool `category()` impls) since product flows never filter/select tools by category.
- Removed dormant tool-source health surface (`ToolSource::health_check`, adapter impls, and unused `McpClient` passthrough helpers), keeping runtime health checks focused on active LLM/backend paths.
- Removed unused `enabled` field from `McpParsedServer` (enabled filtering already happens before parsing to runtime server objects).

### Swift removals
- `desktopMac/BoBe/Models/EntityTypes.swift`: removed unused `MCPConfig` type.
- `desktopMac/BoBe/Services/DaemonClient.swift`: removed unused frontend API helpers (`status`, `getGoal`, `getSoul`) after full desktop callsite sweep.

---

## Validation
- `cargo fmt`
- `cargo clippy -q`
- `cargo test -q`
- `cd desktopMac && swift build -c debug`

All passed after the removals above.
