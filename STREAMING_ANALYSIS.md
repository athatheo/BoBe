# Proactive Streaming Implementation Analysis

## Executive Summary

The proactive streaming implementation is **well-architected with clean separation of concerns**, but shows several areas of complexity around state management and potential redundancy in streaming control flow.

**Key Finding**: The system uses indicator-based state tracking (Idle/Streaming/Thinking) rather than a dedicated "is_streaming" boolean, which works but creates indirect state management.

---

## 1. PROACTIVE MESSAGE GENERATION - TRIGGER & CONTROL FLOW

### Where Generation is Triggered (4 Trigger Points)

**File**: `src/runtime/triggers/`

Multiple triggers can initiate `ProactiveGenerator::generate_proactive_response()`:

#### a) **Capture Trigger** (`capture_trigger.rs:108-111`)
```rust
if decision == Decision::Engage {
    self.generator
        .generate_proactive_response(cfg.conversation.auto_close_minutes as i64, None)
        .await;
}
```
- Triggered by screenshot + ML classification
- Uses `Decision::Engage` from DecisionEngine
- Can be cooldown-blocked before reaching this point

#### b) **Check-in Trigger** (`checkin_trigger.rs:77`)
```rust
.generate_proactive_response(
    cfg.conversation.auto_close_minutes as i64,
    Some(summary),
)
.await;
```
- Time-based periodic check-ins (no decision engine)
- **Automatic engagement** - no decision query

#### c) **Goal Trigger** (`goal_trigger.rs:95`)
```rust
.generate_proactive_response(cfg.conversation.auto_close_minutes as i64, None)
.await;
```
- Goal-based proactive outreach
- Uses DecisionEngine with `TriggerType::Goal`

#### d) **Agent Job Trigger** (`agent_job_trigger.rs:248`)
- For scheduled/background job completions

### Control Flow Architecture

**Decision Engine** (`src/runtime/decision_engine.rs`):
- Routes decisions based on `TriggerType`
- Returns: `Decision::Engage`, `Decision::Idle`, or `Decision::NeedMoreInfo`
- Uses LLM prompting for Capture and Goal triggers
- Check-in always returns `Decision::Engage`

**State Machine**:
```
Trigger Fire → Decision Engine → Decision::Engage → generate_proactive_response()
                                                    ├─ begin_proactive_stream() [marks conversation state]
                                                    ├─ Streaming Loop (llm.stream)
                                                    ├─ finalize_proactive_stream() [persists]
                                                    └─ set_indicator(Idle)
```

---

## 2. STREAMING STATE MANAGEMENT - RUNTIME SOURCES OF TRUTH

### Backend (Rust) - `src/runtime/` & `src/services/`

#### Primary State: `ConversationService::streaming_assistant_turns`
**File**: `src/services/conversation_service.rs:25`

```rust
streaming_assistant_turns: DashMap<ConversationId, StreamingAssistantTurn>
```

**Structure**:
```rust
struct StreamingAssistantTurn {
    turn: ConversationTurn,
    persisted: bool,
}
```

**Operations**:
1. **`begin_proactive_stream()`** (line 102)
   - Creates/resets streaming turn in memory
   - Handles preferred conversation or creates new one
   - Returns early if conversation closed
   - **Note**: Replaces any existing streaming turn (logged as warning)

2. **`push_proactive_stream_delta()`** (line 140)
   - Accumulates text in memory while streaming
   - Updates `draft.turn.append_content(delta)`
   - No persistence yet

3. **`finalize_proactive_stream()`** (line 150)
   - Removes from memory map
   - Persists to database (either insert or update)
   - Returns `Option<ConversationTurn>` - None if already removed

4. **`discard_proactive_stream()`** (line 174)
   - Removes without persisting
   - Used when response is empty

