/**
 * Preload script - secure bridge between renderer and main process
 *
 * SECURITY NOTES:
 * - Uses explicit channel allowlisting
 * - Wraps callbacks to strip IPC event objects
 * - Never exposes raw ipcRenderer methods
 */

import { contextBridge, ipcRenderer } from 'electron'

// =============================================================================
// CORE BOBE API
// Channel security is enforced by only exposing specific hardcoded channels
// in the contextBridge API below. See src/types/ipc.ts for the canonical list.
// =============================================================================

const bobeApi = {
  /**
   * Get current application state
   */
  getState: () => {
    return ipcRenderer.invoke('bobe:get-state')
  },

  /**
   * Toggle screen capture on/off
   */
  toggleCapture: () => {
    return ipcRenderer.invoke('bobe:toggle-capture')
  },

  /**
   * Dismiss the current message
   */
  dismissMessage: () => {
    return ipcRenderer.invoke('bobe:dismiss-message')
  },

  /**
   * Resize window for speech bubble display (legacy boolean API)
   */
  resizeForBubble: (show: boolean) => {
    return ipcRenderer.invoke('bobe:resize-for-bubble', show)
  },

  /**
   * Resize window to specific dimensions (dynamic sizing)
   */
  resizeWindow: (width: number, height: number) => {
    return ipcRenderer.invoke('bobe:resize-window', width, height)
  },

  /**
   * Send a message to the daemon
   * Returns the message ID (response streams via SSE)
   */
  sendMessage: (content: string) => {
    return ipcRenderer.invoke('bobe:send-message', content)
  },

  /**
   * Clear all messages in the chat stack
   */
  clearMessages: () => {
    return ipcRenderer.invoke('bobe:clear-messages')
  },

  /**
   * Subscribe to state updates from main process
   * Returns unsubscribe function
   *
   * SECURITY: Callback wrapper strips the IPC event object
   * which contains references to ipcRenderer internals
   */
  onStateUpdate: (callback: (state: unknown) => void): (() => void) => {
    const channel = 'bobe:state-update'

    // Wrap callback to strip the event object (security best practice)
    const handler = (_event: Electron.IpcRendererEvent, state: unknown) => {
      callback(state)
    }

    ipcRenderer.on(channel, handler)

    // Return unsubscribe function
    return () => {
      ipcRenderer.removeListener(channel, handler)
    }
  },
}

// =============================================================================
// GOALS API
// =============================================================================

const goalsApi = {
  list: () => ipcRenderer.invoke('goals:list'),
  get: (id: string) => ipcRenderer.invoke('goals:get', id),
  create: (data: { content: string; priority?: string; enabled?: boolean }) =>
    ipcRenderer.invoke('goals:create', data),
  update: (
    id: string,
    data: { content?: string; status?: string; priority?: string; enabled?: boolean },
  ) => ipcRenderer.invoke('goals:update', id, data),
  delete: (id: string) => ipcRenderer.invoke('goals:delete', id),
  complete: (id: string) => ipcRenderer.invoke('goals:complete', id),
  archive: (id: string) => ipcRenderer.invoke('goals:archive', id),
}

// =============================================================================
// SOULS API
// =============================================================================

const soulsApi = {
  list: () => ipcRenderer.invoke('souls:list'),
  get: (id: string) => ipcRenderer.invoke('souls:get', id),
  create: (data: { name: string; content: string; enabled?: boolean }) =>
    ipcRenderer.invoke('souls:create', data),
  update: (id: string, data: { content?: string; enabled?: boolean }) =>
    ipcRenderer.invoke('souls:update', id, data),
  delete: (id: string) => ipcRenderer.invoke('souls:delete', id),
  enable: (id: string) => ipcRenderer.invoke('souls:enable', id),
  disable: (id: string) => ipcRenderer.invoke('souls:disable', id),
}

// =============================================================================
// USER PROFILES API
// =============================================================================

