# Current Issues

## Planned: Conversation History Compaction

**Problem:** `build_conversation_history()` accepts a `token_budget` and trims the oldest turns to fit the budget, which prevents overflow. However, it does not compact old turns — trimmed history is simply dropped.

**Current safety nets:**
- `build_conversation_history()` in `message_handler.rs` respects a token budget derived from the context window
- `clamp_max_tokens` reduces the response budget when the prompt is large

**Planned improvement:** Proactive compaction — when history tokens exceed **80%** of the history budget, compact the **oldest 60%** of turns into a ~200-token summary using the existing `ConversationSummaryPrompt`. Store the summary on `Conversation.summary`. This preserves context from early turns that would otherwise be dropped.

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

1. **Settings window close crash (mitigated)**
   - Was: `EXC_BAD_ACCESS` on window close.
   - Mitigation: `windowShouldClose` now uses `orderOut(nil)` instead of closing, preventing the NSWindow dealloc race. Needs further testing.

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