**State Consistency Issues**:
- Turn is held in memory in `streaming_assistant_turns` during streaming
- If streaming is interrupted, must be explicitly discarded or finalized
- `sync_streaming_assistant_turn_locked()` syncs partial turns if new message arrives during streaming
- **Potential Race**: Two simultaneous streaming attempts on same conversation trigger warning but replace (line 131-134)

#### Secondary State: EventQueue Indicator
**File**: `src/util/sse/event_queue.rs:52`

```rust
pub(crate) fn set_indicator(&self, indicator: IndicatorType) {
    *lock_or_recover(&self.current_indicator, "event_queue.current_indicator") = indicator;
    self.push(indicator_event(indicator, None));
}
```

**Indicator Types**:
```rust
enum IndicatorType {
    Idle,
    Streaming,
    Thinking,
    ScreenCapture,
    ToolCalling,
}
```

**Where Set**:
- `proactive_generator.rs:128` → `Streaming` before response generation
- `proactive_generator.rs:247` → `Idle` after completion
- `message_handler.rs:111` → `Streaming` for user responses
- `message_handler.rs:213` → `Idle` after completion

### Frontend (Swift) - Streaming Message Buffering

**File**: `desktopMac/BoBe/Stores/BobeStore.swift`

#### Local Buffering (Private State):
```swift
private var streamingMessage = ""
private var streamingMessageId: String?
private var textDeltaFlushTask: Task<Void, Never>?
```

**Flow**:
1. `handleTextDelta()` accumulates text in `streamingMessage`
2. Throttled flushing at ~150ms intervals (`textDeltaFlushTask`)
3. `flushStreamingToUI()` updates UI message or creates new message
4. `finalizeStreamingMessage()` persists and clears buffers

#### Indicator State in Context:
```swift
var activeIndicator: IndicatorType? { 
    // Derived from indicator events
}
var thinking: Bool
var speaking: Bool  // Set when indicator == .streaming && hasVisibleText
```

**State Tracking**:
- No dedicated "isStreaming" boolean
- Derived from `activeIndicator` and content presence
- Message `isStreaming` property set true during accumulation, false on `end_of_turn`

---

## 3. FRONTEND MESSAGE RECEPTION & DISPLAY

### SSE Connection & Event Loop

**File**: `desktopMac/BoBe/Services/DaemonClient.swift:62-111`

```swift
func connectSSE(...) {
    eventHandler = onEvent
    connectionHandler = onConnectionChange
    reconnectAttempts = 0
    startSSE()
}

private func runSSELoop() async {
    // Exponential backoff reconnection: up to 10 attempts, max 30s delay
    // Tracks: reconnectAttempts, isReconnecting flag
    // Calls eventHandler?(bundle) for each event
}
```

**SSE Protocol**:
- Connects to `GET /events` endpoint
- Receives newline-delimited JSON: `data: {StreamBundle JSON}`
- Automatic reconnection with exponential backoff (2^n seconds, capped at 30s)

### Event Processing Pipeline

**File**: `desktopMac/BoBe/Stores/BobeStore.swift:305-378`

1. **Bundle Receives**:
   ```swift
   func connect() {
       await self.client.connectSSE(
           onEvent: { [weak self] bundle in
               await MainActor.run { self?.processBundle(bundle) }
           },
           onConnectionChange: { [weak self] connected in
               // Handle reconnection/disconnection
           }
       )
   }
   ```

2. **Event Type Dispatch**:
   ```swift
   switch bundle.type {
   case .indicator: handleIndicator(payload)
   case .textDelta: handleTextDelta(payload, messageId:)
   case .endOfTurn: finalizeStreamingMessage()
   case .toolCall, .toolCallStart, .toolCallComplete: handleToolCall(payload)
   case .conversationClosed: handleConversationClosed(payload)
   case .error: handleError(payload)
   }
   ```

3. **Text Delta Handling**:
   - Accumulates in local `streamingMessage` buffer
   - Schedules throttled flush task if not already scheduled
   - On `payload.done`, immediately flushes and finalizes