const userProfilesApi = {
  list: () => ipcRenderer.invoke('user-profiles:list'),
  get: (id: string) => ipcRenderer.invoke('user-profiles:get', id),
  create: (data: { name: string; content: string; enabled?: boolean }) =>
    ipcRenderer.invoke('user-profiles:create', data),
  update: (id: string, data: { content?: string; enabled?: boolean }) =>
    ipcRenderer.invoke('user-profiles:update', id, data),
  delete: (id: string) => ipcRenderer.invoke('user-profiles:delete', id),
  enable: (id: string) => ipcRenderer.invoke('user-profiles:enable', id),
  disable: (id: string) => ipcRenderer.invoke('user-profiles:disable', id),
}

// =============================================================================
// TOOLS API
// =============================================================================

const toolsApi = {
  list: () => ipcRenderer.invoke('tools:list'),
  enable: (name: string) => ipcRenderer.invoke('tools:enable', name),
  disable: (name: string) => ipcRenderer.invoke('tools:disable', name),
}

// =============================================================================
// MCP SERVERS API
// =============================================================================

const mcpServersApi = {
  list: () => ipcRenderer.invoke('mcp-servers:list'),
  listConfigs: () => ipcRenderer.invoke('mcp-servers:list-configs'),
  create: (data: {
    name: string
    command: string
    args?: string[]
    env?: Record<string, string>
    enabled?: boolean
    excluded_tools?: string[]
  }) => ipcRenderer.invoke('mcp-servers:create', data),
  update: (id: string, data: { excluded_tools?: string[] }) =>
    ipcRenderer.invoke('mcp-servers:update', id, data),
  delete: (name: string) => ipcRenderer.invoke('mcp-servers:delete', name),
  reconnect: (name: string) => ipcRenderer.invoke('mcp-servers:reconnect', name),
}

// =============================================================================
// SETTINGS API
// =============================================================================

const settingsApi = {
  get: () => ipcRenderer.invoke('settings:get'),
  update: (data: Record<string, unknown>) => ipcRenderer.invoke('settings:update', data),
  selectDirectory: () => ipcRenderer.invoke('settings:select-directory') as Promise<string | null>,
  listModels: () => ipcRenderer.invoke('models:list'),
  listRegistryModels: () => ipcRenderer.invoke('models:registry'),
  pullModel: (name: string) => ipcRenderer.invoke('models:pull', name),
  deleteModel: (name: string) => ipcRenderer.invoke('models:delete', name),
}

// =============================================================================
// MEMORIES API
// =============================================================================

const memoriesApi = {
  list: (params?: {
    memory_type?: 'short_term' | 'long_term' | 'explicit'
    category?: 'preference' | 'pattern' | 'fact' | 'interest' | 'general'
    source?: 'observation' | 'conversation' | 'user'
    enabled_only?: boolean
    limit?: number
    offset?: number
  }) => ipcRenderer.invoke('memories:list', params),
  get: (id: string) => ipcRenderer.invoke('memories:get', id),
  create: (data: {
    content: string
    category?: 'preference' | 'pattern' | 'fact' | 'interest' | 'general'
    memory_type?: 'short_term' | 'long_term' | 'explicit'
  }) => ipcRenderer.invoke('memories:create', data),
  update: (
    id: string,
    data: {
      content?: string
      category?: 'preference' | 'pattern' | 'fact' | 'interest' | 'general'
      enabled?: boolean
    },
  ) => ipcRenderer.invoke('memories:update', id, data),
  delete: (id: string) => ipcRenderer.invoke('memories:delete', id),
  enable: (id: string) => ipcRenderer.invoke('memories:enable', id),
  disable: (id: string) => ipcRenderer.invoke('memories:disable', id),
}

// =============================================================================
// EXPOSE TO RENDERER
// =============================================================================

// =============================================================================
// SETUP / ONBOARDING API (proxied to service /onboarding/* via main process)
// =============================================================================

const setupApi = {
  startLocalSetup: (modelName: string) => ipcRenderer.invoke('bobe:start-local-setup', modelName),
  configureLLM: (mode: string, model: string, apiKey: string) =>
    ipcRenderer.invoke('bobe:configure-llm', mode, model, apiKey),
  completeSetup: () => ipcRenderer.invoke('bobe:complete-setup'),
  getOnboardingStatus: () => ipcRenderer.invoke('bobe:get-onboarding-status'),
  onProgress: (
    callback: (data: { step: string; progress: number; message: string }) => void,
  ): (() => void) => {
    const handler = (
      _event: Electron.IpcRendererEvent,
      data: { step: string; progress: number; message: string },
    ) => {
      callback(data)
    }
    ipcRenderer.on('bobe:setup-progress', handler)
    return () => ipcRenderer.removeListener('bobe:setup-progress', handler)
  },
}

