/**
 * Browser Daemon Client
 *
 * Provides a window.bobe-compatible API that talks directly to the Python daemon
 * via HTTP/SSE. Used when running in a browser without Electron.
 */

import type { BobeContext, ChatMessage, ToolExecution, IndicatorType } from '@/types/bobe'
import type {
  StreamBundle,
  IndicatorPayload,
  TextDeltaPayload,
  ToolCallPayload,
  ConversationClosedPayload,
} from '@/types/api'

const DAEMON_URL = 'http://localhost:8766'
const FETCH_TIMEOUT = 10_000

function daemonFetch(url: string, init?: RequestInit): Promise<Response> {
  return fetch(url, { signal: AbortSignal.timeout(FETCH_TIMEOUT), ...init })
}
const MAX_VISIBLE_MESSAGES = 4

type StateUpdateCallback = (state: Partial<BobeContext>) => void

/**
 * Browser-compatible daemon client that mimics window.bobe API
 */
class BrowserDaemonClient {
  private eventSource: EventSource | null = null
  private stateCallbacks: Set<StateUpdateCallback> = new Set()
  private currentState: Partial<BobeContext> = {
    daemonConnected: false,
    capturing: false,
    thinking: false,
    speaking: false,
    lastMessage: null,
    currentMessage: '',
    currentMessageId: null,
    messages: [],
    activeIndicator: null,
    toolExecutions: [],
  }

  // Text delta accumulator (mirrors daemon-client.ts)
  private streamingMessage = ''
  private streamingMessageId: string | null = null

  constructor() {
    this.connectSSE()
  }

  /**
   * Connect to daemon SSE stream
   */
  private connectSSE(): void {
    if (this.eventSource) {
      this.eventSource.close()
    }

    try {
      this.eventSource = new EventSource(`${DAEMON_URL}/events`)

      this.eventSource.onopen = () => {
        console.log('[BrowserDaemonClient] SSE connected')
        // Clear stale state on reconnect
        this.updateState({
          daemonConnected: true,
          lastMessage: null,
          currentMessage: '',
          currentMessageId: null,
          speaking: false,
        })
      }

      this.eventSource.onerror = () => {
        console.log('[BrowserDaemonClient] SSE error/disconnected')
        this.updateState({ daemonConnected: false })
        // Reconnect after delay
        setTimeout(() => this.connectSSE(), 3000)
      }

      // Handle SSE messages - daemon sends StreamBundle format
      // The daemon may send events without an event: field (generic message)
      // or with event: message, so we handle both
      this.eventSource.onmessage = (event) => {
        this.processSSEData(event.data)
      }

      // Also listen for named 'message' event in case daemon sends it that way
      this.eventSource.addEventListener('message', (event) => {
        this.processSSEData(event.data)
      })
    } catch (e) {
      console.error('[BrowserDaemonClient] Failed to create EventSource:', e)
      this.updateState({ daemonConnected: false })
    }
  }

  /**
   * Process SSE data as StreamBundle format
   */
  private processSSEData(data: string): void {
    try {
      const bundle = JSON.parse(data) as StreamBundle
      console.log('[BrowserDaemonClient] Event:', bundle.type)
      this.processBundle(bundle)
    } catch (e) {
      console.error('[BrowserDaemonClient] Failed to parse SSE data:', e, data)
    }
  }

  /**
   * Process a StreamBundle based on its type
   */
  private processBundle(bundle: StreamBundle): void {
    switch (bundle.type) {
      case 'indicator':
        this.handleIndicator(bundle.payload as IndicatorPayload)
        break

      case 'text_delta':
        this.handleTextDelta(bundle.payload as TextDeltaPayload, bundle.message_id)
        break

      case 'tool_call':
        this.handleToolCall(bundle.payload as ToolCallPayload)
        break

      case 'conversation_closed':
        this.handleConversationClosed(bundle.payload as ConversationClosedPayload)
        break

      case 'error':
        console.error('[BrowserDaemonClient] Error event:', bundle.payload)
        break

      case 'heartbeat':
        // Keep-alive, no action needed
        break

      default:
        console.warn('[BrowserDaemonClient] Unknown bundle type:', (bundle as StreamBundle).type)
    }
  }

