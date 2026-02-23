/**
 * IPC channel contracts - type-safe communication between processes
 */

import type { BobeContext } from './bobe'
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
  SettingsUpdateRequest,
  SettingsUpdateResponse,
  ModelsListResponse,
  ModelActionResponse,
  Memory,
  MemoryListResponse,
  MemoryListParams,
  MemoryCreateRequest,
  MemoryUpdateRequest,
  MemoryActionResponse,
} from './api'

// =============================================================================
// INVOKE CHANNELS (Renderer → Main → Python service)
// Request/response pattern using ipcMain.handle / ipcRenderer.invoke
// =============================================================================

export interface InvokeChannels {
  // Core BoBe channels
  'bobe:get-state': () => Promise<BobeContext>
  'bobe:toggle-capture': () => Promise<boolean>
  'bobe:dismiss-message': () => Promise<void>
  'bobe:resize-for-bubble': (show: boolean) => Promise<void>
  'bobe:resize-window': (width: number, height: number) => Promise<void>
  'bobe:send-message': (content: string) => Promise<string>
  'bobe:clear-messages': () => Promise<void>

  // Goals API
  'goals:list': () => Promise<GoalListResponse>
  'goals:get': (id: string) => Promise<Goal>
  'goals:create': (data: GoalCreateRequest) => Promise<Goal>
  'goals:update': (id: string, data: GoalUpdateRequest) => Promise<Goal>
  'goals:delete': (id: string) => Promise<GoalActionResponse>
  'goals:complete': (id: string) => Promise<GoalActionResponse>
  'goals:archive': (id: string) => Promise<GoalActionResponse>

  // Souls API
  'souls:list': () => Promise<SoulListResponse>
  'souls:get': (id: string) => Promise<Soul>
  'souls:create': (data: SoulCreateRequest) => Promise<Soul>
  'souls:update': (id: string, data: SoulUpdateRequest) => Promise<Soul>
  'souls:delete': (id: string) => Promise<SoulActionResponse>
  'souls:enable': (id: string) => Promise<SoulActionResponse>
  'souls:disable': (id: string) => Promise<SoulActionResponse>

  // User Profiles API
  'user-profiles:list': () => Promise<UserProfileListResponse>
  'user-profiles:get': (id: string) => Promise<UserProfile>
  'user-profiles:create': (data: UserProfileCreateRequest) => Promise<UserProfile>
  'user-profiles:update': (id: string, data: UserProfileUpdateRequest) => Promise<UserProfile>
  'user-profiles:delete': (id: string) => Promise<UserProfileActionResponse>
  'user-profiles:enable': (id: string) => Promise<UserProfileActionResponse>
  'user-profiles:disable': (id: string) => Promise<UserProfileActionResponse>

  // Tools API
  'tools:list': () => Promise<ToolListResponse>
  'tools:enable': (name: string) => Promise<ToolUpdateResponse>
  'tools:disable': (name: string) => Promise<ToolUpdateResponse>

  // MCP Servers API
  'mcp-servers:list': () => Promise<MCPServerListResponse>
  'mcp-servers:list-configs': () => Promise<MCPConfigListResponse>
  'mcp-servers:create': (data: MCPServerCreateRequest) => Promise<MCPServerCreateResponse>
  'mcp-servers:update': (
    id: string,
    data: MCPServerUpdateRequest,
  ) => Promise<MCPServerUpdateResponse>
  'mcp-servers:delete': (name: string) => Promise<MCPServerDeleteResponse>
  'mcp-servers:reconnect': (name: string) => Promise<MCPServerReconnectResponse>

  // Settings API
  'settings:get': () => Promise<DaemonSettings>
  'settings:update': (data: SettingsUpdateRequest) => Promise<SettingsUpdateResponse>

  // Models API
  'models:list': () => Promise<ModelsListResponse>
  'models:registry': () => Promise<ModelsListResponse>
  'models:pull': (name: string) => Promise<ModelActionResponse>
  'models:delete': (name: string) => Promise<ModelActionResponse>

