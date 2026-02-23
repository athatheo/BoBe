/**
 * IPC handlers
 *
 * Handles all IPC communication between renderer and main process.
 * Routes commands to daemon and broadcasts state updates from SSE.
 */

import { ipcMain } from 'electron'
import {
  IPC_CHANNELS,
  type BobeState,
  type IndicatorPayload,
  type IndicatorType,
  type ChatMessage,
  type ToolCallPayload,
  type ConversationClosedPayload,
  deriveStateType,
} from '../types'
import type { ToolExecution } from '../../src/types/bobe'
import { getOverlayWindow, resizeForBubble, resizeWindow, daemonClient } from '../services'
import { setupPermissionIpcHandlers } from './permission-handlers'

// Maximum visible messages in chat stack
const MAX_VISIBLE_MESSAGES = 4

// Auto-clear lastMessage after this duration (ms)
const LAST_MESSAGE_AUTO_CLEAR_MS = 30_000

// Timer for auto-clearing lastMessage
let lastMessageClearTimer: ReturnType<typeof setTimeout> | null = null

// Application state - single source of truth
let state: BobeState & { toolExecutions: ToolExecution[] } = {
  capturing: false,
  thinking: false,
  speaking: false,
  lastMessage: null,
  currentMessage: '',
  currentMessageId: null,
  daemonConnected: false,
  stateType: 'loading',
  messages: [],
  activeIndicator: null,
  toolExecutions: [],
}

// =============================================================================
// STATE MANAGEMENT
// =============================================================================

/**
 * Get current state
 */
export function getState(): BobeState {
  return { ...state }
}

/**
 * Update state and broadcast to renderer
 */
export function setState(newState: Partial<Omit<BobeState, 'stateType'>>): void {
  // Handle lastMessage auto-clear timer
  if ('lastMessage' in newState) {
    // Clear any existing timer when lastMessage changes
    if (lastMessageClearTimer) {
      clearTimeout(lastMessageClearTimer)
      lastMessageClearTimer = null
    }

    // Start new timer if lastMessage is being set to a value
    if (newState.lastMessage !== null) {
      lastMessageClearTimer = setTimeout(() => {
        console.log('[STATE] Auto-clearing lastMessage after 30s')
        setState({ lastMessage: null })
      }, LAST_MESSAGE_AUTO_CLEAR_MS)
    }
  }

  const updatedState = { ...state, ...newState }
  state = {
    ...updatedState,
    stateType: deriveStateType(updatedState),
  }
  broadcastState()
}

/**
 * Broadcast state to renderer
 */
export function broadcastState(): void {
  const overlayWindow = getOverlayWindow()
  if (!overlayWindow || overlayWindow.isDestroyed()) {
    console.log('[BROADCAST] Window not available')
    return
  }
  console.log('[BROADCAST] Sending state:', state.stateType, {
    capturing: state.capturing,
    thinking: state.thinking,
    speaking: state.speaking,
    connected: state.daemonConnected,
  })
  overlayWindow.webContents.send(IPC_CHANNELS.STATE_UPDATE, { ...state })
}

// =============================================================================
// INDICATOR MAPPING
// =============================================================================

/**
 * Map daemon indicator to frontend state flags
 */
function mapIndicatorToState(payload: IndicatorPayload): Partial<Omit<BobeState, 'stateType'>> {
  const indicator = payload.indicator

  // Base state changes for each indicator
  const indicatorMap: Record<IndicatorType, Partial<Omit<BobeState, 'stateType'>>> = {
    idle: {
      capturing: false,
      thinking: false,
      speaking: false,
    },
    capturing: {
      capturing: true,
      thinking: false,
      speaking: false,
    },
    analyzing: {
      thinking: true,
      speaking: false,
    },
    thinking: {
      thinking: true,
      speaking: false,
    },
    generating: {
      thinking: true,
      speaking: false,
    },
    speaking: {
      thinking: false,
      speaking: true,
    },
  }

  return indicatorMap[indicator] || {}
}

// =============================================================================
// DAEMON EVENT HANDLERS
// =============================================================================

/**
 * Initialize daemon client event handlers
 */