4. **UI Message Updates**:
   - Checks if message exists with `messageId` and `isStreaming == true`
   - Updates content, replaces `isStreaming: false`
   - Or creates new message if first delta for ID

### Tool Execution Display

**File**: `desktopMac/BoBe/Stores/ToolExecutionController.swift:19-68`

```swift
func process(_ payload: AnyCodablePayload) {
    if let start = try? payload.decode(as: ToolCallStartPayload.self) {
        handleStart(start)  // Appends running execution
    }
    if let complete = try? payload.decode(as: ToolCallCompletePayload.self) {
        handleComplete(complete)  // Updates status, schedules cleanup after 2s
    }
}
```

- Tool executions stored in `context.toolExecutions: [ToolExecution]`
- Auto-cleanup 2 seconds after completion
- Status: `.running` → `.success`/`.error`

---

## 4. DUPLICATE STREAMING LOGIC & STATE TRACKING ISSUES

### Issue #1: Dual State Management - Indicator vs. Conversation State

**Problem**: "Is currently streaming?" answered by two different sources:

1. **EventQueue Indicator** (reactive):
   ```rust
   event_queue.set_indicator(IndicatorType::Streaming)
   event_queue.set_indicator(IndicatorType::Idle)
   ```

2. **ConversationService State** (active):
   ```rust
   streaming_assistant_turns.contains_key(conversation_id)  // implies streaming
   ```

**Issue**: These can diverge if:
- Streaming finalized but indicator not yet set to Idle
- Indicator set to Idle but streaming turn still in memory (race condition)
- New message arrives during streaming → calls `sync_streaming_assistant_turn_locked()`

**Location**: 
- `proactive_generator.rs:128, 247` sets indicator
- `conversation_service.rs:25, 102, 150, 174` manages turn map
- No atomic update between the two

### Issue #2: Multiple "Start Streaming" Calls Can Race

**Code**: `conversation_service.rs:123-135`

```rust
let replaced = self.streaming_assistant_turns.insert(
    conversation_id,
    StreamingAssistantTurn { turn, persisted: false }
);
if replaced.is_some() {
    warn!("conversation.streaming_turn_replaced");
}
```

**Problem**:
- If two proactive generations trigger simultaneously for same conversation
- First turn gets replaced without warning in DashMap (concurrent insert)
- Warning only fires after replacement
- No prevention mechanism - could lose partial streaming text

**Example Scenario**:
1. Capture trigger starts streaming at T0
2. Check-in trigger also starts at T0.1ms
3. Second one replaces first turn silently
4. First stream's text is lost

### Issue #3: Streaming State in Frontend Not Fully Atomic

**File**: `BobeStore.swift:75-79, 383-390`

```swift
private var streamingMessage = ""
private var streamingMessageId: String?
private var textDeltaFlushTask: Task<Void, Never>?
```

**Race Conditions**:
- If two message IDs arrive (e.g., simultaneous proactive + user response)
- `streamingMessageId` might not match current ID being processed
- Text Delta Handler clears buffer (line 384) without synchronization guarantee
- Multiple `textDeltaFlushTask` cancellations could cause UI glitches

**Example**:
```
T0: Message A starts, streamingMessageId = "msg_A", text = "Hello"
T1: Message B starts, streamingMessageId = "msg_B", text = "" (reset!)
T2: Flush from Message A scheduled - but ID is now B, content is lost
```

### Issue #4: Indicator Payload Processing Not Synchronized with Content

**File**: `BobeStore.swift:340-378`

```swift
private func handleIndicator(_ payload: IndicatorPayload) {
    let indicator = payload.indicator
    
    if indicator == .idle, !self.context.currentMessage.isEmpty {
        self.finalizeStreamingMessage()  // But text might still be buffered!
    }
```

**Race**:
- Indicator says Idle
- But `streamingMessage` buffer still has pending text
- `textDeltaFlushTask` might fire after finalization
- Orphaned text never reaches UI