  // Memories API
  'memories:list': (params?: MemoryListParams) => Promise<MemoryListResponse>
  'memories:get': (id: string) => Promise<Memory>
  'memories:create': (data: MemoryCreateRequest) => Promise<Memory>
  'memories:update': (id: string, data: MemoryUpdateRequest) => Promise<Memory>
  'memories:delete': (id: string) => Promise<MemoryActionResponse>
  'memories:enable': (id: string) => Promise<MemoryActionResponse>
  'memories:disable': (id: string) => Promise<MemoryActionResponse>

  // Permissions API
  'permissions:check-screen': () => Promise<MediaAccessStatus>
  'permissions:open-screen-settings': () => Promise<void>
  'permissions:check-data-dir': () => Promise<{ ok: boolean; error?: string }>
}

// Channel names as const array for runtime validation
export const INVOKE_CHANNELS = [
  // Core BoBe channels
  'bobe:get-state',
  'bobe:toggle-capture',
  'bobe:dismiss-message',
  'bobe:resize-for-bubble',
  'bobe:resize-window',
  'bobe:send-message',
  'bobe:clear-messages',
  // Goals API
  'goals:list',
  'goals:get',
  'goals:create',
  'goals:update',
  'goals:delete',
  'goals:complete',
  'goals:archive',
  // Souls API
  'souls:list',
  'souls:get',
  'souls:create',
  'souls:update',
  'souls:delete',
  'souls:enable',
  'souls:disable',
  // User Profiles API
  'user-profiles:list',
  'user-profiles:get',
  'user-profiles:create',
  'user-profiles:update',
  'user-profiles:delete',
  'user-profiles:enable',
  'user-profiles:disable',
  // Tools API
  'tools:list',
  'tools:enable',
  'tools:disable',
  // MCP Servers API
  'mcp-servers:list',
  'mcp-servers:list-configs',
  'mcp-servers:create',
  'mcp-servers:update',
  'mcp-servers:delete',
  'mcp-servers:reconnect',
  // Settings API
  'settings:get',
  'settings:update',
  // Models API
  'models:list',
  'models:registry',
  'models:pull',
  'models:delete',
  // Memories API
  'memories:list',
  'memories:get',
  'memories:create',
  'memories:update',
  'memories:delete',
  'memories:enable',
  'memories:disable',
  // Permissions API
  'permissions:check-screen',
  'permissions:open-screen-settings',
  'permissions:check-data-dir',
] as const

export type InvokeChannel = (typeof INVOKE_CHANNELS)[number]

// =============================================================================
// EVENT CHANNELS (Main → Renderer)
// Push-based events using webContents.send / ipcRenderer.on
// =============================================================================

export interface EventChannels {
  'bobe:state-update': (state: BobeContext) => void
}

// Channel names as const array for runtime validation
export const EVENT_CHANNELS = ['bobe:state-update'] as const

export type EventChannel = (typeof EVENT_CHANNELS)[number]

// =============================================================================
// PRELOAD API (exposed via contextBridge)
// =============================================================================

export interface BobeAPI {
  // Commands (invoke channels)
  getState: () => Promise<BobeContext>
  toggleCapture: () => Promise<boolean>
  dismissMessage: () => Promise<void>
  resizeForBubble: (show: boolean) => Promise<void>
  resizeWindow: (width: number, height: number) => Promise<void>
  sendMessage: (content: string) => Promise<string>
  clearMessages: () => Promise<void>

  // Subscriptions (event channels)
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

export type MediaAccessStatus = 'granted' | 'denied' | 'restricted' | 'not-determined'

export interface PermissionsAPI {
  checkScreen: () => Promise<MediaAccessStatus>
  openScreenSettings: () => Promise<void>
  checkDataDir: () => Promise<{ ok: boolean; error?: string }>
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
