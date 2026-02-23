/**
 * Electron process type definitions
 */

// Re-export daemon types
export * from './daemon'

// Re-export shared types from renderer (single source of truth)
export type { BobeStateType, IndicatorType, ChatMessage, ToolExecution } from '../../src/types/bobe'

// Import for local use in this file
import type { BobeStateType, IndicatorType, ChatMessage, ToolExecution } from '../../src/types/bobe'

// State interface - mirrors the renderer types
export interface BobeState {
  capturing: boolean
  thinking: boolean
  speaking: boolean
  lastMessage: string | null
  currentMessage: string // Accumulator for streaming text_delta
  currentMessageId: string | null // Track which message is being streamed
  daemonConnected: boolean
  stateType: BobeStateType
  // New chat system
  messages: ChatMessage[] // Stack of chat messages
  activeIndicator: IndicatorType | null // Current indicator for bubble display
  // Tool execution tracking
  toolExecutions: ToolExecution[] // Currently/recently executing tools
}

// IPC channel constants
export const IPC_CHANNELS = {
  // Invoke channels (renderer → main)
  GET_STATE: 'bobe:get-state',
  TOGGLE_CAPTURE: 'bobe:toggle-capture',
  DISMISS_MESSAGE: 'bobe:dismiss-message',
  RESIZE_FOR_BUBBLE: 'bobe:resize-for-bubble',
  RESIZE_WINDOW: 'bobe:resize-window',
  SEND_MESSAGE: 'bobe:send-message',
  CLEAR_MESSAGES: 'bobe:clear-messages',

  // Goals API channels
  GOALS_LIST: 'goals:list',
  GOALS_GET: 'goals:get',
  GOALS_CREATE: 'goals:create',
  GOALS_UPDATE: 'goals:update',
  GOALS_DELETE: 'goals:delete',
  GOALS_COMPLETE: 'goals:complete',
  GOALS_ARCHIVE: 'goals:archive',

  // Souls API channels
  SOULS_LIST: 'souls:list',
  SOULS_GET: 'souls:get',
  SOULS_CREATE: 'souls:create',
  SOULS_UPDATE: 'souls:update',
  SOULS_DELETE: 'souls:delete',
  SOULS_ENABLE: 'souls:enable',
  SOULS_DISABLE: 'souls:disable',

  // User Profiles API channels
  USER_PROFILES_LIST: 'user-profiles:list',
  USER_PROFILES_GET: 'user-profiles:get',
  USER_PROFILES_CREATE: 'user-profiles:create',
  USER_PROFILES_UPDATE: 'user-profiles:update',
  USER_PROFILES_DELETE: 'user-profiles:delete',
  USER_PROFILES_ENABLE: 'user-profiles:enable',
  USER_PROFILES_DISABLE: 'user-profiles:disable',

  // Tools API channels
  TOOLS_LIST: 'tools:list',
  TOOLS_ENABLE: 'tools:enable',
  TOOLS_DISABLE: 'tools:disable',

  // MCP Servers API channels
  MCP_SERVERS_LIST: 'mcp-servers:list',
  MCP_SERVERS_LIST_CONFIGS: 'mcp-servers:list-configs',
  MCP_SERVERS_CREATE: 'mcp-servers:create',
  MCP_SERVERS_UPDATE: 'mcp-servers:update',
  MCP_SERVERS_DELETE: 'mcp-servers:delete',
  MCP_SERVERS_RECONNECT: 'mcp-servers:reconnect',

  // Settings API channels
  SETTINGS_GET: 'settings:get',
  SETTINGS_UPDATE: 'settings:update',
  SETTINGS_SELECT_DIRECTORY: 'settings:select-directory',

  // Models API channels
  MODELS_LIST: 'models:list',
  MODELS_REGISTRY: 'models:registry',
  MODELS_PULL: 'models:pull',
  MODELS_DELETE: 'models:delete',

  // Memories API channels
  MEMORIES_LIST: 'memories:list',
  MEMORIES_GET: 'memories:get',
  MEMORIES_CREATE: 'memories:create',
  MEMORIES_UPDATE: 'memories:update',
  MEMORIES_DELETE: 'memories:delete',
  MEMORIES_ENABLE: 'memories:enable',
  MEMORIES_DISABLE: 'memories:disable',

  // Permission channels
  PERMISSIONS_CHECK_SCREEN: 'permissions:check-screen',
  PERMISSIONS_OPEN_SCREEN_SETTINGS: 'permissions:open-screen-settings',
  PERMISSIONS_CHECK_DATA_DIR: 'permissions:check-data-dir',

  // Event channels (main → renderer)
  STATE_UPDATE: 'bobe:state-update',
} as const

/**
 * Derive the state type from boolean flags
 * Priority order: loading > speaking > thinking > wants_to_speak > capturing > idle
 */
export function deriveStateType(state: Omit<BobeState, 'stateType'>): BobeStateType {
  if (!state.daemonConnected) return 'loading'
  if (state.speaking) return 'speaking'
  if (state.thinking) return 'thinking'
  if (state.lastMessage && !state.speaking) return 'wants_to_speak'
  if (state.capturing) return 'capturing'
  return 'idle'
}
