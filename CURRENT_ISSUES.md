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