// =============================================================================
// APP DATA MANAGEMENT API
// =============================================================================

const appDataApi = {
  deleteAllData: () => ipcRenderer.invoke('app:delete-all-data'),
  getDataSize: () =>
    ipcRenderer.invoke('app:get-data-size') as Promise<{
      totalMB: number
      breakdown: Record<string, number>
    }>,
}

// =============================================================================
// PERMISSIONS API
// =============================================================================

const permissionsApi = {
  checkScreen: () => ipcRenderer.invoke('permissions:check-screen'),
  openScreenSettings: () => ipcRenderer.invoke('permissions:open-screen-settings'),
  checkDataDir: () => ipcRenderer.invoke('permissions:check-data-dir'),
}

contextBridge.exposeInMainWorld('bobe', bobeApi)
contextBridge.exposeInMainWorld('setup', setupApi)
contextBridge.exposeInMainWorld('appData', appDataApi)
contextBridge.exposeInMainWorld('permissions', permissionsApi)
contextBridge.exposeInMainWorld('goals', goalsApi)
contextBridge.exposeInMainWorld('souls', soulsApi)
contextBridge.exposeInMainWorld('userProfiles', userProfilesApi)
contextBridge.exposeInMainWorld('tools', toolsApi)
contextBridge.exposeInMainWorld('mcpServers', mcpServersApi)
contextBridge.exposeInMainWorld('settings', settingsApi)
contextBridge.exposeInMainWorld('memories', memoriesApi)

// =============================================================================
// TYPE DECLARATIONS
// =============================================================================

// Re-export shared types for consumers of this module
export type { IndicatorType, ChatMessage, BobeContext as BobeState } from '../../src/types/bobe'

// Import daemon types
import type {
  Goal,
  GoalListResponse,
  GoalCreateRequest,
  GoalUpdateRequest,
  GoalActionResponse,
  Soul,
  SoulListResponse,
  SoulCreateRequest,
  SoulUpdateRequest,
  SoulActionResponse,
  UserProfile,
  UserProfileListResponse,
  UserProfileCreateRequest,
  UserProfileUpdateRequest,
  UserProfileActionResponse,
  ToolListResponse,
  ToolUpdateResponse,
  MCPServerListResponse,
  MCPConfigListResponse,
  MCPServerCreateRequest,
  MCPServerCreateResponse,
  MCPServerUpdateRequest,
  MCPServerUpdateResponse,
  MCPServerDeleteResponse,
  MCPServerReconnectResponse,
  DaemonSettings,
  SettingsUpdateResponse,
  ModelsListResponse,
  ModelActionResponse,
  Memory,
  MemoryListResponse,
  MemoryListParams,
  MemoryCreateRequest,
  MemoryUpdateRequest,
  MemoryActionResponse,
} from '../types/daemon'

// Import for local use
import type { BobeContext, IndicatorType, ChatMessage } from '../../src/types/bobe'

type MediaAccessStatus = 'granted' | 'denied' | 'restricted' | 'not-determined'

export interface BobeAPI {
  getState: () => Promise<BobeContext>
  toggleCapture: () => Promise<boolean>
  dismissMessage: () => Promise<void>
  resizeForBubble: (show: boolean) => Promise<void>
  resizeWindow: (width: number, height: number) => Promise<void>
  sendMessage: (content: string) => Promise<string>
  clearMessages: () => Promise<void>
  onStateUpdate: (callback: (state: BobeContext) => void) => () => void
}

export interface GoalsAPI {
  list: () => Promise<GoalListResponse>
  get: (id: string) => Promise<Goal>
  create: (data: GoalCreateRequest) => Promise<Goal>
  update: (id: string, data: GoalUpdateRequest) => Promise<Goal>
  delete: (id: string) => Promise<GoalActionResponse>
  complete: (id: string) => Promise<GoalActionResponse>
  archive: (id: string) => Promise<GoalActionResponse>
}

