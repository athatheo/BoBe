# SwiftUI catch-up to the new daemon API contract

> **Status:** plan approved, ready to execute. Daemon side is fully done. Swift foundation (Models + Services) landed in commit `9c1d6ae`. View layer is the remaining work — 6 broken files + welcome wizard + i18n purge.
>
> **Branch:** `feat/copilot-sdk-pivot`
> **Plan file:** `/Users/john/.claude/plans/iridescent-percolating-octopus.md` (canonical) → mirrored here at repo root for review.

---

## Quick links

- [Tasks](#tasks)
- [Context](#context)
- [Daemon API quick reference](#daemon-api-quick-reference)
- [Reusable Swift primitives](#reusable-swift-primitives)
- [Sidebar layout (final)](#sidebar-layout-final)
- [Pane-by-pane plans](#pane-by-pane-plans)
- [Welcome wizard](#welcomewizard--new-commit-7-task-47)
- [i18n purge](#i18n-purge-commit-8-task-43)
- [Commit plan](#commit-plan)
- [Acceptance criteria](#acceptance-criteria-per-pane)
- [Verification](#verification)
- [Risks & open notes](#risks--open-notes)
- [Files touched](#files-touched)

---

## Tasks

### Active SwiftUI work (this plan)

| # | Status | Title |
|---|--------|-------|
| 42 | in_progress | SwiftUI commit 1: rebuild BehaviorPanel for 9-field DTO |
| 48 | pending | SwiftUI commit 2: rebuild AdvancedPanel + add RestartRequiredBanner |
| 46 | pending | SwiftUI commit 3: rebuild GoalsEditor for rich-section daemon shape |
| 45 | pending | SwiftUI commit 4: rebuild MemoriesEditor as single-doc CodeEditor + byte gauge |
| 49 | pending | SwiftUI commit 5: split PrivacyPanel, drop GoalWorkerPanel, fix nuke memory step |
| 44 | pending | SwiftUI commit 6: trim SettingsWindow sidebar + locale fix + overview cards |
| 47 | pending | SwiftUI commit 7: thin first-launch welcome wizard |
| 43 | pending | SwiftUI commit 8: i18n purge dead namespaces + add new keys across 9 locales |

### Broader umbrella tasks (still tracked)

| # | Status | Title |
|---|--------|-------|
| 27 | in_progress | SwiftUI overlay catch-up to new daemon API contract (encompasses #42–#49) |
| 28 | pending | End-to-end smoke test: run the daemon and exercise each flow |
| 31 | pending | Live MCP runtime state via SDK sideband query (daemon-side; unblocks `MCPServer.connected/tool_count/tools`) |

### Recently completed (this branch, for context)

| # | Status | Title |
|---|--------|-------|
| 24 | ✅ | Fix UTF-8 byte-slice panic in DecisionEngine debug log |
| 25 | ✅ | CaptureLearner: surface persistent vision failures vs no-content |
| 26 | ✅ | Harden BatchWorker JSON extraction: prefer last balanced object |
| 29 | ✅ | Add InfiniteSessionConfig to batch worker classes (Goals/Observe/Decide/Consolidate) |
| 30 | ✅ | Bound consolidation writer-lock holding time |
| 32 | ✅ | Decide chat session retention policy + prune SessionStore |
| 33 | ✅ | P1 dead traits/variants: repo methods, ChatWorker abort/compact, IndicatorType::ToolCalling, EventType::ToolCall |
| 34 | ✅ | P1 cargo deps + comment hygiene |
| 35 | ✅ | P1 config drift: delete soul_file, goals.max_active, coding_agent.poll_interval, ghost HOT_SWAP_FIELDS |
| 36 | ✅ | P1 strip allow(dead_code) blanket attrs |
| 37 | ✅ | P0 boot/wiring fixes: integrity, schema compat, binary_manager, http_client, spike |
| 38 | ✅ | P1 i18n cleanup: delete onboarding.ftl, trim prompts.ftl |
| 39 | ✅ | Nuke i18n + locale_override |
| 40 | ✅ | Final small cleanups: UsageMeter, EndOfTurn, tools→mcp, cooldown Option collapse |
| 41 | ✅ | Nuke coding-agent subsystem (no entry point — dead cycle) |

---

## Context

The BoBe Rust daemon was rewritten across 12 commits this branch. It now exposes a much smaller surface (~33 routes, 9-field settings, single-doc memory, rich-section file-backed goals, no onboarding/models/tools/goal-worker endpoints). The Swift Models + Services layers were already rewritten in commit `9c1d6ae` to match — but the view layer hasn't caught up. `swift build` currently fails in 6 files, all under `Features/Settings/`. ~136 dead i18n keys × 9 locales also need pruning.

This plan finishes the Swift catch-up: rebuild the broken settings panes around the new daemon shape, add a lightweight welcome wizard with Copilot CLI presence check, purge dead localization keys, and land a green build.

**User constraints (decided):**
- Keep editing surfaces for goals + memories + souls + user-profiles + MCP in Settings (same place as before).
- All 9 surviving config fields must be editable.
- Keep all 9 locales.
- `archived` IS a status the picker can set (full CRUD: create / edit / archive-via-picker / delete).
- Welcome wizard kept, slimmed to: welcome explanation → Copilot CLI presence check → permissions → done.
- Settings stays split: BehaviorPanel (capture/checkins/conversation, 6 fields) + AdvancedPanel (goals/MCP, 3 fields).

---

## Daemon API quick reference

### Routes (33 endpoints, grouped)

| Group | Method + path | Notes |
|---|---|---|
| Health | `GET /health` | Returns 200 even on DB error; inspect `services.database` |
| | `GET /status` | `{indicator, capturing, accepting_user_messages, version}` |
| SSE | `GET /events` | Single-consumer; second consumer kicks first off silently |
| Chat | `POST /message` | Returns `{message_id}` immediately; reply streams via `/events`. 409 with hardcoded "BoBe is still..." string when busy |
| Capture | `POST /capture/{start,stop}` | Toggle loop; auto-gated on SSE connection |
| | `POST /capture/once` | Diagnostic-only; emits NO SSE events |
| Goals | `GET /goals?status=&include_archived=` | Hides archived by default |
| | `POST /goals` | `{title, summary?, why_it_matters?, priority?}` — only 4 fields settable on create |
| | `GET /goals/{id}` | Full GoalResponse (11 fields) |
| | `PATCH /goals/{id}` | Only 6 fields: `title, status, priority, summary, why_it_matters, notes` |
| | `POST /goals/{id}/{complete,archive}` | Sets status |
| | `DELETE /goals/{id}` | 204 |
| Memory | `GET /memory` | `{content, bytes}` — single document |
| | `PUT /memory` | `{content}` — full replacement |
| Souls | `GET/POST /souls`, `GET /souls/by-name/{name}`, `GET/PATCH/DELETE /souls/{id}`, `POST /souls/{id}/{enable,disable}` | Copy-on-write on default-row PATCH |
| User Profiles | (same shape as Souls) | **NO** copy-on-write on default PATCH (asymmetric) |
| Settings | `GET/PATCH /settings` | 9-field DTO |
| MCP | `GET/PUT/DELETE /tools/mcp/config`, `POST /tools/mcp/config/validate` | `connected/tool_count/tools` are stubbed defaults pending task #31 |

### Settings DTO (9 fields total)

```rust
struct SettingsResponse {
    capture_enabled: bool,                            // hot-applied on next SSE connect
    capture_interval_seconds: u64,                    // hot-applied next loop iter
    checkin_enabled: bool,                            // ⚠ daemon claims hot, actually restart-required
    checkin_times: Vec<String>,                       // ⚠ same
    checkin_jitter_minutes: u32,                      // ⚠ same
    conversation_inactivity_timeout_seconds: u64,     // hot-applied
    conversation_auto_close_minutes: u64,             // hot-applied
    goal_check_interval_seconds: f64,                 // hot-applied
    mcp_enabled: bool,                                // ⚠ daemon claims hot, actually restart-required (sessions captured map at boot)
}
```

### SSE event vocabulary

| Event | Payload | Notes |
|---|---|---|
| `indicator` | `{indicator, message?, progress?}` | SCREAMING_SNAKE_CASE values: `IDLE, SCREEN_CAPTURE, THINKING, STREAMING`. **No `TOOL_CALLING`** |
| `text_delta` | `{delta, sequence, done}` | **`done: true` IS the end-of-turn marker** (no separate event) |
| `tool_call_start` | `{tool_name, tool_call_id, status: "start"}` | |
| `tool_call_complete` | `{tool_name, tool_call_id, success, error: null, duration_ms: null, status: "complete"}` | error/duration_ms always null today |
| `error` | TWO shapes: `{code, message, recoverable}` (chat) **or** `{trigger, message, recoverable}` (trigger) | Recoverable=true means stream may continue |
| `heartbeat` | `{}` | Every 15s |
| `conversation_closed` | `{conversation_id, reason: "inactivity_timeout", turn_count}` | UI clears history |

### Goal MD shape

Stored at `~/.bobe/goals/<id>.md`. Sections (canonical order, all optional, round-trip as empty strings):
1. `Summary`
2. `Why It Matters`
3. `How They're Working On It` (apostrophe is significant for parser)
4. `Patterns Observed`
5. `Attitude & Feelings` (ampersand significant)
6. `Open Questions`
7. `Notes`

Required: `# <title>` heading + `**ID**: <uuid>`. Defaults: status=`active`, priority=`2`, created_at/updated_at=`now()`. `GoalStatus` wire values lowercase: `active|paused|completed|archived`.

### Memory MD shape

Stored at `~/.bobe/memory.md`. Default skeleton (`MemoryFile::DEFAULT_BODY` in daemon):

```md
# BoBe Memory

## Profile

## Active Goals

## Long-term

## Recent
```

Pruned nightly by `Consolidate` worker at **03:00 local**. `TARGET_MAX_BYTES = 50KB`. PUT mid-prune resolves silently (consolidation aborts that night).

---

## Reusable Swift primitives

> **Do not reinvent these.** Reference by file + symbol name; pass theme via `@Environment(\.theme)` unless noted.

### Settings framework

| Symbol | File | Purpose |
|---|---|---|
| `SettingsEditorState<SelectionID: Hashable>` | `Features/Settings/SettingsEditorFramework.swift` | Generic state struct (selection, isLoading, isSaving, isDirty, isCreating, errorMessage, showDeleteConfirmation) |
| `SettingsEditorScaffold<List, Detail, Empty>` | same | Split-pane scaffold with auto-select-first |
| `SettingsEditorActionRow` | same | Delete/duplicate row at the bottom of detail |
| `SettingsEditorSaveActions` | same | Discard / Save row |
| `SettingsEditorErrorText` | same | Small caption-size red text |
| `SettingsPaneHeader(title:onAddNew:)` | `Features/Settings/SharedControls.swift` | Title + optional `+ Add new` button |
| `SettingsRow` | same | Label + control row |
| `CollapsibleSection` | same | Section header that expands. **Need to extend with `aiCuratedBadge: Bool` for Goals editor** |
| `DebouncedNumberInput` / `DebouncedDecimalInput` | same | 600ms debounce + range validation |
| `BobeSelectableRow<Content>` | same | List row with hover/selected/accessibility |
| `BobeSpinner(size:)` | same | Loading indicator |

### Themed primitives

| Symbol | File | Purpose |
|---|---|---|
| `BobeButton{Style}` (`.primary`, `.secondary`, `.ghost`) | `Features/Settings/ThemedControls.swift` | Buttons |
| `BobeTextField` | same | Themed text field |
| `BobeSecureField` | same | Password field |
| `BobeMenuPicker<Value>` | same | Themed menu picker |
| `BobeToggle` | same | Themed toggle |
| `BobeLinearProgressBar` | same | Linear progress (used for memory byte gauge) |
| `bobeInputChrome` | same | Modifier for input field decoration |
| `bobeTextStyle(.rowTitle/.rowMeta/.helper/.heading/.body/.badge)` | same | Typographic styles |

### Editors / IO

| Symbol | File | Purpose |
|---|---|---|
| `CodeEditor(text:theme:fontSize:)` | `Features/Settings/CodeEditor.swift` | NSTextView wrapper. Takes `theme: ThemeConfig` directly (NSViewRepresentable can't read environment cleanly) |

### Theme + locale

| Symbol | File | Purpose |
|---|---|---|
| `ThemeKey: EnvironmentKey` + `EnvironmentValues.theme` | `Theme/ThemeConfig.swift` | Universal injection — every view does `@Environment(\.theme) private var theme` |
| `L10n.tr(_ key:_ args:CVarArg...)` | `Util/Localization.swift` | Localized string lookup |
| `L10n.setLocaleOverride(_:)` | same | Override active bundle |
| `BobeStore.localeOverride` | `Stores/BobeStore.swift` | UserDefaults-backed, `bobe.locale_override` key |
| `BobeStore.supportedLocales` | same | Static array of 9 BCP-47 codes |
| `BobeStore.applyPersistedLocale()` / `updateLocale(_:)` | same | Read/write locale override |

### Stores

| Symbol | File | Purpose |
|---|---|---|
| `BobeStore.shared` | `Stores/BobeStore.swift` | `@Observable @MainActor` singleton; SSE event aggregation, message lifecycle, capture sync |
| `ThemeStore.shared` | `Stores/ThemeStore.swift` | Persists theme to UserDefaults `bobe_theme_id` |
| `ToolExecutionController` | `Stores/ToolExecutionController.swift` | Owned by BobeStore; manages running/complete tool executions |

### Services

| Symbol | File | Purpose |
|---|---|---|
| `DaemonClient.shared` | `Services/DaemonClient.swift` | Actor; HTTP wrapper + SSE consumer |
| `BackendService.shared` | `Services/BackendService.swift` | Actor; daemon process lifecycle (spawn/health/restart/stop) |
| `DaemonError` | `Services/DaemonTypes.swift` | `.invalidResponse, .httpError(code, msg), .connectionFailed, .operationFailed` |

---

## Architecture overview

```
Swift catch-up = 8 commits

  ┌───────────────────────────────────────────────────────────┐
  │ FOUNDATION (already landed in 9c1d6ae)                    │
  │   Models  ✅  Services  ✅  Stores  ✅  Overlay  ✅       │
  │   SettingsEditorFramework / SharedControls / Themed* ✅   │
  │   SoulsEditor ✅  UserProfilesEditor ✅  MCPServersPanel ✅│
  │   AppearancePanel ✅                                      │
  └───────────────────────────────────────────────────────────┘
           ↓ this plan rebuilds the 6 broken files + adds wizard ↓
  ┌───────────────────────────────────────────────────────────┐
  │ THIS PLAN                                                  │
  │  • BehaviorPanel  — capture / checkins / conversation     │
  │  • AdvancedPanel  — goals / MCP toggle                    │
  │  • GoalsEditor    — list+detail with rich sections        │
  │  • MemoriesEditor — single-doc CodeEditor + byte gauge    │
  │  • PrivacyPanel   — split off; new memory reset path      │
  │  • SettingsWindow — sidebar trim + locale fix             │
  │  • WelcomeWizard  — first-launch only                     │
  │  • i18n purge     — 9 locales                             │
  │  • RestartRequiredBanner (new shared component)            │
  └───────────────────────────────────────────────────────────┘
```

---

## Sidebar layout (final)

```swift
enum SettingsCategory: String, CaseIterable, Identifiable {
    case souls, goals, memories
    case userProfiles = "user-profiles"
    case mcpServers   = "mcp-servers"
    case appearance, behavior, advanced, privacy
    var id: String { rawValue }
}

enum SettingsCategoryGroup {
    case context        // souls, goals, memories, userProfiles
    case integrations   // mcpServers
    case preferences    // appearance, behavior, privacy
    case advanced       // advanced
}
```

Drop `aiModel`, `tools`, `goalWorker` cases entirely.

---

## Pane-by-pane plans

### `BehaviorPanel.swift` — rewrite (commit 1, task #42)

**File:** `BoBeMacUI/BoBe/Features/Settings/BehaviorPanel.swift`

Sections (debounce-on-edit, no Save button — preserve existing convention):

1. **Capture**
   - `captureEnabled` toggle
   - `captureIntervalSeconds` (`DebouncedNumberInput`, range `1...600`)
   - Existing capture-permission warning ("Open System Settings" link)
2. **Check-ins**
   - `checkinEnabled` toggle
   - `checkinTimes` (existing FlowLayout pills, add/remove)
   - `checkinJitterMinutes` (`DebouncedNumberInput`, range `0...30`)
3. **Conversation**
   - `conversationAutoCloseMinutes` (`DebouncedNumberInput`, range `1...60`)

**Drops:** `memorySection`, `toolsSection`, `conversationSummaryEnabled` row, learning section.

**`debounceSave()` body:**

```swift
var req = SettingsUpdateRequest()
req.captureEnabled = currentSettings.captureEnabled
req.captureIntervalSeconds = currentSettings.captureIntervalSeconds
req.checkinEnabled = currentSettings.checkinEnabled
req.checkinTimes = currentSettings.checkinTimes
req.checkinJitterMinutes = currentSettings.checkinJitterMinutes
req.conversationAutoCloseMinutes = currentSettings.conversationAutoCloseMinutes
let resp = try await DaemonClient.shared.updateSettings(req)
self.applyUpdateResponse(resp)
```

**Restart-required banner shadow set:**

```swift
private static let DEFER_TO_RESTART_FIELDS: Set<String> = [
    "checkin_enabled", "checkin_times", "checkin_jitter_minutes"
]
```

When any saved field intersects this set OR appears in `restart_required_fields` from the daemon response, show `RestartRequiredBanner` at the top of the pane. Banner is dismissible; reappears on next save touching the set.

---

### `AdvancedPanel.swift` — rewrite (commit 2, task #48)

**File:** `BoBeMacUI/BoBe/Features/Settings/AdvancedPanel.swift`

Sections:

1. **Goals**
   - `goalCheckIntervalSeconds` (Double on the wire; render as Int via `intBinding(...)` — range `60...7200`, suffix "seconds")
2. **Conversation**
   - `conversationInactivityTimeoutSeconds` (`DebouncedNumberInput`, range `5...600`, suffix "seconds")
3. **MCP**
   - `mcpEnabled` toggle (with description: "Enable Model Context Protocol servers (changes apply at next daemon restart)")

**Drops:** `similaritySection`, `learningSection`, `projectsSection`, locale-refresh code (lines 226–229).

**Shadow set:** `DEFER_TO_RESTART_FIELDS = ["mcp_enabled"]`.

---

### `RestartRequiredBanner.swift` — new (commit 2, task #48)

**File:** `BoBeMacUI/BoBe/Features/Settings/RestartRequiredBanner.swift`

```swift
struct RestartRequiredBanner: View {
    let fields: Set<String>
    let onDismiss: () -> Void
    @Environment(\.theme) private var theme

    var body: some View {
        HStack(spacing: 8) {
            Image(systemName: "arrow.triangle.2.circlepath")
                .foregroundStyle(self.theme.colors.tertiary)
            Text(L10n.tr("settings.shared.restart_banner.message"))
                .font(.system(size: 12))
                .foregroundStyle(self.theme.colors.text)
            Spacer()
            Button(L10n.tr("settings.shared.restart_banner.dismiss"), action: self.onDismiss)
                .bobeButton(.ghost, size: .mini)
        }
        .padding(10)
        .background(RoundedRectangle(cornerRadius: 8).fill(self.theme.colors.tertiary.opacity(0.12)))
        .overlay(RoundedRectangle(cornerRadius: 8).stroke(self.theme.colors.tertiary.opacity(0.4), lineWidth: 1))
    }
}
```

Used by Behavior + Advanced panels.

---

### `GoalsEditor.swift` — rewrite (commit 3, task #46)

**File:** `BoBeMacUI/BoBe/Features/Settings/GoalsEditor.swift`

List+detail via `SettingsEditorScaffold` + `SettingsEditorState<String>`. Single internal `GoalDraft` struct holds the 6 user-editable fields:

```swift
struct GoalDraft {
    var title: String = ""
    var summary: String = ""
    var whyItMatters: String = ""
    var notes: String = ""
    var priority: Int = 2
    var status: GoalStatus = .active

    init(_ goal: Goal? = nil) {
        guard let goal else { return }
        self.title = goal.title
        self.summary = goal.summary
        self.whyItMatters = goal.whyItMatters
        self.notes = goal.notes
        self.priority = goal.priority
        self.status = goal.status
    }
}
```

#### List pane

- `SettingsPaneHeader(title:)` with `+` toggle for inline create.
- "Show archived" toggle (`@State` Bool); calls `listGoals(includeArchived:)`.
- Status filter `BobeMenuPicker<GoalStatus?>` with `[nil, .active, .paused, .completed, .archived]`.
- Inline create form: `BobeTextField` (title) + `BobeMenuPicker<Int>` (priority 0-5, default 2) + Create / Cancel.
- Goal rows: priority dot (color-mapped), title, meta `"<status> • P<priority> • updated <date>"`. No per-row toggle.

#### Detail pane

- **Header strip:** editable title (`BobeTextField`), status pill, priority pill, action buttons:
  - `active` → "Pause" (PATCH status=paused), "Complete" (POST /complete), "Archive" (POST /archive)
  - `paused` → "Resume" (PATCH status=active), "Complete", "Archive"
  - `completed` → "Reactivate" (PATCH status=active)
  - `archived` → "Restore" (PATCH status=active)
- **Editable sections** (`CollapsibleSection`s, expanded by default):
  - Status (`BobeMenuPicker<GoalStatus>` — full set including `.archived` for direct CRUD)
  - Priority (`BobeMenuPicker<Int>` 0-5)
  - Summary (`CodeEditor`, ~80pt tall)
  - Why It Matters (`CodeEditor`, ~100pt tall)
  - Notes (`CodeEditor`, ~240pt tall — dated entries scratchpad)
- **Read-only "AI-curated" sections** (`CollapsibleSection` with new `aiCuratedBadge: Bool` parameter, collapsed by default):
  - How They're Working On It
  - Patterns Observed
  - Attitude & Feelings
  - Open Questions
- Empty agent-only sections show `"BoBe hasn't filled this in yet."` muted stub.
- **Bottom:** `SettingsEditorActionRow` (delete confirm) + `SettingsEditorSaveActions`.

Save builds:

```swift
GoalUpdateRequest(
    title: draft.title,
    status: draft.status,
    priority: draft.priority,
    summary: draft.summary,
    whyItMatters: draft.whyItMatters,
    notes: draft.notes
)
```

`priorityColor(_ priority: Int) -> Color`:
- 0-1 → `theme.colors.textMuted`
- 2 → `theme.colors.secondary`
- 3 → `theme.colors.tertiary`
- 4-5 → `theme.colors.primary`

**Drops:** `GoalPriority` enum, `GoalSource` enum, `goalSourceLabel(_:)`, every `goal.enabled` reference.

**Extension to `CollapsibleSection`** in `SharedControls.swift`: add an optional `aiCuratedBadge: Bool = false` parameter that renders an "AI-curated" badge in the section header.

---

### `MemoriesEditor.swift` — rebuild as single-doc editor (commit 4, task #45)

**File:** `BoBeMacUI/BoBe/Features/Settings/MemoriesEditor.swift`

```
+--------------------------------------------------------+
| Memory                                                  |
| BoBe's living narrative. Pruned automatically each night.|
|                                                         |
| ┌─────────────────────────────────────────────────────┐│
| │ # BoBe Memory                                       ││
| │ ## Profile                                          ││
| │   ...                                               ││
| └─────────────────────────────────────────────────────┘│
|                                                         |
| [████████░░░░░░░░░░] 12.4 KB / 50 KB                    |
|                                                         |
| [Reload]                          [Reset]   [Save]      |
| Saved.                                                  |
+--------------------------------------------------------+
```

Components:
- `SettingsPaneHeader(title:)` (no `+` action).
- Description text from `settings.memory.description`.
- `CodeEditor(text: $text, theme: theme, fontSize: 13)` — `minHeight: 360`, flex up.
- `BobeLinearProgressBar(progress: Double(text.utf8.count) / 51200)` — caps at 1.0.
- Bytes label: `"X.X KB / 50 KB"` (use `ByteCountFormatter`).
- **Action row:**
  - "Reload" (left, secondary) — re-GETs `/memory`; warn if unsaved changes
  - "Reset to default" (right, destructive secondary) — replaces editor body with skeleton; doesn't auto-save
  - "Save" (right, primary) — PUTs full body
- **Status footer:** success in `theme.colors.secondary`; error in `theme.colors.primary`.

Mirror daemon `MemoryFile::DEFAULT_BODY`:

```swift
private let defaultMemoryBody = """
# BoBe Memory

## Profile

## Active Goals

## Long-term

## Recent
"""
```

**Conflict heuristic:** after save returns, if response `bytes` differs from sent length by >200 bytes, suspect consolidation ran — show one-time toast `settings.memory.consolidation_hint`.

**State:**

```swift
@State private var text: String = ""
@State private var savedText: String = ""    // last loaded server body
@State private var bytes: Int = 0
@State private var isLoading = false
@State private var isSaving = false
@State private var status: String?
@State private var error: String?
@Environment(\.theme) private var theme
private let targetMaxBytes = 50 * 1024
```

**Drops:** every reference to `Memory`, `MemoryType`, `MemoryCategory`, `MemorySource`, `MemoryListResponse`, `MemoryCreateRequest`, `MemoryUpdateRequest(category:)`, `MemoryActionResponse`, `listMemories`, `createMemory`, `enable/disableMemory`, `deleteMemory`. All filter/list/category UI.

---

### `PrivacyPanel.swift` — split off + fix nuke flow (commit 5, task #49)

**Action:** rename `PrivacyGoalWorkerPanels.swift` → `PrivacyPanel.swift`. Delete the entire `GoalWorkerPanel` struct.

Update `PrivacyPanel.deleteAllData()`:

- Replace memories per-id loop (lines 132-143) with a **single call**: `_ = try await DaemonClient.shared.updateMemory(content: defaultMemoryBody)`.
- Keep goals/souls/userProfiles/MCP delete loops (their endpoints survive).

L10n changes:
- Drop `settings.privacy.danger.error.memory_format` (per-id format).
- Add `settings.privacy.danger.error.memory` (singular).
- Update `settings.privacy.storage.included.list` copy from "Memories and observations" → "Memory document".

---

### `SettingsWindow.swift` — sidebar trim + locale fix + overview cards (commit 6, task #44)

**File:** `BoBeMacUI/BoBe/Features/Settings/SettingsWindow.swift`

- Drop `.tools, .aiModel, .goalWorker` from `SettingsCategory` enum and `label`/`icon` arms.
- Update `SettingsCategoryGroup.preferences.categories` → `[.appearance, .behavior, .privacy]`.
- Remove `ToolsPanel()`, `AIModelPanel()`, `GoalWorkerPanel()` cases from `settingsContent` switch.
- **Locale rerender fix:** `.id(self.store.localeOverride)` (cheap; reactive via `@Observable` BobeStore).
- `SettingsOverview`: drop the 5th card; keep 4-card 2×2 grid:

| Card | Icon | Heading L10n | Target |
|------|------|--------------|--------|
| Sees | `eye.fill` | `card.sees.heading` | `.behavior` |
| Remembers | `brain.head.profile` | `card.remembers.heading` | `.memories` |
| Speaks | `message.fill` | `card.speaks.heading` | `.behavior` |
| Sounds | `paintbrush.fill` | `card.sounds.heading` | `.souls` |

This is the **first commit at which `swift build` succeeds**.

---

### WelcomeWizard — new (commit 7, task #47)

**Files (new):**
- `BoBeMacUI/BoBe/Views/Setup/WelcomeWizard.swift`
- `BoBeMacUI/BoBe/Views/Setup/WelcomeWizardSteps.swift`
- `BoBeMacUI/BoBe/App/SetupWindowManager.swift`

**State:** UserDefaults flag `bobe.onboarding_completed: Bool`. App launches into wizard if false; into overlay if true.

**Steps (one-shot, no back-tracking once finished):**

1. **Welcome** — single card with brand mark + 2-3 lines explaining BoBe (private local-first companion that watches your screen optionally, takes notes in `~/.bobe/memory.md`, learns goals over time). "Let's get set up" → Continue.
2. **Copilot CLI check** — invokes `Process` with `which copilot`. Outcomes:
   - **Found** → green checkmark, "GitHub Copilot CLI found. BoBe will use it as the engine." → Continue.
   - **Missing** → warning, "BoBe needs the GitHub Copilot CLI." Two buttons: "Install instructions" (opens browser to `https://docs.github.com/en/copilot/cli`) + "Retry check".
   - "Continue anyway" hidden secondary button (escape hatch for non-PATH installs). One-time skip.
3. **Permissions** — explain screen recording. Calls `CGRequestScreenCaptureAccess()` if not granted.
   - **Granted** → green checkmark.
   - **Denied** → "Open System Settings" button (opens `x-apple.systempreferences:com.apple.preference.security?Privacy_ScreenCapture`). Skip allowed.
4. **Done** — one card "BoBe is ready." → "Open BoBe" → sets `bobe.onboarding_completed = true`, posts `.bobeWelcomeCompleted`, AppDelegate observer closes wizard window and shows overlay.

**Window:** `NSWindow` 540×640 borderless-titled, centered, hosted via `NSHostingController`.

**`AppDelegate.startApp()` updates:**

```swift
if UserDefaults.standard.bool(forKey: "bobe.onboarding_completed") {
    self.showOverlay()
    self.store.connect()
} else {
    self.showWelcomeWizard()  // store.connect() defers until wizard completes
}
```

---

### i18n purge (commit 8, task #43)

**Files:** 9 × `BoBeMacUI/BoBe/Resources/i18n/<locale>.lproj/UI.strings`

#### Drop dead namespaces (each row × 9 locales)

| Namespace | Reason |
|---|---|
| `setup.*` (~64 keys) | Old wizard deleted; replaced by smaller welcome wizard set |
| `settings.ai_model.*` (~36) | AIModelPanel deleted |
| `settings.tools.*` (~10) | ToolsPanel deleted |
| `settings.goal_worker.*` (~10) | GoalWorkerPanel deleted |
| `app.setup_incomplete.*` (4) | Setup incomplete dialog gone |
| `app.backend_error.*` (2) | resolveStartupRoute deleted (verify via grep before purge) |
| `settings.category.{ai_model,tools,goal_worker}` (3) | Sidebar entries deleted |
| `settings.memories.{category,type,filter,toggle,delete,badge}.*` (~16) | Single-doc rebuild |
| `settings.goals.priority.{high,medium,low,...}` (6) | Priority is Int 0-5 now |
| `settings.goals.source.*` (2) | GoalSource enum gone |
| `settings.goals.toggle.enable_accessibility` (1) | Goal.enabled gone |
| `settings.behavior.{memory,tools}.*` (~7) | Sections deleted |
| `settings.behavior.conversation.generate_summaries` (1) | Feature gone |
| `settings.advanced.{similarity,learning,projects}.*` (~15) | Sections deleted |
| `settings.privacy.danger.error.memory_format` (1) | Replaced by singular |
| `settings.window.overview.card.can_do.*` (2) | 5th card removed |

#### Add new keys

| Key | Used by |
|---|---|
| `settings.shared.restart_banner.message` | Behavior + Advanced |
| `settings.shared.restart_banner.dismiss` | same |
| `settings.goals.priority.label_format` | "Priority %d" picker labels |
| `settings.goals.priority.range_hint` | "0 = lowest, 5 = highest" |
| `settings.goals.section.summary` | "Summary" |
| `settings.goals.section.why_it_matters` | "Why It Matters" |
| `settings.goals.section.notes` | "Notes" |
| `settings.goals.section.how_working_on_it` | "How They're Working On It" |
| `settings.goals.section.patterns_observed` | "Patterns Observed" |
| `settings.goals.section.attitude_feelings` | "Attitude & Feelings" |
| `settings.goals.section.open_questions` | "Open Questions" |
| `settings.goals.section.ai_curated_badge` | "AI-curated" |
| `settings.goals.section.ai_empty` | "BoBe hasn't filled this in yet." |
| `settings.goals.action.resume` | Goal action button |
| `settings.goals.action.reactivate` | same |
| `settings.goals.action.restore` | same |
| `settings.goals.filter.show_archived` | "Show archived" toggle |
| `settings.memory.title` | "Memory" |
| `settings.memory.description` | "BoBe's living narrative. Pruned automatically each night." |
| `settings.memory.action.reload` | "Reload" |
| `settings.memory.action.reset_default` | "Reset to default" |
| `settings.memory.bytes_format` | "%@ / %@" (e.g. "12.4 KB / 50 KB") |
| `settings.memory.consolidation_hint` | "BoBe pruned older entries while you were editing." |
| `settings.privacy.danger.error.memory` | "memory" (singular error item) |
| `setup.welcome.{title,body,continue}` | Welcome wizard step 1 |
| `setup.copilot.{title,checking,found,missing,install_link,retry,continue_anyway}` | step 2 |
| `setup.permissions.{title,body,grant,open_settings,skip}` | step 3 |
| `setup.done.{title,body,launch}` | step 4 |

**Net delta:** ~136 dead × 9 = 1224 lines deleted; ~21 new × 9 = 189 lines added; net **-1035 lines** across 9 files. Translations for non-en locales: machine-translate per existing convention; English placeholders acceptable for the welcome strings if MT churn is excessive.

---

## Commit plan

8 focused commits, each leaves the build progressively healthier:

| # | Title | Task | Files | Build |
|---|---|------|-------|-------|
| 1 | `swiftui(behavior): rebuild BehaviorPanel for 9-field DTO` | #42 | BehaviorPanel.swift | broken |
| 2 | `swiftui(advanced): rebuild AdvancedPanel + add RestartRequiredBanner` | #48 | AdvancedPanel.swift + new RestartRequiredBanner.swift | broken |
| 3 | `swiftui(goals): rebuild GoalsEditor for rich-section daemon shape` | #46 | GoalsEditor.swift + minor SharedControls.swift extension | broken |
| 4 | `swiftui(memory): single-doc MemoriesEditor over GET/PUT /memory` | #45 | MemoriesEditor.swift | broken |
| 5 | `swiftui(privacy): split off PrivacyPanel, drop GoalWorkerPanel, fix memory reset` | #49 | rename PrivacyGoalWorkerPanels.swift → PrivacyPanel.swift | broken |
| 6 | `swiftui(settings): trim sidebar + overview cards + locale rerender` | #44 | SettingsWindow.swift only | **green** |
| 7 | `swiftui(welcome): thin first-launch wizard with Copilot CLI presence check` | #47 | new Views/Setup/* + SetupWindowManager.swift + BoBeApp.swift wiring | green |
| 8 | `swiftui(i18n): purge dead namespaces, add new wizard + section keys` | #43 | 9× UI.strings | green |

---

## Acceptance criteria (per pane)

### BehaviorPanel
- [ ] Loads existing settings via `client.getSettings()` and renders the 6 fields.
- [ ] Toggling capture sends a PATCH and the toggle stays in the new state on reload.
- [ ] Adding a check-in time pill triggers a PATCH; the pill survives reload.
- [ ] DebouncedNumberInput honors range bounds (test: try 0 — should clamp to 1; try 700 — should clamp to 600).
- [ ] Restart banner appears when any of `[checkin_enabled, checkin_times, checkin_jitter_minutes]` is touched.
- [ ] Restart banner is dismissible; dismissing doesn't lose the underlying state.
- [ ] Capture permission warning appears iff `CGPreflightScreenCaptureAccess() == false`.

### AdvancedPanel
- [ ] Loads + renders 3 sections (goals, conversation, MCP).
- [ ] `goalCheckIntervalSeconds` accepts `60...7200`; PATCH round-trips.
- [ ] Toggling `mcpEnabled` shows the restart banner.

### GoalsEditor
- [ ] List shows non-archived goals by default.
- [ ] "Show archived" toggle adds archived goals to the list.
- [ ] Status filter `[nil, active, paused, completed, archived]` filters list.
- [ ] Inline create form requires title; priority defaults to 2.
- [ ] Detail pane shows all 11 fields; 4 agent-only sections render with "AI-curated" badge and read-only.
- [ ] Empty agent-only sections show muted "BoBe hasn't filled this in yet." stub.
- [ ] Status picker supports `archived` selection (direct PATCH to archived works).
- [ ] Action buttons match status: active → Pause/Complete/Archive; paused → Resume/Complete/Archive; completed → Reactivate; archived → Restore.
- [ ] Save sends `GoalUpdateRequest` with the 6 user-editable fields only.
- [ ] Delete confirm flow works (`SettingsEditorActionRow`).
- [ ] Priority dot color maps correctly (0-1 muted, 2 secondary, 3 tertiary, 4-5 primary).

### MemoriesEditor
- [ ] Loads body via `getMemory()`; populates `text`, `savedText`, `bytes`.
- [ ] Editing changes `text`; isDirty derives from `text != savedText`.
- [ ] Save calls `updateMemory(text)`; success updates `savedText` and `bytes`.
- [ ] Reload re-GETs; warns if unsaved changes (confirmation).
- [ ] Reset to default replaces editor body with skeleton; doesn't auto-save.
- [ ] Byte gauge fills proportionally; caps at 1.0.
- [ ] Consolidation hint shows iff response `bytes` deviates from sent length by >200 bytes.

### PrivacyPanel
- [ ] DELETE-typing flow works.
- [ ] Goals/souls/userProfiles delete loops succeed.
- [ ] Memory reset writes `defaultMemoryBody` (single PUT).
- [ ] MCP reset succeeds.
- [ ] Errors surface in red; partial success surfaces remaining errors.

### SettingsWindow
- [ ] Sidebar shows 8 categories in 4 groups.
- [ ] Overview shows 4 cards in 2×2 grid.
- [ ] Locale switch in tray menu re-renders the entire settings window (verify via German/Greek/Japanese).
- [ ] First commit at which `swift build` exits with no errors.

### WelcomeWizard
- [ ] Fresh UserDefaults: launches into wizard, not overlay.
- [ ] Copilot CLI presence check correctly detects `which copilot` (test: temporarily move binary out of PATH; wizard should report missing).
- [ ] Retry check actually re-runs the probe.
- [ ] Continue-anyway escape hatch advances past the check.
- [ ] Permissions step calls `CGRequestScreenCaptureAccess()` only if not yet granted.
- [ ] Open System Settings button opens the right pane.
- [ ] Done step sets `bobe.onboarding_completed = true`; on next launch the wizard does NOT show.
- [ ] AppDelegate connection happens after wizard close, not during.

### i18n purge
- [ ] L10n key drift check (see Verification) returns empty.
- [ ] Each locale `.strings` file passes `plutil -lint` (or equivalent strings-syntax check).

---

## Verification

### After each commit
- `swift build` from `BoBeMacUI/` — must succeed by commit 6 onward.
- SwiftLint: `swiftlint lint --strict` per `BoBeMacUI/.swiftlint.yml`.

### After commit 8

#### L10n key drift check

```bash
rg -No 'L10n\.tr\("([^"]+)' BoBeMacUI/BoBe --replace '$1' | sort -u > /tmp/used.txt
rg -No '^"([^"]+)"' BoBeMacUI/BoBe/Resources/i18n/en.lproj/UI.strings --replace '$1' | sort -u > /tmp/en.txt
comm -23 /tmp/used.txt /tmp/en.txt   # expected: empty (every used key has a translation)
```

#### Cross-locale parity check

```bash
for loc in BoBeMacUI/BoBe/Resources/i18n/*.lproj; do
  rg -c '^"' "$loc/UI.strings"
done | sort -u   # all 9 locales should have identical key counts
```

### Manual smoke tests (live daemon via `just run`)

1. **Tab through all 8 settings categories** — each pane renders without errors.
2. **Welcome wizard** on a fresh `~/Library/Preferences/com.bobe.app.plist`:
   ```bash
   defaults delete com.bobe.app bobe.onboarding_completed 2>/dev/null
   just run
   ```
   Runs through 4 steps; setting `bobe.onboarding_completed = true` skips it next launch.
3. **Souls / UserProfiles** — list, create, edit, save, enable/disable, delete (defaults).
4. **Goals** — create with priority 4; cycle status active → paused → archived → active; complete a goal; archive a goal; un-archive via picker; verify "Show archived" toggle; delete a goal with confirm.
5. **Memories** — load body; edit a section; save; verify byte gauge progresses; click Reset to default; verify text replaces with skeleton; save; reload to confirm round-trip.
6. **MCP** — load existing mcp.json; validate; save; reset.
7. **Appearance** — switch themes; overlay reflects.
8. **Behavior** — toggle capture; change `captureIntervalSeconds` (debounce); add a check-in time and verify restart banner appears (saving any of `checkin_*` triggers it); change `conversationAutoCloseMinutes`.
9. **Advanced** — change `goalCheckIntervalSeconds`; toggle `mcpEnabled` (banner appears); change `conversationInactivityTimeoutSeconds`.
10. **Privacy** — type DELETE, click Delete All Data; verify all goals/souls/profiles/mcp clear; memory.md returns to skeleton.
11. **Tray menu language switch** — pick `el-GR`; settings + overlay re-render in Greek.
12. **Daemon endpoints exercised:**
    - `GET /goals` (with `?include_archived=true`)
    - `POST /goals`
    - `PATCH /goals/{id}`
    - `POST /goals/{id}/{complete,archive}`
    - `DELETE /goals/{id}`
    - `GET /memory`
    - `PUT /memory`
    - `GET /settings`
    - `PATCH /settings`
    - all `/souls/*`
    - all `/user-profiles/*`
    - all `/tools/mcp/config*`
    - `/health`, `/events`

---

## Risks & open notes

- **Restart banner shadow set is a UI-side workaround for daemon imprecision.** Once daemon's `CheckinScheduler` and SDK MCP map are reactive to `ConfigManager` swaps, the shadow set becomes a false positive. Comment the constant with `// TODO(daemon): remove once <subsystem> reads from ConfigManager live`. Acceptable.
- **Memory consolidation race.** PUT vs nightly prune. Daemon resolves silently; UI can only opportunistically detect via byte-delta heuristic. Worst case the user re-saves; acceptable.
- **Welcome wizard `which copilot` heuristic** misses non-PATH installs. "Continue anyway" escape hatch covers it.
- **Locale `.id(localeOverride)` assumes** the empty-string default ↔ system-locale boundary doesn't change without `localeOverride` changing. Holds for now.
- **Soul vs UserProfile copy-on-write asymmetry** — souls have it, user-profiles don't. Document as a daemon TODO; UI works correctly for the asymmetric behavior as-is. Out of scope.
- **MCP `connected/tool_count/tools` will report stubs until task #31 lands.** UI must NOT show "connected/disconnected" pills as truth — surface only enabled/disabled.

---

## Files touched

**Modified:**
- `BoBeMacUI/BoBe/Features/Settings/SettingsWindow.swift`
- `BoBeMacUI/BoBe/Features/Settings/BehaviorPanel.swift`
- `BoBeMacUI/BoBe/Features/Settings/AdvancedPanel.swift`
- `BoBeMacUI/BoBe/Features/Settings/GoalsEditor.swift`
- `BoBeMacUI/BoBe/Features/Settings/MemoriesEditor.swift`
- `BoBeMacUI/BoBe/Features/Settings/SharedControls.swift` (small `CollapsibleSection.aiCuratedBadge:` extension)
- `BoBeMacUI/BoBe/App/BoBeApp.swift` (wizard branch in `startApp()`)
- 9× `BoBeMacUI/BoBe/Resources/i18n/<locale>.lproj/UI.strings`

**Renamed:**
- `BoBeMacUI/BoBe/Features/Settings/PrivacyGoalWorkerPanels.swift` → `PrivacyPanel.swift`

**New:**
- `BoBeMacUI/BoBe/Features/Settings/RestartRequiredBanner.swift`
- `BoBeMacUI/BoBe/Views/Setup/WelcomeWizard.swift`
- `BoBeMacUI/BoBe/Views/Setup/WelcomeWizardSteps.swift`
- `BoBeMacUI/BoBe/App/SetupWindowManager.swift`

**Untouched (verified working):**
- `Stores/*` — BobeStore, ThemeStore, ToolExecutionController
- `Services/*` — DaemonClient, DaemonTypes, BackendService
- `Models/*` — SettingsTypes, EntityTypes, APITypes, BobeTypes
- `Views/Overlay/*` — overlay panel, avatar, eyes, chat, message input, motion
- `Theme/*` — ThemeConfig, ThemeStore
- `Util/Localization.swift`, `Utilities/Bundle+Resources.swift`
- `App/*` (except BoBeApp.swift) — TrayManager, SettingsWindowManager, OverlayWindowManager, OverlayPanel, UpdaterManager, MoveToApplications
- `Features/Settings/*` (the working subset) — SoulsEditor, UserProfilesEditor, MCPServersPanel, AppearancePanel, SettingsEditorFramework, ThemedControls, ThemedControlsPreviews, CodeEditor

---

## Cross-references

- **Canonical plan file:** `/Users/john/.claude/plans/iridescent-percolating-octopus.md` (this file mirrors that)
- **Daemon API spec dump (deep dive):** session 2026-05-09, available via Explore agent if needed
- **Memory:** `/Users/john/.claude/projects/-Users-john-Repos-bobrust/memory/project_copilot_workers_initiative.md` — initiative summary + post-cleanup state
- **Current foundation commit:** `9c1d6ae wip(swiftui): foundation rewrite for new daemon API contract (build broken)`
- **Daemon-cleanup commits this branch (final 14):** `552dce6, 6228d40, a1e9d84, 169562b, a14ef01, a354266, 6bc64c8, 877369e, 5f133f1, 8f88439, b7e60c3, 479d6f1, a5a6dca, 1ba0cca`

---

*Generated 2026-05-09. Mirror of `/Users/john/.claude/plans/iridescent-percolating-octopus.md`.*