---

## 5. STREAMING ENDPOINT & CONNECTION HANDLING

### Backend SSE Endpoint

**File**: `src/api/handlers/events.rs:12-62`

```rust
pub(crate) async fn stream_events(
    State(state): State<Arc<AppState>>,
) -> Sse<impl Stream<Item = Result<Event, Infallible>>> {
    let conn_id = connection_manager.connect().await;  // Single consumer model
    
    tokio::spawn(async move {
        loop {
            if !cm.is_active_connection(&conn_id_inner).await {
                break;  // Replaced by new connection
            }
            let event = tokio::time::timeout(Duration::from_secs(1), queue.pop()).await;
            if let Ok(bundle) = event {
                tx.send(Ok(sse_event)).await?
            }
        }
    });
}
```

**Architecture**:
- Single consumer model (ConnectionManager enforces this)
- On new connection, old one is replaced
- Channel buffering: 64 events in `tokio::sync::mpsc::channel`
- Timeouts: 1 second per pop, keeps connection alive

**Connection Manager State** (`src/util/sse/connection_manager.rs:14-26`):
```rust
struct ConnectionState {
    connected: bool,
    connection_id: Option<String>,
    generation: u64,
    disconnect_time: Option<DateTime<Utc>>,
    current_indicator: IndicatorType,  // Tracks last indicator seen
}
```

### Frontend SSE Loop

**File**: `DaemonClient.swift:62-130`

```swift
private func runSSELoop() async {
    var request = URLRequest(url: url)
    request.setValue("text/event-stream", forHTTPHeaderField: "Accept")
    
    do {
        let (bytes, response) = try await session.bytes(for: request)
        for try await line in bytes.lines {
            guard line.hasPrefix("data: ") else { continue }
            let jsonStr = String(line.dropFirst(6))
            let bundle = try decoder.decode(StreamBundle.self, from: data)
            self.eventHandler?(bundle)
        }
    } catch {
        await self.handleSSEDisconnect()
    }
}

private func handleSSEDisconnect() async {
    guard !self.isReconnecting else { return }
    self.isReconnecting = true
    defer { isReconnecting = false }
    
    // Exponential backoff: 2^(attempt-1) seconds, capped at 30s
    let delay = min(pow(2.0, Double(reconnectAttempts - 1)), 30.0)
    try? await Task.sleep(for: .seconds(delay))
    if !Task.isCancelled {
        self.startSSE()
    }
}
```

**Key Properties**:
- Handles SSE line-by-line parsing
- Reconnection: max 10 attempts before giving up
- Backoff: 1s, 2s, 4s, 8s, 16s, 30s, 30s, 30s, 30s, 30s
- Max total wait time: ~180s (3 min) before final failure

---

## 6. CODE METRICS & DEDUPLICATION ANALYSIS

### File Sizes:
- `proactive_generator.rs`: 387 lines (generates proactive responses)
- `response_streamer.rs`: 286 lines (shared streaming infrastructure)
- `message_handler.rs`: 428 lines (user message responses)

**Total streaming code**: 1,101 lines in these three files alone

### Duplicate Streaming Functions

**In `response_streamer.rs`**:
```rust
pub async fn stream_response() → calls → stream_response_with_text_observer()
pub async fn stream_llm_response() → calls → stream_llm_response_with_text_observer()
```

- `stream_response_with_text_observer()` (line 45): Takes StreamItem (text + tool notifications)
- `stream_llm_response_with_text_observer()` (line 123): Takes only StreamChunk (text only)
- Both share ~95% of logic
- Could be unified with conditional type handling

**Duplication Detail**:
- Lines 66-93: handling `StreamItem::Chunk` (tools stream)
- Lines 144-165: handling `StreamChunk` (llm stream)
- Identical chunk handling logic at lines 184-205
- Shared error/done handling (lines 276-286)

### Where Streaming is Called