export interface SoulsAPI {
  list: () => Promise<SoulListResponse>
  get: (id: string) => Promise<Soul>
  create: (data: SoulCreateRequest) => Promise<Soul>
  update: (id: string, data: SoulUpdateRequest) => Promise<Soul>
  delete: (id: string) => Promise<SoulActionResponse>
  enable: (id: string) => Promise<SoulActionResponse>
  disable: (id: string) => Promise<SoulActionResponse>
}

export interface UserProfilesAPI {
  list: () => Promise<UserProfileListResponse>
  get: (id: string) => Promise<UserProfile>
  create: (data: UserProfileCreateRequest) => Promise<UserProfile>
  update: (id: string, data: UserProfileUpdateRequest) => Promise<UserProfile>
  delete: (id: string) => Promise<UserProfileActionResponse>
  enable: (id: string) => Promise<UserProfileActionResponse>
  disable: (id: string) => Promise<UserProfileActionResponse>
}

export interface ToolsAPI {
  list: () => Promise<ToolListResponse>
  enable: (name: string) => Promise<ToolUpdateResponse>
  disable: (name: string) => Promise<ToolUpdateResponse>
}

export interface MCPServersAPI {
  list: () => Promise<MCPServerListResponse>
  listConfigs: () => Promise<MCPConfigListResponse>
  create: (data: MCPServerCreateRequest) => Promise<MCPServerCreateResponse>
  update: (id: string, data: MCPServerUpdateRequest) => Promise<MCPServerUpdateResponse>
  delete: (name: string) => Promise<MCPServerDeleteResponse>
  reconnect: (name: string) => Promise<MCPServerReconnectResponse>
}

export interface SettingsAPI {
  get: () => Promise<DaemonSettings>
  update: (data: Record<string, unknown>) => Promise<SettingsUpdateResponse>
  selectDirectory: () => Promise<string | null>
  listModels: () => Promise<ModelsListResponse>
  listRegistryModels: () => Promise<ModelsListResponse>
  pullModel: (name: string) => Promise<ModelActionResponse>
  deleteModel: (name: string) => Promise<ModelActionResponse>
}

export interface MemoriesAPI {
  list: (params?: MemoryListParams) => Promise<MemoryListResponse>
  get: (id: string) => Promise<Memory>
  create: (data: MemoryCreateRequest) => Promise<Memory>
  update: (id: string, data: MemoryUpdateRequest) => Promise<Memory>
  delete: (id: string) => Promise<MemoryActionResponse>
  enable: (id: string) => Promise<MemoryActionResponse>
  disable: (id: string) => Promise<MemoryActionResponse>
}

export interface SetupAPI {
  startLocalSetup: (modelName: string) => Promise<void>
  configureLLM: (
    mode: string,
    model: string,
    apiKey: string,
  ) => Promise<{ ok: boolean; message: string }>
  completeSetup: () => Promise<void>
  getOnboardingStatus: () => Promise<{
    complete: boolean
    needs_onboarding: boolean
    steps: Record<string, { status: string; detail: string }>
  } | null>
  onProgress: (
    callback: (data: { step: string; progress: number; message: string }) => void,
  ) => () => void
}

export interface AppDataAPI {
  deleteAllData: () => Promise<void>
  getDataSize: () => Promise<{ totalMB: number; breakdown: Record<string, number> }>
}

export interface PermissionsAPI {
  checkScreen: () => Promise<MediaAccessStatus>
  openScreenSettings: () => Promise<void>
  checkDataDir: () => Promise<{ ok: boolean; error?: string }>
}

declare global {
  interface Window {
    bobe: BobeAPI
    setup: SetupAPI
    appData: AppDataAPI
    goals: GoalsAPI
    souls: SoulsAPI
    userProfiles: UserProfilesAPI
    tools: ToolsAPI
    mcpServers: MCPServersAPI
    settings: SettingsAPI
    memories: MemoriesAPI
    permissions: PermissionsAPI
  }
}