export function initDaemonEventHandlers(): void {
  // Connection events
  daemonClient.on('connected', async () => {
    console.log('[IPC] Daemon connected')
    // Clear stale message state on reconnect to prevent "Hey" from re-appearing
    // after laptop sleep/wake cycles
    setState({
      daemonConnected: true,
      lastMessage: null,
      currentMessage: '',
      currentMessageId: null,
      speaking: false,
    })
  })

  daemonClient.on('disconnected', () => {
    console.log('[IPC] Daemon disconnected')
    setState({ daemonConnected: false })
  })

  daemonClient.on('reconnecting', ({ attempt, maxAttempts }) => {
    console.log(`[IPC] Reconnecting to daemon (${attempt}/${maxAttempts})`)
  })

  // Indicator changes
  daemonClient.on('indicator', (payload) => {
    console.log('[IPC] Indicator event:', payload.indicator)

    // When transitioning to idle with accumulated text, finalize the message
    if (payload.indicator === 'idle' && state.currentMessage) {
      console.log('[IPC] Finalizing accumulated message:', state.currentMessage.substring(0, 50))
      setState({
        lastMessage: state.currentMessage,
        currentMessage: '',
        currentMessageId: null,
        thinking: false,
        speaking: false,
        activeIndicator: null,
      })
      return
    }

    // Determine which indicators should show as bubbles
    // thinking, analyzing get bubble treatment
    // idle, capturing, speaking, generating don't need indicator bubbles
    const bubbleIndicators: IndicatorType[] = ['thinking', 'analyzing']
    const activeIndicator = bubbleIndicators.includes(payload.indicator) ? payload.indicator : null

    const stateChanges = mapIndicatorToState(payload)
    setState({ ...stateChanges, activeIndicator })
  })

  // Text streaming
  daemonClient.on('text_delta', (payload) => {
    const streamingContent = daemonClient.getCurrentMessage()

    // Check if we already have a streaming message in the array
    const existingStreamingIdx = state.messages.findIndex(
      (m) => m.id === payload.message_id && m.isStreaming,
    )

    let updatedMessages = [...state.messages]

    // Clear pending state from user messages now that we're getting a response
    updatedMessages = updatedMessages.map((m) => (m.isPending ? { ...m, isPending: false } : m))

    if (existingStreamingIdx >= 0) {
      // Update existing streaming message
      const existing = updatedMessages[existingStreamingIdx]!
      updatedMessages[existingStreamingIdx] = {
        ...existing,
        content: streamingContent,
      }
    } else {
      // Create new streaming message
      const newMessage: ChatMessage = {
        id: payload.message_id,
        sender: 'bobe',
        content: streamingContent,
        timestamp: Date.now(),
        isStreaming: true,
      }
      updatedMessages.push(newMessage)
    }

    setState({
      currentMessage: streamingContent,
      currentMessageId: payload.message_id,
      messages: updatedMessages,
    })
  })

  // Message complete
  daemonClient.on('message_complete', ({ message_id, content }) => {
    console.log('[IPC] Message complete:', message_id, content.substring(0, 50) + '...')

    // Mark the streaming message as complete
    let updatedMessages = state.messages.map((m) =>
      m.id === message_id ? { ...m, content, isStreaming: false } : m,
    )

    // Trim to max visible
    if (updatedMessages.length > MAX_VISIBLE_MESSAGES) {
      updatedMessages = updatedMessages.slice(-MAX_VISIBLE_MESSAGES)
    }

    setState({
      lastMessage: content,
      currentMessage: '',
      currentMessageId: null,
      thinking: false,
      speaking: false, // Explicitly clear speaking to allow wants_to_speak state
      activeIndicator: null,
      messages: updatedMessages,
    })
  })

  // Error events
  daemonClient.on('error', (payload) => {
    console.error('[IPC] Daemon error:', payload.code, payload.message)
    // Could show notification or update UI state
  })

  // Heartbeat (keep-alive)
  daemonClient.on('heartbeat', () => {
    // Could update a "last seen" timestamp if needed
  })

  // Tool call events - track tool executions for indicator display
  daemonClient.on('tool_call', (payload: ToolCallPayload) => {
    console.log('[IPC] Tool call event:', payload.tool_name, payload.status)

    let updatedExecutions = [...state.toolExecutions]

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
              error: payload.error,
              duration_ms: payload.duration_ms,
              completedAt: Date.now(),
            }
          : t,
      ) as ToolExecution[]

      // Clean up completed tools after short delay (for fade-out animation)
      const completedId = payload.tool_call_id
      setTimeout(() => {
        setState({
          toolExecutions: state.toolExecutions.filter(
            (t) => t.tool_call_id !== completedId || t.status === 'running',
          ),
        })
      }, 2000)
    }

    setState({ toolExecutions: updatedExecutions })
  })

  // Conversation closed - clear chat history
  daemonClient.on('conversation_closed', (payload: ConversationClosedPayload) => {
    console.log(
      '[IPC] Conversation closed:',
      payload.conversation_id,
      payload.reason,
      `(${payload.turn_count} turns)`,
    )

    // Clear all message state
    setState({
      messages: [],
      lastMessage: null,
      currentMessage: '',
      currentMessageId: null,
      thinking: false,
      speaking: false,
      activeIndicator: null,
      toolExecutions: [],
    })
  })

}