  /**
   * Handle indicator events - map to state flags
   */
  private handleIndicator(payload: IndicatorPayload): void {
    const indicator = payload.indicator

    // When transitioning to idle with accumulated text, finalize the message
    if (indicator === 'idle' && this.currentState.currentMessage) {
      this.finalizeStreamingMessage()
      return
    }

    // Determine which indicators should show as bubbles
    const bubbleIndicators: IndicatorType[] = ['thinking', 'analyzing']
    const activeIndicator = bubbleIndicators.includes(indicator) ? indicator : null

    // Map indicator to state flags
    const stateChanges = this.mapIndicatorToState(indicator)
    this.updateState({ ...stateChanges, activeIndicator })
  }

  private mapIndicatorToState(indicator: IndicatorType): Partial<BobeContext> {
    const map: Record<IndicatorType, Partial<BobeContext>> = {
      idle: { capturing: false, thinking: false, speaking: false },
      capturing: { capturing: true, thinking: false, speaking: false },
      analyzing: { thinking: true, speaking: false },
      thinking: { thinking: true, speaking: false },
      generating: { thinking: true, speaking: false },
      speaking: { thinking: false, speaking: true },
    }
    return map[indicator] || {}
  }

  /**
   * Handle text_delta events - accumulate and update messages array for streaming display
   */
  private handleTextDelta(payload: TextDeltaPayload, messageId: string): void {
    // New message started
    if (this.streamingMessageId !== messageId) {
      this.streamingMessage = ''
      this.streamingMessageId = messageId
    }

    // Accumulate delta
    this.streamingMessage += payload.delta

    // Update messages array for UI
    let updatedMessages = [...(this.currentState.messages || [])]

    // Clear pending state from user messages now that we're getting a response
    updatedMessages = updatedMessages.map((m) => (m.isPending ? { ...m, isPending: false } : m))

    // Check if we already have a streaming message
    const existingIdx = updatedMessages.findIndex((m) => m.id === messageId && m.isStreaming)

    if (existingIdx >= 0) {
      // Update existing streaming message
      const existing = updatedMessages[existingIdx]!
      updatedMessages[existingIdx] = {
        ...existing,
        content: this.streamingMessage,
      }
    } else {
      // Create new streaming message
      const newMessage: ChatMessage = {
        id: messageId,
        sender: 'bobe',
        content: this.streamingMessage,
        timestamp: Date.now(),
        isStreaming: true,
      }
      updatedMessages.push(newMessage)
    }

    this.updateState({
      currentMessage: this.streamingMessage,
      currentMessageId: messageId,
      messages: updatedMessages,
    })

    // Message complete
    if (payload.done) {
      this.finalizeStreamingMessage()
    }
  }

  /**
   * Finalize a streaming message - mark as complete and clear streaming state
   */
  private finalizeStreamingMessage(): void {
    if (!this.streamingMessageId) return

    let updatedMessages = (this.currentState.messages || []).map((m) =>
      m.id === this.streamingMessageId
        ? { ...m, content: this.streamingMessage, isStreaming: false }
        : m,
    )

    // Trim to max visible
    if (updatedMessages.length > MAX_VISIBLE_MESSAGES) {
      updatedMessages = updatedMessages.slice(-MAX_VISIBLE_MESSAGES)
    }

    this.updateState({
      lastMessage: this.streamingMessage,
      currentMessage: '',
      currentMessageId: null,
      thinking: false,
      speaking: false,
      activeIndicator: null,
      messages: updatedMessages,
    })

    // Reset streaming state
    this.streamingMessage = ''
    this.streamingMessageId = null
  }

  private handleToolCall(payload: ToolCallPayload): void {
    let updatedExecutions = [...(this.currentState.toolExecutions || [])]

    if (payload.status === 'start') {
      // Add new running tool
      updatedExecutions.push({
        tool_name: payload.tool_name,
        tool_call_id: payload.tool_call_id,
        status: 'running',
        startedAt: Date.now(),
      })
    } else {
      // Update completed tool
      updatedExecutions = updatedExecutions.map((t) =>
        t.tool_call_id === payload.tool_call_id
          ? {
              ...t,
              status: payload.success ? 'success' : 'error',
              error: payload.error ?? undefined,
              duration_ms: payload.duration_ms ?? undefined,
              completedAt: Date.now(),
            }
          : t,
      ) as ToolExecution[]

      // Clean up completed tools after short delay (for fade-out animation)
      const completedId = payload.tool_call_id
      setTimeout(() => {
        this.updateState({
          toolExecutions: (this.currentState.toolExecutions || []).filter(
            (t) => t.tool_call_id !== completedId || t.status === 'running',
          ),
        })
      }, 2000)
    }

    this.updateState({ toolExecutions: updatedExecutions })
  }

