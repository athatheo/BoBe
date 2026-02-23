/**
 * BoBe Store - Single source of truth for application state
 *
 * ARCHITECTURE:
 * - State: External store with subscribe/getSnapshot (React 18/19 pattern)
 * - Actions: Plain async functions (NOT hooks)
 * - Hooks: useBobe() for full access, useBobeSelector() for performance
 *
 * RULES:
 * 1. No hook nesting - this is the only state hook
 * 2. Actions are functions, not hooks - call from anywhere
 * 3. Loading state is handled locally via useTransition (React 19)
 * 4. All state flows through this single store
 *
 * USAGE:
 * ```tsx
 * // Most components - full access
 * const { state, toggleCapture } = useBobe()
 *
 * // Performance-critical - selector
 * const isCapturing = useBobeSelector(s => s.capturing)
 *
 * // With local pending state (React 19)
 * const [isPending, startTransition] = useTransition()
 * onClick={() => startTransition(() => toggleCapture())}
 * ```
 */

import { useSyncExternalStore } from 'react'
import { type BobeContext, DEFAULT_BOBE_CONTEXT, deriveStateType } from '@/types/bobe'
import { getBobeClient, isElectron } from '@/lib/browser-daemon-client'

// =============================================================================
// STORE STATE
// =============================================================================

let currentState: BobeContext = { ...DEFAULT_BOBE_CONTEXT }
const listeners = new Set<() => void>()

// =============================================================================
// STORE PRIMITIVES
// =============================================================================

/**
 * Get current state snapshot.
 * Used by useSyncExternalStore.
 */
function getSnapshot(): BobeContext {
  return currentState
}

/**
 * Subscribe to state changes.
 * Used by useSyncExternalStore.
 */
function subscribe(callback: () => void): () => void {
  listeners.add(callback)
  return () => listeners.delete(callback)
}

/**
 * Update state and notify subscribers.
 * @internal Used by IPC handlers only
 */
function setState(partial: Partial<BobeContext>): void {
  const merged = { ...currentState, ...partial }
  currentState = {
    ...merged,
    stateType: deriveStateType(merged),
  }
  // Notify synchronously
  listeners.forEach((cb) => cb())
}

// =============================================================================
// CLIENT INITIALIZATION
// =============================================================================

let initialized = false

/**
 * Initialize connection to backend (Electron IPC or direct daemon).
 * Called automatically on module load.
 *
 * In Electron: Uses window.bobe from preload script
 * In Browser: Uses BrowserDaemonClient for direct HTTP/SSE
 */
function initializeClient(): void {
  if (initialized) return
  if (typeof window === 'undefined') return

  initialized = true

  const client = getBobeClient()
  const mode = isElectron() ? 'Electron IPC' : 'Browser Direct'
  console.log(`[BobeStore] Initializing in ${mode} mode`)

  // Fetch initial state
  client.getState().then((state) => {
    setState(state as Partial<BobeContext>)
  })

  // Subscribe to state updates
  client.onStateUpdate((state) => {
    setState(state as Partial<BobeContext>)
  })
}

// Auto-initialize in browser
if (typeof window !== 'undefined') {
  // Small delay to ensure DOM is ready
  setTimeout(initializeClient, 0)
}

// =============================================================================
// ACTIONS
// Plain async functions - NOT hooks
// Call these from components, event handlers, or anywhere
// Uses getBobeClient() to work in both Electron and Browser modes
// =============================================================================

/**
 * Toggle screen capture on/off.
 * State update comes back via subscription.
 */
async function toggleCapture(): Promise<boolean | undefined> {
  return getBobeClient().toggleCapture()
}

/**
 * Dismiss the current message/speech bubble.
 * State update comes back via subscription.
 */
async function dismissMessage(): Promise<void> {
  return getBobeClient().dismissMessage()
}

/**
 * Resize window to accommodate speech bubble (legacy boolean API).
 * No-op in browser mode.
 */
async function resizeForBubble(show: boolean): Promise<void> {
  return getBobeClient().resizeForBubble(show)
}

/**
 * Resize window to specific dimensions.
 * Used for dynamic content-based sizing. No-op in browser mode.
 */
async function resizeWindow(width: number, height: number): Promise<void> {
  return getBobeClient().resizeWindow(width, height)
}

/**
 * Send a message to the daemon.
 * Response will stream via SSE and update state.
 * Returns the message ID.
 */
async function sendMessage(content: string): Promise<string | undefined> {
  return getBobeClient().sendMessage(content)
}

/**
 * Clear all messages in the chat stack.
 * Called when closing the chat panel.
 */
async function clearMessages(): Promise<void> {
  return getBobeClient().clearMessages()
}

// Export actions as a const object for stable reference
export const bobeActions = {
  toggleCapture,
  dismissMessage,
  resizeForBubble,
  resizeWindow,
  sendMessage,
  clearMessages,
} as const

// =============================================================================
// HOOKS
// =============================================================================

/**
 * Primary hook for accessing BoBe state and actions.
 *
 * Returns full state and all actions. Actions are stable references
 * (same function every render).
 *
 * For pending/loading states during actions, use React 19's useTransition:
 * ```tsx
 * const { toggleCapture } = useBobe()
 * const [isPending, startTransition] = useTransition()
 *
 * <button
 *   onClick={() => startTransition(() => toggleCapture())}
 *   disabled={isPending}
 * >
 *   {isPending ? 'Loading...' : 'Toggle'}
 * </button>
 * ```
 */
export function useBobe() {
  const state = useSyncExternalStore(subscribe, getSnapshot, getSnapshot)

  // Return state + stable action references
  // Actions object is defined at module level, so reference is stable
  return {
    // Full state object
    state,

    // Commonly accessed properties (convenience)
    stateType: state.stateType,
    isConnected: state.daemonConnected,
    isCapturing: state.capturing,
    isThinking: state.thinking,
    isSpeaking: state.speaking,
    hasMessage: state.lastMessage !== null,
    lastMessage: state.lastMessage,
    currentMessage: state.currentMessage,
    currentMessageId: state.currentMessageId,

    // New chat system
    messages: state.messages,
    activeIndicator: state.activeIndicator,

    // Tool executions
    toolExecutions: state.toolExecutions,
    runningTools: state.toolExecutions.filter((t) => t.status === 'running'),

    // Actions (stable references)
    toggleCapture,
    dismissMessage,
    resizeForBubble,
    resizeWindow,
    sendMessage,
    clearMessages,
  }
}

/**
 * Selector hook for subscribing to specific state slices.
 *
 * Only re-renders when the selected value changes (by reference).
 * Use for performance-critical components that only need part of the state.
 *
 * @example
 * // Only re-renders when capturing changes
 * const isCapturing = useBobeSelector(s => s.capturing)
 *
 * // Only re-renders when stateType changes
 * const stateType = useBobeSelector(s => s.stateType)
 *
 * // Derived value - re-renders when any dependency changes
 * const isActive = useBobeSelector(s => s.capturing || s.thinking)
 */
export function useBobeSelector<T>(selector: (state: BobeContext) => T): T {
  return useSyncExternalStore(
    subscribe,
    () => selector(getSnapshot()),
    () => selector(getSnapshot()),
  )
}

// =============================================================================
// EXPORTS FOR TESTING/ADVANCED USE
// =============================================================================

export { getSnapshot, subscribe, initializeClient }