1. **Proactive Generation** (`proactive_generator.rs:200, 218`):
   ```rust
   stream_response_with_text_observer(tool_stream, ...)  // With tools
   stream_llm_response_with_text_observer(stream, ...)    // Without tools
   ```

2. **User Message Response** (`message_handler.rs:200, 209`):
   ```rust
   stream_response(stream, ...)           // With tools
   stream_llm_response(stream, ...)       // Without tools
   ```

**Observation**: Identical conditional logic (if tools available, use tool stream, else llm stream) duplicated in two places

### No Dead Code Found
- No `#[allow(dead_code)]` annotations
- All exported `pub(crate)` functions are used
- No TODO/FIXME comments related to streaming

---

## 7. KEY ARCHITECTURAL CONCERNS

### A. State Machine Clarity
**Issue**: Multiple state sources make it hard to track true "streaming" state:
- EventQueue indicator (what frontend sees)
- ConversationService memory map (what backend knows)
- Frontend buffering state (private to BobeStore)
- No unified "is streaming" query method

**Recommendation**: Add a getter that unifies state:
```rust
impl ConversationService {
    pub async fn is_streaming(&self, conversation_id: ConversationId) -> bool {
        self.streaming_assistant_turns.contains_key(&conversation_id)
    }
}
```

### B. Simultaneous Streaming Prevention
**Issue**: Two triggers can start streaming same conversation with only a warning

**Recommendation**: 
```rust
pub async fn begin_proactive_stream(&self) -> Result<Conversation, AppError> {
    if self.streaming_assistant_turns.contains_key(&conversation_id) {
        return Err(AppError::AlreadyStreaming);
    }
    // ... proceed
}
```

### C. Streaming Text Loss on Reconnect
**Issue**: Frontend clears `streamingMessage` when new message ID arrives
- No guarantee last deltas were flushed before clear
- If flush task scheduled but not yet fired, text is lost

**Recommendation**: Wait for pending flush before clearing:
```swift
private func handleTextDelta(...) {
    if self.streamingMessageId != messageId {
        textDeltaFlushTask?.cancel()
        self.streamingMessage = ""
        self.streamingMessageId = messageId
    }
}
```

### D. Indicator-Content Sync Race
**Issue**: Idle indicator can arrive before all text is flushed

**Recommendation**: Flush before finalizing on Idle indicator:
```swift
if indicator == .idle {
    textDeltaFlushTask?.cancel()  // Cancel pending
    await textDeltaFlushTask?.value  // Wait for inflight
    self.flushStreamingToUI(messageId: streamingMessageId)
}
```

---

## 8. SUMMARY TABLE: STREAMING STATE SOURCES OF TRUTH

| State | Backend Source | Frontend Source | Sync Mechanism |
|-------|---|---|---|
| Is streaming? | `streaming_assistant_turns` DashMap | `activeIndicator == .streaming` | Event queue indicator |
| Current text | `ConversationTurn.content` | `BobeStore.streamingMessage` | Text delta events |
| Message ID | None (implicit) | `streamingMessageId` | Bundled with deltas |
| Completion | `finalize_proactive_stream()` | `end_of_turn` event | SSE event |
| UI Display | N/A | `BobeStore.context.messages` | Flushing logic |

---

## CONCLUSION

**Strengths**:
✅ Clean separation: triggers → decision → generation → streaming → persistence
✅ Well-organized event propagation through EventQueue and SSE
✅ Proper async handling with reconnection logic
✅ Tool execution notifications alongside text streaming

**Weaknesses**:
⚠️ Dual state sources for "is streaming" can diverge
⚠️ No prevention of concurrent streaming on same conversation
⚠️ Frontend text buffering lacks synchronization guarantees
⚠️ Redundant streaming functions (stream_response + stream_llm_response)
⚠️ Race between indicator completion and text flushing

**Risk Level**: Medium - System works but has subtle race conditions that could cause text loss or state inconsistency under high load or rapid trigger sequences.

