# Proactive Streaming Implementation - Exploration Index

## 📋 Overview

This exploration analyzed the proactive message generation and streaming implementation in the BoBe codebase, focusing on:
1. How proactive messages are triggered and controlled
2. Where streaming state is managed
3. How the frontend receives and displays streaming messages
4. Duplicate streaming logic and state tracking issues

**Key Finding**: Well-architected system with clean separation of concerns, but several race conditions and state management issues identified.

---

## 📁 Generated Documents

### 1. **STREAMING_ANALYSIS.md** (Detailed Technical Report)
- **Size**: ~40KB comprehensive analysis
- **Contains**:
  - Executive summary of architecture
  - Trigger points (4 entry mechanisms)
  - Backend streaming state management (DashMap + Indicator)
  - Frontend message buffering (BobeStore)
  - SSE connection handling
  - Code metrics and deduplication analysis
  - Detailed issue descriptions with code locations
  - Architectural concerns and recommendations

- **Key Sections**:
  - Section 1: Proactive message trigger control flow
  - Section 2: Streaming state management (dual sources of truth)
  - Section 3: Frontend reception & display pipeline
  - Section 4: Duplicate streaming logic analysis
  - Section 5: SSE endpoint & connection handling
  - Section 6: Code metrics
  - Section 7: Architectural concerns
  - Section 8: Summary table

### 2. **STREAMING_ISSUES_DIAGRAM.txt** (Visual Issue Breakdown)
- **Contains**: ASCII diagrams of all 4 critical issues
- **Each issue includes**:
  - Timeline/sequence showing the race condition
  - File locations and line numbers
  - Code snippets
  - Current behavior vs. proposed fixes
  - Lost text scenario trace

---

## 🎯 Issues Summary

| # | Issue | Severity | Location | Type |
|---|-------|----------|----------|------|
| 1 | Concurrent streaming race (same conversation) | HIGH | `conversation_service.rs:123-135` | Race condition |
| 2 | Indicator-text sync race | MEDIUM | `BobeStore.swift:340-378` | Race condition |
| 3 | Dual source of truth (state divergence) | MEDIUM | Multiple files | Architecture |
| 4 | Duplicate streaming functions | LOW | `response_streamer.rs` | Code quality |

---

## 🔧 Quick Reference Guide

### How to Find Specific Components

#### **Proactive Message Triggers**
- Capture: `src/runtime/triggers/capture_trigger.rs:108`
- Check-in: `src/runtime/triggers/checkin_trigger.rs:77`
- Goal: `src/runtime/triggers/goal_trigger.rs:95`
- Agent Job: `src/runtime/triggers/agent_job_trigger.rs:248`

#### **Backend Streaming State**
- State Map: `src/services/conversation_service.rs:25`
- `begin_proactive_stream()`: line 102
- `push_proactive_stream_delta()`: line 140
- `finalize_proactive_stream()`: line 150
- `discard_proactive_stream()`: line 174

#### **Indicator State**
- EventQueue: `src/util/sse/event_queue.rs:52` (`set_indicator()`)
- Proactive Gen: `src/runtime/proactive_generator.rs:128, 247`
- Message Handler: `src/runtime/message_handler.rs:111, 213`

#### **Frontend SSE**
- DaemonClient: `desktopMac/BoBe/Services/DaemonClient.swift:62-130`
- BobeStore: `desktopMac/BoBe/Stores/BobeStore.swift:75-460`
- ToolController: `desktopMac/BoBe/Stores/ToolExecutionController.swift`

#### **Response Streaming**
- Proactive path: `src/runtime/response_streamer.rs:45-108` (with text observer)
- User response: `src/runtime/response_streamer.rs:123-181` (llm only)
- Tool stream: `src/runtime/response_streamer.rs:35-43` (tools enabled)

---

## 🚀 Recommended Fix Priority

### Priority 1: HIGH (Prevent Data Loss)
**Issue #1: Concurrent Streaming Race**
- File: `src/services/conversation_service.rs`
- Change: Add `contains_key()` check before insert
- Impact: Prevents silent text loss from simultaneous triggers
- Estimated effort: 5 minutes
- Code snippet in STREAMING_ANALYSIS.md Section 7.B

### Priority 2: MEDIUM (Prevent Orphaned Text)
**Issue #2: Indicator-Text Sync Race**
- File: `desktopMac/BoBe/Stores/BobeStore.swift`
- Change: Wait for pending flush task before finalizing on Idle
- Impact: Ensures all text reaches UI before cleanup
- Estimated effort: 15 minutes
- Code snippet in STREAMING_ANALYSIS.md Section 7.D

### Priority 3: MEDIUM (State Clarity)
**Issue #3: Dual Source of Truth**
- File: `src/services/conversation_service.rs`
- Change: Add public `is_streaming()` query method
- Impact: Clarifies state ownership, prevents divergence
- Estimated effort: 10 minutes
- Code snippet in STREAMING_ANALYSIS.md Section 7.A

