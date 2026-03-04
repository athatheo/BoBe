# Current Issues

## Planned: Conversation History Compaction

**Problem:** `build_conversation_history()` fetches up to 20 turns with no token awareness. On Ollama's default 4K context, this overflows quickly.

**Current safety net:** `clamp_max_tokens` reduces the response budget when the prompt is large, but does not trim history itself.

**Planned solution:** Proactive compaction — when history tokens exceed **80%** of the history budget, compact the **oldest 60%** of turns into a ~200-token summary using the existing `ConversationSummaryPrompt`. Store the summary on `Conversation.summary`. This creates headroom for many turns before the next compaction cycle.

**What it replaces:**

- The warning-only `CONTEXT_TOKEN_WARN_THRESHOLD` (4K) in `context_assembler.rs`
- The unused `DecisionConfig.max_response_tokens` field
- The naive 20-turn fetch with no size awareness

**Infrastructure to reuse:**

- `ConversationSummaryPrompt` (already generates summaries)
- `Conversation.summary` field (already persisted)
- `generate_summary()` in `proactive_generator.rs`

**Model reference:**

| Model | Context Window | Max Input | Ollama Default |
|-------|---------------|-----------|----------------|
| gpt-5-mini/nano/5.2 | 400K | 272K | — |
| Ollama (any) | Varies | — | **4,096** |
| qwen3:14b | 40K native | — | 4,096 |

---

## Active UX/Runtime Issues (reported 2026-03-04)

1. **Settings window close crash (critical)**
   - Repro: open Settings, click window close (**X**).
   - Result: app crashes with `EXC_BAD_ACCESS` / bad pointer dereference in AppKit runloop teardown.

2. **Settings navbar/toggle UX regressions**
   - Duplicate sidebar-toggle buttons visible.
   - Toggle placement/behavior feels inconsistent when sidebar is collapsed.

3. **Settings sidebar theming mismatches**
   - Sidebar text/icons/selected-row styling not consistently following BoBe theme.
   - Native macOS blue selection highlight clashes with custom highlight styling (double-highlight effect).

4. **AI Model dropdown control regressions**
   - Two chevron indicators shown at once.
   - Dropdown interaction reliability issues (reported as non-clickable in panel).

5. **Goal Worker panel database error**
   - Reported error: `DATABASE_ERROR` / SQLite `(code: 14) unable to open database file`.

6. **Memories panel database error**
   - Reported 503 / `DATABASE_ERROR` when navigating to Memories.

### Backend API health snapshot (manual curl smoke check)

- Verified **GET 200**: `/settings`, `/goals`, `/memories`, `/souls`, `/user-profiles`, `/goal-plans/status`
- Verified **read/write**:
  - `POST/PATCH/DELETE /memories` (201/200/204)
  - `POST/PATCH/DELETE /goals` (201/200/204)