  private handleConversationClosed(payload: ConversationClosedPayload): void {
    console.log(
      '[BrowserDaemonClient] Conversation closed:',
      payload.conversation_id,
      payload.reason,
      `(${payload.turn_count} turns)`,
    )

    // Clear all message state
    this.updateState({
      messages: [],
      lastMessage: null,
      currentMessage: '',
      currentMessageId: null,
      thinking: false,
      speaking: false,
      activeIndicator: null,
      toolExecutions: [],
    })

    // Reset streaming state
    this.streamingMessage = ''
    this.streamingMessageId = null
  }

  private updateState(partial: Partial<BobeContext>): void {
    this.currentState = { ...this.currentState, ...partial }
    this.stateCallbacks.forEach((cb) => cb(this.currentState))
  }

  async getState(): Promise<Partial<BobeContext>> {
    try {
      const response = await daemonFetch(`${DAEMON_URL}/status`)
      if (response.ok) {
        const data = await response.json()
        this.currentState = {
          ...this.currentState,
          ...data,
          daemonConnected: true,
        }
        return this.currentState
      }
    } catch {
      console.log('[BrowserDaemonClient] Daemon not available')
    }
    return { ...this.currentState, daemonConnected: false }
  }

  onStateUpdate(callback: StateUpdateCallback): () => void {
    this.stateCallbacks.add(callback)
    return () => this.stateCallbacks.delete(callback)
  }

  async toggleCapture(): Promise<boolean> {
    try {
      const newState = !this.currentState.capturing
      const endpoint = newState ? '/capture/start' : '/capture/stop'
      const response = await daemonFetch(`${DAEMON_URL}${endpoint}`, { method: 'POST' })
      if (response.ok) {
        this.updateState({ capturing: newState })
        return newState
      }
    } catch (e) {
      console.error('[BrowserDaemonClient] toggleCapture failed:', e)
    }
    return this.currentState.capturing ?? false
  }

  async dismissMessage(): Promise<void> {
    this.updateState({
      lastMessage: null,
      currentMessage: '',
      speaking: false,
    })
    try {
      await daemonFetch(`${DAEMON_URL}/message/dismiss`, { method: 'POST' })
    } catch {
      // Ignore - local state already updated
    }
  }

  async sendMessage(content: string): Promise<string | undefined> {
    try {
      // Add user message to the chat stack with pending state
      const userMessage: ChatMessage = {
        id: `user-${Date.now()}`,
        sender: 'user',
        content,
        timestamp: Date.now(),
        isStreaming: false,
        isPending: true, // Greyed out until we get a response
      }

      const updatedMessages = [...(this.currentState.messages || []), userMessage]
      this.updateState({ messages: updatedMessages })

      const response = await daemonFetch(`${DAEMON_URL}/message`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ content }),
      })
      if (response.ok) {
        const data = await response.json()
        return data.message_id
      }
    } catch (e) {
      console.error('[BrowserDaemonClient] sendMessage failed:', e)
      // Mark message as failed (not pending anymore)
      const updatedMessages = (this.currentState.messages || []).map((m) =>
        m.isPending ? { ...m, isPending: false } : m,
      )
      this.updateState({ messages: updatedMessages })
    }
    return undefined
  }

  async clearMessages(): Promise<void> {
    this.updateState({
      messages: [],
      lastMessage: null,
      currentMessage: '',
      currentMessageId: null,
    })
    // Reset streaming state
    this.streamingMessage = ''
    this.streamingMessageId = null
  }

  async resizeForBubble(_show: boolean): Promise<void> {
    /* no-op in browser */
  }
  async resizeWindow(_width: number, _height: number): Promise<void> {
    /* no-op in browser */
  }
}

// Singleton instance
let browserClient: BrowserDaemonClient | null = null

export function getBrowserDaemonClient(): BrowserDaemonClient {
  if (!browserClient) {
    browserClient = new BrowserDaemonClient()
  }
  return browserClient
}

export function isElectron(): boolean {
  return typeof window !== 'undefined' && 'bobe' in window && window.bobe !== undefined
}

export function getBobeClient() {
  if (isElectron()) {
    return window.bobe!
  }
  return getBrowserDaemonClient()
}