// =============================================================================
// IPC HANDLERS
// =============================================================================

/**
 * Toggle capture state via daemon
 */
async function toggleCapture(): Promise<boolean> {
  const newCapturing = !state.capturing

  try {
    if (newCapturing) {
      await daemonClient.startCapture()
    } else {
      await daemonClient.stopCapture()
    }
    // Note: actual state update comes via SSE indicator event
    // But we can optimistically update for responsiveness
    setState({ capturing: newCapturing })
    return newCapturing
  } catch (error) {
    console.error('[IPC] Failed to toggle capture:', error)
    return state.capturing // Return current state on failure
  }
}

/**
 * Dismiss current message
 */
function dismissMessage(): void {
  setState({
    lastMessage: null,
    currentMessage: '',
    currentMessageId: null,
    speaking: false,
  })
}

/**
 * Clear all messages in the chat stack
 */
function clearMessages(): void {
  setState({
    messages: [],
    lastMessage: null,
    currentMessage: '',
    currentMessageId: null,
  })
}

/**
 * Send a message to the daemon
 */
async function sendMessage(content: string): Promise<string> {
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

    const updatedMessages = [...state.messages, userMessage]

    // Don't trim while sending - we want to keep the pending message visible
    setState({ messages: updatedMessages })

    const response = await daemonClient.sendMessage(content)
    // Response streams via SSE - just return the message ID
    return response.message_id
  } catch (error) {
    console.error('[IPC] Failed to send message:', error)
    // Mark message as failed (not pending anymore but could add error state)
    const updatedMessages = state.messages.map((m) =>
      m.isPending ? { ...m, isPending: false } : m,
    )
    setState({ messages: updatedMessages })
    throw error
  }
}

/**
 * Setup all IPC handlers
 */
export function setupIpcHandlers(): void {
  ipcMain.handle(IPC_CHANNELS.GET_STATE, () => getState())

  ipcMain.handle(IPC_CHANNELS.TOGGLE_CAPTURE, () => toggleCapture())

  ipcMain.handle(IPC_CHANNELS.DISMISS_MESSAGE, () => dismissMessage())

  ipcMain.handle(IPC_CHANNELS.RESIZE_FOR_BUBBLE, (_e, showBubble: boolean) => {
    resizeForBubble(showBubble)
  })

  ipcMain.handle(IPC_CHANNELS.RESIZE_WINDOW, (_e, width: number, height: number) => {
    resizeWindow(width, height)
  })

  ipcMain.handle(IPC_CHANNELS.SEND_MESSAGE, (_e, content: string) => sendMessage(content))

  ipcMain.handle(IPC_CHANNELS.CLEAR_MESSAGES, () => clearMessages())

  // Permission IPC handlers (screen recording, data dir)
  setupPermissionIpcHandlers()
}

// =============================================================================
// DEBUG ACTIONS (for development)
// =============================================================================

export const debugActions = {
  loading: () =>
    setState({
      daemonConnected: false,
      capturing: false,
      thinking: false,
      speaking: false,
      lastMessage: null,
      currentMessage: '',
      currentMessageId: null,
    }),

  idle: () =>
    setState({
      daemonConnected: true,
      capturing: false,
      thinking: false,
      speaking: false,
      lastMessage: null,
      currentMessage: '',
      currentMessageId: null,
    }),

  capturing: () =>
    setState({
      daemonConnected: true,
      capturing: true,
      thinking: false,
      speaking: false,
      lastMessage: null,
    }),

  thinking: () =>
    setState({
      daemonConnected: true,
      capturing: true,
      thinking: true,
      speaking: false,
      lastMessage: null,
    }),

  wants_to_speak: () =>
    setState({
      daemonConnected: true,
      capturing: true,
      thinking: false,
      speaking: false,
      lastMessage:
        "I noticed you've been looking at that error for a while. Want me to help debug it?",
    }),

  speaking: () =>
    setState({
      daemonConnected: true,
      capturing: true,
      thinking: false,
      speaking: true,
      lastMessage:
        "I noticed you've been looking at that error for a while. Want me to help debug it?",
    }),

  set_message: () =>
    setState({
      lastMessage: 'Hey! I noticed something interesting in your code. Want to hear about it?',
    }),

  clear_message: () =>
    setState({
      lastMessage: null,
      currentMessage: '',
      currentMessageId: null,
      speaking: false,
    }),

  clear_all: () =>
    setState({
      daemonConnected: true,
      capturing: false,
      thinking: false,
      speaking: false,
      lastMessage: null,
      currentMessage: '',
      currentMessageId: null,
    }),
}