### Priority 4: LOW (Technical Debt)
**Issue #4: Duplicate Functions**
- File: `src/runtime/response_streamer.rs`
- Change: Merge `stream_response` and `stream_llm_response` with generics
- Impact: Reduces maintenance burden, ~100 LOC savings
- Estimated effort: 45 minutes
- Architecture documented in STREAMING_ISSUES_DIAGRAM.txt

---

## 📊 File Summary

### Backend Files (Rust)
- **proactive_generator.rs** (387 lines)
  - Coordinates decision → generation → streaming → persistence
  - Entry point: `generate_proactive_response()`
  
- **message_handler.rs** (428 lines)
  - Handles user-initiated responses
  - Mirrors proactive_generator flow
  - Shares streaming infrastructure
  
- **response_streamer.rs** (286 lines)
  - Shared streaming utilities
  - Two variants: with tools, without tools (95% duplicate)
  - Produces StreamResult with metrics
  
- **conversation_service.rs** (partial, ~300 lines)
  - Primary streaming state holder
  - DashMap-based turn buffering
  - Lifecycle management with lock
  
- **event_queue.rs** (79 lines)
  - Secondary streaming state (indicator)
  - SSE event buffering
  - Wake notification for consumers

### Frontend Files (Swift)
- **BobeStore.swift** (530+ lines)
  - Main UI state container
  - Text delta buffering & throttling
  - Message lifecycle management
  - Indicator state derivation
  
- **DaemonClient.swift** (200+ lines)
  - SSE connection management
  - Exponential backoff reconnection
  - Event decode & dispatch
  - Max 10 reconnect attempts
  
- **ToolExecutionController.swift** (68 lines)
  - Tool execution UI updates
  - Auto-cleanup after 2s completion

---

## 🔍 State Flow Diagrams

### Trigger to Persistence
```
[Trigger: Capture/Goal/Checkin/Job]
    ↓
[Decision Engine] → Decision::Engage
    ↓
[generate_proactive_response()]
    ├─ event_queue.set_indicator(Streaming)
    ├─ conversation.begin_proactive_stream() [DashMap insert]
    ├─ llm.stream() → push_proactive_stream_delta() [accumulate]
    ├─ conversation.finalize_proactive_stream() [persist, DashMap remove]
    └─ event_queue.set_indicator(Idle)
```

### Frontend Reception
```
[SSE Connection] → DaemonClient.runSSELoop()
    ↓ (each event)
[BobeStore.processBundle()]
    ├─ indicator → handleIndicator() [updates thinking/speaking]
    ├─ textDelta → handleTextDelta() [buffers + throttles]
    ├─ toolCall* → ToolExecutionController.process()
    ├─ endOfTurn → finalizeStreamingMessage()
    └─ conversationClosed → handleConversationClosed()
```

---

## ⚠️ Known Risks

1. **Concurrent Triggers**: Two simultaneous proactive generations on same conversation → text loss
2. **Timing Windows**: Indicator arrives before all text deltas → orphaned buffered text
3. **State Fragmentation**: "Is streaming?" stored in 3 places (DashMap, EventQueue, ConnectionManager)
4. **Code Duplication**: 95% shared code between tool and non-tool streaming paths

---

## 📚 Related Documentation

- **Architecture notes**: See Section 1 of STREAMING_ANALYSIS.md
- **Control flow**: See control flow diagrams in this file and STREAMING_ISSUES_DIAGRAM.txt
- **Detailed race conditions**: See STREAMING_ISSUES_DIAGRAM.txt
- **Code metrics**: See Section 6 of STREAMING_ANALYSIS.md

---

## 🎓 Key Learnings

1. **Single-Consumer SSE Model**: ConnectionManager enforces one active connection, replacing old ones
2. **Indicator as State Proxy**: UI uses indicator events to infer streaming state, not dedicated flag
3. **Throttled Text Flushing**: 150ms interval batching for typing appearance, creates sync challenges
4. **Lifecycle Lock Pattern**: Mutex guards conversation state transitions to prevent splits
5. **Event Queue Buffering**: 64-event capacity with overflow warning, not persisted on disconnect

---

## 🔗 External References

- SSE Spec: Event-stream format in `DaemonClient.swift:90-100`
- Connection Protocol: Single-consumer + reconnection in `connection_manager.rs:75-126`
- Indicator Types: `IndicatorType` enum in `src/util/sse/types.rs`
- StreamBundle Format: JSON serializable event wrapper in `BobeStore.swift:305-338`

---

**Generated**: 2024 Exploration Report
**Report Location**: `/Users/john/Repos/bobrust/`
- STREAMING_ANALYSIS.md (detailed)
- STREAMING_ISSUES_DIAGRAM.txt (visual)
- STREAMING_EXPLORATION_INDEX.md (this file)

