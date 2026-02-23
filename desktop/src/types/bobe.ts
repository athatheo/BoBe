/**
 * Core BoBe types with discriminated unions for type-safe state handling
 */

// State type for IPC communication (matches current daemon API)
export type BobeStateType =
  | 'loading'
  | 'idle'
  | 'capturing'
  | 'thinking'
  | 'speaking'
  | 'wants_to_speak'

// Indicator types from daemon (for indicator bubbles)
export type IndicatorType =
  | 'idle'
  | 'capturing'
  | 'analyzing'
  | 'thinking'
  | 'generating'
  | 'speaking'

// Chat message in the stacking bubble system
export interface ChatMessage {
  id: string
  sender: 'user' | 'bobe'
  content: string
  timestamp: number
  isStreaming?: boolean // True while response is being streamed
  isPending?: boolean // True for user messages awaiting response
}

// Tool execution tracking for indicator display
export interface ToolExecution {
  tool_name: string
  tool_call_id: string
  status: 'running' | 'success' | 'error'
  error?: string
  duration_ms?: number
  startedAt: number
  completedAt?: number
}

// Context containing full application state
export interface BobeContext {
  // Connection status
  daemonConnected: boolean

  // Capture states
  capturing: boolean

  // LLM states
  thinking: boolean
  speaking: boolean

  // Legacy message state (kept for compatibility)
  lastMessage: string | null
  currentMessage: string // Streaming message accumulator
  currentMessageId: string | null // ID of message being streamed

  // New chat system
  messages: ChatMessage[] // Stack of chat messages
  activeIndicator: IndicatorType | null // Current indicator for bubble display

  // Tool execution tracking
  toolExecutions: ToolExecution[] // Currently/recently executing tools

  // Computed state type for UI
  stateType: BobeStateType
}

// Default state
export const DEFAULT_BOBE_CONTEXT: BobeContext = {
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
  stateType: 'loading',
}

/**
 * Derive the UI state type from the context
 * Priority order: loading > speaking > thinking > wants_to_speak > capturing > idle
 */
export function deriveStateType(context: Omit<BobeContext, 'stateType'>): BobeStateType {
  if (!context.daemonConnected) return 'loading'
  if (context.speaking) return 'speaking'
  if (context.thinking) return 'thinking'
  if (context.lastMessage && !context.speaking) return 'wants_to_speak'
  if (context.capturing) return 'capturing'
  return 'idle'
}
